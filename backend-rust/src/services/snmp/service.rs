//! Casos de uso SNMP para controllers, scheduler e checker.

use chrono::Utc;
use futures::future;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::services::{
    monitoring::{
        device_status::{self, DeviceStatus},
        interface_monitoring,
    },
    shared::errors::{AppError, AppResult},
    snmp::{
        client::{SnmpClient, SnmpConfig, SnmpError, SnmpVersion},
        collectors::{
            collect_cpu, collect_interfaces, collect_lldp, collect_memory, collect_system,
            collect_traffic, status_label, InterfaceTraffic, LldpNeighbor, SnmpCpuInfo,
            SnmpInterface, SnmpMemoryInfo, SnmpSystemInfo,
        },
    },
    zabbix::collector::ZabbixTemplateItemReading,
};
use crate::{
    models::{
        _entities::{
            device_interfaces as device_interfaces_entity, metrics as metrics_entity,
            monitors as monitors_entity,
        },
        device_interfaces, devices, metrics, monitors,
    },
    services::topology,
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpTestResult {
    pub success: bool,
    pub system: SnmpSystemInfo,
    pub message: String,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpScanResult {
    pub snmp_responded: bool,
    pub system_info: SnmpSystemInfo,
    pub interfaces: Vec<SnmpInterface>,
    pub traffic: Vec<InterfaceTraffic>,
    pub cpu_info: SnmpCpuInfo,
    pub memory_info: SnmpMemoryInfo,
    pub neighbors: Vec<LldpNeighbor>,
    /// Campos abaixo dependem do banco: a coleta pura os devolve zerados e
    /// `scan_device` os preenche para a tela de descoberta.
    pub has_cpu_monitor: bool,
    pub has_memory_monitor: bool,
    pub zabbix_template_items: Vec<ZabbixTemplateItemReading>,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpDetectResult {
    pub detected: bool,
    pub version: Option<String>,
    pub community: Option<String>,
    pub result: Option<SnmpTestResult>,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpPollResult {
    pub scan: SnmpScanResult,
    pub interfaces_synced: usize,
    pub metrics_recorded: usize,
    pub links_resolved: usize,
    pub reboot_detected: bool,
}
/// Nomes dos monitores criados pela tela de descoberta. São a chave que liga o
/// que o usuário marcou no diálogo ao que já existe no banco, então mudá-los
/// desgarra os monitores já criados.
pub const CPU_MONITOR_NAME: &str = "Monitor de Uso de CPU";
pub const MEMORY_MONITOR_NAME: &str = "Monitor de Uso de Memoria";

#[must_use]
pub fn interface_monitor_name(if_name: &str) -> String {
    format!("Interface {if_name}")
}

#[derive(Debug, Default)]
pub struct SnmpApplyOptions {
    pub enable_cpu_monitor: Option<bool>,
    pub enable_memory_monitor: Option<bool>,
    pub monitored_if_indexes: Vec<i32>,
}

pub async fn test_connection(config: SnmpConfig) -> AppResult<SnmpTestResult> {
    let system = collect_system(&super::client::SnmpClient::new(config))
        .await
        .map_err(map_error)?;
    let success = system.responded();
    Ok(SnmpTestResult {
        success,
        system,
        message: if success {
            "Conexão SNMP estabelecida".into()
        } else {
            "Agente SNMP respondeu sem OIDs utilizáveis".into()
        },
    })
}

pub async fn scan(config: SnmpConfig) -> AppResult<SnmpScanResult> {
    let client = super::client::SnmpClient::new(config);
    let (system, interfaces, cpu, memory, traffic, neighbors) = tokio::join!(
        collect_system(&client),
        collect_interfaces(&client),
        collect_cpu(&client),
        collect_memory(&client),
        collect_traffic(&client),
        collect_lldp(&client)
    );
    let system = system.map_err(map_error)?;
    Ok(SnmpScanResult {
        snmp_responded: system.responded(),
        system_info: system,
        interfaces: interfaces.unwrap_or_default(),
        traffic: traffic.unwrap_or_default(),
        cpu_info: cpu.unwrap_or_default(),
        memory_info: memory.unwrap_or_default(),
        neighbors: neighbors.unwrap_or_default(),
        has_cpu_monitor: false,
        has_memory_monitor: false,
        zabbix_template_items: vec![],
    })
}

/// Varredura para a tela "Escaneamento & Descoberta SNMP".
///
/// É o `scan` puro mais o que só o banco sabe: quais monitores já estão
/// habilitados — para o diálogo abrir refletindo o que já está configurado, em
/// vez de propor tudo de novo — e a leitura dos itens do template Zabbix.
pub async fn scan_device(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
) -> AppResult<SnmpScanResult> {
    let client = SnmpClient::new(config.clone());
    let mut scan = scan(config).await?;
    let existing = monitors::Entity::find()
        .filter(monitors_entity::Column::DeviceId.eq(Some(device.id)))
        .filter(monitors_entity::Column::Enabled.eq(true))
        .all(&ctx.db)
        .await?;
    let enabled = |name: &str| existing.iter().any(|monitor| monitor.name == name);
    scan.has_cpu_monitor = enabled(CPU_MONITOR_NAME);
    scan.has_memory_monitor = enabled(MEMORY_MONITOR_NAME);
    for interface in &mut scan.interfaces {
        interface.is_monitored = enabled(&interface_monitor_name(&interface.if_name));
    }
    scan.zabbix_template_items =
        crate::services::zabbix::collector::preview(&ctx.db, device, &client).await?;
    Ok(scan)
}

pub async fn poll_device(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
) -> AppResult<SnmpPollResult> {
    let client = SnmpClient::new(config.clone());
    let scan = scan(config).await?;
    if !scan.snmp_responded {
        return Ok(SnmpPollResult {
            scan,
            interfaces_synced: 0,
            metrics_recorded: 0,
            links_resolved: 0,
            reboot_detected: false,
        });
    }

    let mut interfaces = std::collections::BTreeMap::new();
    for interface in &scan.interfaces {
        let saved = sync_interface(&ctx.db, device.id, interface).await?;
        // Só interface administrativamente habilitada é avaliada: uma porta
        // que o operador desligou não pode gerar alerta de queda de link.
        if saved.interface.admin_status.as_deref() == Some("up") {
            interface_monitoring::evaluate_interface_state(
                ctx,
                device,
                &saved.interface,
                saved.previous_oper_status.as_deref(),
                saved.previous_speed,
            )
            .await?;
        }
        interfaces.insert(interface.if_index, saved.interface);
    }
    let previous_uptime = latest_device_metric(&ctx.db, device.id, "snmp_uptime")
        .await?
        .map(|metric| metric.value.max(0.0) as u64);
    let mut metrics_recorded = 0;
    let mut reboot_detected = false;
    for traffic in &scan.traffic {
        let Some(interface) = interfaces.get(&traffic.if_index) else {
            continue;
        };
        let (recorded, reboot) = persist_traffic(
            &ctx.db,
            device.id,
            interface,
            traffic,
            previous_uptime,
            scan.system_info.sys_up_time,
        )
        .await?;
        metrics_recorded += recorded;
        reboot_detected |= reboot;
    }
    persist_system_metrics(&ctx.db, device.id, &scan).await?;
    crate::services::zabbix::collector::sync_zabbix_template_monitor(&ctx.db, device).await?;
    metrics_recorded +=
        crate::services::zabbix::collector::collect(&ctx.db, device, &client).await? as usize;
    // O status **não** é escrito aqui (matriz de paridade #4): quem decide é o
    // `device_status`, agregando todos os monitores habilitados. Gravar "online"
    // direto era o bug de alternância — a coleta subia o dispositivo em silêncio
    // e o ping seguinte o derrubava, publicando transição a cada ciclo. A coleta
    // entra como `observed_status`, que só prevalece quando não há monitor algum.
    device_status::refresh_from_monitors(ctx, device, Some(DeviceStatus::Online), Some(Utc::now()))
        .await?;
    let links_resolved = topology::resolve_discovered_neighbors(ctx, device, &scan.neighbors)
        .await?
        .len();
    Ok(SnmpPollResult {
        scan,
        interfaces_synced: interfaces.len(),
        metrics_recorded,
        links_resolved,
        reboot_detected,
    })
}

pub async fn apply_monitors(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
    options: SnmpApplyOptions,
) -> AppResult<()> {
    let scan = scan(config.clone()).await?;
    devices::ActiveModel {
        id: Set(device.id),
        snmp_enabled: Set(true),
        is_monitored: Set(true),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    let selected = options
        .monitored_if_indexes
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    for source in &scan.interfaces {
        let interface = sync_interface(&ctx.db, device.id, source).await?.interface;
        let enabled = selected.contains(&source.if_index);
        device_interfaces::ActiveModel {
            id: Set(interface.id),
            admin_status: Set(Some(if enabled { "up" } else { "down" }.into())),
            ..Default::default()
        }
        .update(&ctx.db)
        .await?;
        let name = interface_monitor_name(&source.if_name);
        sync_monitor(
            &ctx.db,
            device.id,
            &name,
            enabled,
            serde_json::json!({
                "host": config.host.clone(),
                "version": version_name(config.version),
                "community": config.community.clone(),
                "port": config.port,
                "timeoutMs": config.timeout_ms,
                "ifIndex": source.if_index,
                "ifName": source.if_name,
                "metric": "traffic",
            }),
            source.if_oper_status == Some(1),
        )
        .await?;
        if !enabled {
            metrics::Entity::delete_many()
                .filter(metrics_entity::Column::InterfaceId.eq(Some(interface.id)))
                .exec(&ctx.db)
                .await?;
        }
    }
    if let Some(enabled) = options.enable_cpu_monitor {
        sync_monitor(
            &ctx.db,
            device.id,
            CPU_MONITOR_NAME,
            enabled,
            monitor_configuration(&config, "cpu_usage"),
            true,
        )
        .await?;
        if !enabled {
            metrics::Entity::delete_many()
                .filter(metrics_entity::Column::DeviceId.eq(device.id))
                .filter(metrics_entity::Column::Name.eq("cpu_usage"))
                .exec(&ctx.db)
                .await?;
        }
    }
    if let Some(enabled) = options.enable_memory_monitor {
        sync_monitor(
            &ctx.db,
            device.id,
            MEMORY_MONITOR_NAME,
            enabled,
            monitor_configuration(&config, "memory_usage"),
            true,
        )
        .await?;
        if !enabled {
            metrics::Entity::delete_many()
                .filter(metrics_entity::Column::DeviceId.eq(device.id))
                .filter(metrics_entity::Column::Name.eq("memory_usage"))
                .exec(&ctx.db)
                .await?;
        }
    }
    // Um poll inicial deixa a configuracao e a primeira visualizacao coerentes.
    let _ = poll_device(ctx, device, config).await;
    Ok(())
}

fn monitor_configuration(config: &SnmpConfig, metric: &str) -> serde_json::Value {
    serde_json::json!({
        "host": config.host,
        "version": version_name(config.version),
        "community": config.community,
        "port": config.port,
        "timeoutMs": config.timeout_ms,
        "metric": metric,
    })
}

async fn sync_monitor(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    name: &str,
    enabled: bool,
    configuration: serde_json::Value,
    up: bool,
) -> AppResult<()> {
    let existing = monitors::Entity::find()
        .filter(monitors_entity::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors_entity::Column::Name.eq(name))
        .one(db)
        .await?;
    if let Some(existing) = existing {
        monitors::ActiveModel {
            id: Set(existing.id),
            configuration: Set(configuration),
            enabled: Set(enabled),
            status: Set(if up { "up" } else { "down" }.into()),
            ..Default::default()
        }
        .update(db)
        .await?;
    } else {
        monitors::ActiveModel {
            device_id: Set(Some(device_id)),
            probe_id: Set(None),
            r#type: Set("snmp".into()),
            name: Set(name.into()),
            configuration: Set(configuration),
            interval_seconds: Set(60),
            timeout_seconds: Set(10),
            retry_count: Set(3),
            enabled: Set(enabled),
            status: Set(if up { "up" } else { "down" }.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

fn version_name(version: SnmpVersion) -> &'static str {
    match version {
        SnmpVersion::V1 => "v1",
        SnmpVersion::V2c => "v2c",
        SnmpVersion::V3 => "v3",
    }
}

/// Uma interface persistida junto do estado que ela tinha antes da coleta.
///
/// O "antes" é lido aqui e não pelo chamador porque é a última janela em que
/// ele ainda existe: depois do `update` a linha já carrega o estado novo, e o
/// motor de alertas não teria como saber que houve transição.
pub struct SyncedInterface {
    pub interface: device_interfaces::Model,
    pub previous_oper_status: Option<String>,
    pub previous_speed: Option<i64>,
}

/// Espelha uma interface lida do agente na tabela local.
///
/// Público porque a matriz de paridade #20 — o `adminStatus` escolhido pelo
/// operador sobrevive ao poll — só se prova contra o banco.
pub async fn sync_interface(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    source: &SnmpInterface,
) -> AppResult<SyncedInterface> {
    let existing = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(device_id))
        .filter(device_interfaces_entity::Column::SnmpIndex.eq(Some(source.if_index)))
        .one(db)
        .await?;
    let now = Utc::now();
    let admin_status = existing.as_ref().and_then(|row| row.admin_status.clone());
    let previous_oper_status = existing.as_ref().and_then(|row| row.oper_status.clone());
    let previous_speed = existing.as_ref().and_then(|row| row.speed);
    let model = device_interfaces::ActiveModel {
        id: existing.as_ref().map(|row| Set(row.id)).unwrap_or_default(),
        device_id: Set(device_id),
        snmp_index: Set(Some(source.if_index)),
        name: Set(source.if_name.clone()),
        description: Set(source.if_descr.clone()),
        alias: Set(source.if_alias.clone()),
        mac_address: Set(source.mac_address.clone()),
        r#type: Set(source.if_type.map(|kind| kind.to_string())),
        speed: Set(source.if_speed.and_then(|speed| i64::try_from(speed).ok())),
        // Uma escolha manual do usuário prevalece sobre o valor observado no poll.
        admin_status: Set(admin_status.or_else(|| {
            source
                .if_admin_status
                .map(|value| status_label(value).into())
        })),
        oper_status: Set(source
            .if_oper_status
            .map(|value| status_label(value).into())),
        last_seen_at: Set(Some(now.into())),
        ..Default::default()
    };
    let interface = if existing.is_some() {
        model.update(db).await?
    } else {
        model.insert(db).await?
    };
    Ok(SyncedInterface {
        interface,
        previous_oper_status,
        previous_speed,
    })
}

async fn persist_traffic(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    interface: &device_interfaces::Model,
    current: &InterfaceTraffic,
    previous_uptime: Option<u64>,
    current_uptime: Option<u64>,
) -> AppResult<(usize, bool)> {
    let previous_in = latest_metric(db, interface.id, "ifHCInOctets").await?;
    let previous_out = latest_metric(db, interface.id, "ifHCOutOctets").await?;
    let mut recorded = 0;
    for (name, value) in [
        ("ifHCInOctets", current.in_octets as f64),
        ("ifHCOutOctets", current.out_octets as f64),
        ("ifInErrors", current.in_errors as f64),
        ("ifOutErrors", current.out_errors as f64),
    ] {
        record_metric(
            db,
            device_id,
            Some(interface.id),
            name,
            value,
            "bytes",
            current.recorded_at,
        )
        .await?;
        recorded += 1;
    }
    let (Some(previous_in), Some(previous_out)) = (previous_in, previous_out) else {
        return Ok((recorded, false));
    };
    let previous = InterfaceTraffic {
        if_index: current.if_index,
        in_octets: previous_in.value.max(0.0) as u64,
        out_octets: previous_out.value.max(0.0) as u64,
        in_errors: 0,
        out_errors: 0,
        counter_bits: current.counter_bits,
        recorded_at: previous_in.recorded_at.with_timezone(&Utc),
    };
    let rates = crate::services::snmp::collectors::calculate_rates_detailed(
        &previous,
        current,
        previous_uptime,
        current_uptime,
    );
    if !rates.reboot_detected {
        record_metric(
            db,
            device_id,
            Some(interface.id),
            "inBps",
            rates.in_bps,
            "bps",
            current.recorded_at,
        )
        .await?;
        record_metric(
            db,
            device_id,
            Some(interface.id),
            "outBps",
            rates.out_bps,
            "bps",
            current.recorded_at,
        )
        .await?;
        recorded += 2;
    }
    Ok((recorded, rates.reboot_detected))
}

async fn persist_system_metrics(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    scan: &SnmpScanResult,
) -> AppResult<()> {
    let recorded_at = Utc::now();
    for (name, value, unit) in [
        ("cpu_usage", scan.cpu_info.usage_percent, "percent"),
        ("memory_used", scan.memory_info.used_percent, "percent"),
        (
            "snmp_uptime",
            scan.system_info.sys_up_time.map(|uptime| uptime as f64),
            "ticks",
        ),
    ] {
        if let Some(value) = value {
            record_metric(db, device_id, None, name, value, unit, recorded_at).await?;
        }
    }
    Ok(())
}

async fn latest_metric(
    db: &sea_orm::DatabaseConnection,
    interface_id: i64,
    name: &str,
) -> AppResult<Option<metrics::Model>> {
    Ok(metrics::Entity::find()
        .filter(metrics_entity::Column::InterfaceId.eq(Some(interface_id)))
        .filter(metrics_entity::Column::Name.eq(name))
        .order_by_desc(metrics_entity::Column::RecordedAt)
        .one(db)
        .await?)
}

async fn latest_device_metric(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    name: &str,
) -> AppResult<Option<metrics::Model>> {
    Ok(metrics::Entity::find()
        .filter(metrics_entity::Column::DeviceId.eq(device_id))
        .filter(metrics_entity::Column::InterfaceId.is_null())
        .filter(metrics_entity::Column::Name.eq(name))
        .order_by_desc(metrics_entity::Column::RecordedAt)
        .one(db)
        .await?)
}

async fn record_metric(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    interface_id: Option<i64>,
    name: &str,
    value: f64,
    unit: &str,
    recorded_at: chrono::DateTime<Utc>,
) -> AppResult<()> {
    metrics::ActiveModel {
        device_id: Set(device_id),
        interface_id: Set(interface_id),
        monitor_id: Set(None),
        name: Set(name.into()),
        value: Set(value),
        unit: Set(unit.into()),
        recorded_at: Set(recorded_at.into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

pub async fn detect_connection(
    host: &str,
    port: u16,
    preferred: Option<SnmpConfig>,
) -> AppResult<SnmpDetectResult> {
    let mut candidates = Vec::new();
    if let Some(config) = preferred {
        candidates.push(config);
    }
    for (version, community) in [
        (SnmpVersion::V2c, "public"),
        (SnmpVersion::V2c, "private"),
        (SnmpVersion::V1, "public"),
        (SnmpVersion::V1, "private"),
    ] {
        if !candidates
            .iter()
            .any(|config| config.version == version && config.community == community)
        {
            let mut config = SnmpConfig::v2c(host, community, port);
            config.version = version;
            config.timeout_ms = 2_500;
            candidates.push(config);
        }
    }
    let attempts = candidates.into_iter().map(|config| async move {
        let label = (config.version, config.community.clone());
        test_connection(config).await.map(|result| (label, result))
    });
    let outcomes = future::join_all(attempts).await;
    match outcomes.into_iter().find_map(Result::ok) {
        Some(((version, community), result)) => Ok(SnmpDetectResult {
            detected: result.success,
            version: Some(
                match version {
                    SnmpVersion::V1 => "v1",
                    SnmpVersion::V2c => "v2c",
                    SnmpVersion::V3 => "v3",
                }
                .into(),
            ),
            community: Some(community),
            result: Some(result),
        }),
        None => Ok(SnmpDetectResult {
            detected: false,
            version: None,
            community: None,
            result: None,
        }),
    }
}
fn map_error(error: SnmpError) -> AppError {
    AppError::BusinessRule(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O diálogo "Escaneamento & Descoberta SNMP" lê estes campos direto do
    /// JSON. Quando eles saíam como `system`/`cpu`/`memory`, a tela quebrava em
    /// `Cannot read properties of undefined` antes de renderizar qualquer coisa.
    #[test]
    fn a_varredura_sai_com_os_campos_que_a_tela_de_descoberta_le() {
        let scan = SnmpScanResult {
            snmp_responded: true,
            system_info: SnmpSystemInfo::default(),
            interfaces: vec![SnmpInterface {
                if_index: 1,
                if_name: "eth0".into(),
                if_descr: None,
                if_alias: None,
                if_type: None,
                if_speed: None,
                if_admin_status: Some(1),
                if_oper_status: Some(1),
                mac_address: None,
                is_monitored: true,
            }],
            traffic: vec![],
            cpu_info: SnmpCpuInfo::default(),
            memory_info: SnmpMemoryInfo::default(),
            neighbors: vec![],
            has_cpu_monitor: true,
            has_memory_monitor: false,
            zabbix_template_items: vec![],
        };
        let json = serde_json::to_value(&scan).unwrap();

        for campo in [
            "snmpResponded",
            "systemInfo",
            "cpuInfo",
            "memoryInfo",
            "interfaces",
            "hasCpuMonitor",
            "hasMemoryMonitor",
            "zabbixTemplateItems",
        ] {
            assert!(json.get(campo).is_some(), "campo ausente: {campo}");
        }
        assert_eq!(json["interfaces"][0]["isMonitored"], true);
        assert_eq!(json["hasCpuMonitor"], true);
    }
}
