//! Casos de uso SNMP para controllers, scheduler e checker.

use chrono::{DateTime, Utc};
use futures::future;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::services::{
    monitoring::{
        contracts::{CheckResult, MonitorStatus},
        device_status::{self, DeviceStatus},
        execution_guard::calculate_smart_timeout_seconds,
        interface_monitoring,
    },
    shared::errors::{AppError, AppResult},
    snmp::{
        client::{SnmpConfig, SnmpError, SnmpVersion},
        collectors::{
            collect_cpu, collect_interfaces_and_traffic, collect_lldp, collect_memory,
            collect_system, status_label, InterfaceTraffic, LldpNeighbor, SnmpCpuInfo,
            SnmpInterface, SnmpMemoryInfo, SnmpSystemInfo,
        },
    },
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
    /// Falhas isoladas por coletor. Dados válidos dos demais coletores são
    /// preservados e a UI consegue explicar por que uma seção ficou vazia.
    pub collector_errors: std::collections::BTreeMap<String, String>,
    /// Campos abaixo dependem do banco: a coleta pura os devolve zerados e
    /// `scan_device` os preenche para a tela de descoberta.
    pub has_cpu_monitor: bool,
    pub has_memory_monitor: bool,
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
pub const DEFAULT_SNMP_POLL_INTERVAL_SECONDS: i32 = 15;

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

/// Monta a configuração de coleta a partir do cadastro canônico do dispositivo.
/// Os monitores SNMP vinculados a ele não devem manter credenciais ou alvos
/// divergentes, pois todos participam da mesma coleta.
pub fn device_config(device: &devices::Model) -> AppResult<SnmpConfig> {
    let host = device
        .ip_address
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| device.name.clone());
    let mut config = SnmpConfig::v2c(
        host,
        device.snmp_community.as_deref().unwrap_or("public"),
        161,
    );
    config.version = SnmpVersion::parse(device.snmp_version.as_deref().unwrap_or("v2c"))
        .ok_or_else(|| AppError::validation("Versão SNMP inválida"))?;
    Ok(config)
}

/// Mantém a cadência e o timeout de todos os itens SNMP do equipamento alinhados
/// ao intervalo que pertence ao próprio dispositivo.
pub async fn sync_monitor_intervals(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    interval_seconds: i32,
) -> AppResult<()> {
    let interval_seconds = interval_seconds.max(1);
    let timeout_seconds = calculate_smart_timeout_seconds("snmp", interval_seconds)
        .min((interval_seconds - 1).max(1))
        .max(1);
    monitors::Entity::update_many()
        .col_expr(
            monitors_entity::Column::IntervalSeconds,
            Expr::value(interval_seconds),
        )
        .col_expr(
            monitors_entity::Column::TimeoutSeconds,
            Expr::value(timeout_seconds),
        )
        .filter(monitors_entity::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors_entity::Column::Type.eq("snmp"))
        .exec(db)
        .await?;
    Ok(())
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
    let (system, interfaces_and_traffic, cpu, memory, neighbors) = tokio::join!(
        collect_system(&client),
        collect_interfaces_and_traffic(&client),
        collect_cpu(&client),
        collect_memory(&client),
        collect_lldp(&client)
    );
    let system = system.map_err(map_error)?;
    let mut collector_errors = std::collections::BTreeMap::new();
    let (interfaces, traffic) = match interfaces_and_traffic {
        Ok(value) => value,
        Err(error) => {
            collector_errors.insert("interfaces".into(), error.to_string());
            Default::default()
        }
    };
    let cpu_info = cpu.unwrap_or_else(|error| {
        collector_errors.insert("cpu".into(), error.to_string());
        Default::default()
    });
    let memory_info = memory.unwrap_or_else(|error| {
        collector_errors.insert("memory".into(), error.to_string());
        Default::default()
    });
    let neighbors = neighbors.unwrap_or_else(|error| {
        collector_errors.insert("neighbors".into(), error.to_string());
        Default::default()
    });
    Ok(SnmpScanResult {
        snmp_responded: system.responded(),
        system_info: system,
        interfaces,
        traffic,
        cpu_info,
        memory_info,
        neighbors,
        collector_errors,
        has_cpu_monitor: false,
        has_memory_monitor: false,
    })
}

/// Varredura para a tela "Escaneamento & Descoberta SNMP".
///
/// É o `scan` puro mais o que só o banco sabe: quais monitores já estão
/// habilitados — para o diálogo abrir refletindo o que já está configurado, em
/// vez de propor tudo de novo.
pub async fn scan_device(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
) -> AppResult<SnmpScanResult> {
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
    Ok(scan)
}

pub async fn poll_device(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
) -> AppResult<SnmpPollResult> {
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

/// Executa uma coleta única e deriva o resultado individual de cada monitor
/// SNMP do dispositivo. Assim o histórico e os alertas continuam por item, mas
/// CPU, memória, interfaces e tráfego não abrem consultas independentes.
pub async fn poll_device_monitors(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    monitored_items: &[monitors::Model],
) -> Vec<(i64, CheckResult)> {
    let started_at = Utc::now();
    let poll = match device_config(device) {
        Ok(config) => poll_device(ctx, device, config).await,
        Err(error) => Err(error),
    };
    let finished_at = Utc::now();
    match poll {
        Ok(poll) => monitored_items
            .iter()
            .map(|monitor| {
                (
                    monitor.id,
                    monitor_result_from_poll(monitor, &poll, started_at, finished_at),
                )
            })
            .collect(),
        Err(error) => monitored_items
            .iter()
            .map(|monitor| {
                (
                    monitor.id,
                    failed_monitor_result(error.to_string(), started_at, finished_at),
                )
            })
            .collect(),
    }
}

fn monitor_result_from_poll(
    monitor: &monitors::Model,
    poll: &SnmpPollResult,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> CheckResult {
    let config = &monitor.configuration;
    let if_index = config
        .get("ifIndex")
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let metric = config
        .get("metric")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(if if_index.is_some() {
            "interface_status"
        } else {
            "uptime"
        });
    let (status, message, data) = match metric {
        "cpu_usage" if poll.scan.cpu_info.usage_percent.is_some() => (
            MonitorStatus::Up,
            "Uso de CPU coletado".to_string(),
            serde_json::json!({ "usagePercent": poll.scan.cpu_info.usage_percent }),
        ),
        "memory_usage" if poll.scan.memory_info.used_percent.is_some() => (
            MonitorStatus::Up,
            "Uso de memória coletado".to_string(),
            serde_json::json!({ "usedPercent": poll.scan.memory_info.used_percent }),
        ),
        "traffic" | "interface_traffic" => match if_index.and_then(|index| {
            poll.scan
                .traffic
                .iter()
                .find(|traffic| traffic.if_index == index)
        }) {
            Some(traffic) => (
                MonitorStatus::Up,
                "Tráfego de interface coletado".to_string(),
                serde_json::json!({
                    "ifIndex": traffic.if_index,
                    "counterBits": traffic.counter_bits,
                }),
            ),
            None => missing_measurement_result("contadores de tráfego da interface"),
        },
        "interface_status" | "status" => match if_index.and_then(|index| {
            poll.scan
                .interfaces
                .iter()
                .find(|interface| interface.if_index == index)
        }) {
            Some(interface) => {
                let status = interface_monitor_status(
                    interface.if_admin_status.unwrap_or(1),
                    interface.if_oper_status.unwrap_or(1),
                );
                (
                    status,
                    "Estado da interface coletado".to_string(),
                    serde_json::json!({
                        "ifName": interface.if_name,
                        "ifIndex": interface.if_index,
                        "adminStatus": interface.if_admin_status.map(status_label),
                        "operStatus": interface.if_oper_status.map(status_label),
                    }),
                )
            }
            None => missing_measurement_result("estado da interface"),
        },
        "cpu_usage" => missing_measurement_result("uso de CPU"),
        "memory_usage" => missing_measurement_result("uso de memória"),
        _ if poll.scan.snmp_responded => (
            MonitorStatus::Up,
            "Agente SNMP respondeu à coleta consolidada".to_string(),
            serde_json::json!({ "sysUpTime": poll.scan.system_info.sys_up_time }),
        ),
        _ => missing_measurement_result("resposta do agente SNMP"),
    };
    CheckResult {
        success: matches!(status, MonitorStatus::Up | MonitorStatus::Disabled),
        status,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message: Some(message),
        metrics: vec![],
        data,
    }
}

fn missing_measurement_result(measurement: &str) -> (MonitorStatus, String, serde_json::Value) {
    (
        MonitorStatus::Down,
        format!("A coleta SNMP não informou {measurement}"),
        serde_json::json!({ "reason": "measurement_missing", "measurement": measurement }),
    )
}

fn failed_monitor_result(
    error: String,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> CheckResult {
    CheckResult {
        success: false,
        status: MonitorStatus::Down,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message: Some(format!("Falha na coleta SNMP consolidada: {error}")),
        metrics: vec![],
        data: serde_json::json!({ "reason": "snmp_poll_failed" }),
    }
}

fn interface_monitor_status(admin: u64, oper: u64) -> MonitorStatus {
    if admin == 2 {
        MonitorStatus::Disabled
    } else if oper == 1 {
        MonitorStatus::Up
    } else if oper == 2 {
        MonitorStatus::Down
    } else {
        MonitorStatus::Warning
    }
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
        set_monitoring(
            &ctx.db,
            device.id,
            &config,
            &interface,
            selected.contains(&source.if_index),
            device.snmp_poll_interval_seconds,
        )
        .await?;
    }
    if let Some(enabled) = options.enable_cpu_monitor {
        sync_monitor(
            &ctx.db,
            device.id,
            CPU_MONITOR_NAME,
            enabled,
            monitor_configuration(&config, "cpu_usage"),
            true,
            device.snmp_poll_interval_seconds,
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
            device.snmp_poll_interval_seconds,
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

/// Liga ou desliga o monitoramento de **uma** interface já persistida.
///
/// São duas escritas que precisam andar juntas: o `admin_status` — a escolha do
/// operador, que o poll respeita (matriz de paridade #20) e o listador usa para
/// decidir o que aparece nas métricas — e o monitor `Interface X`, que é quem
/// faz o agendador coletar a porta. Separá-las era como as duas telas
/// divergiam: a interface aparecia marcada sem monitor por trás.
async fn set_monitoring(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    config: &SnmpConfig,
    interface: &device_interfaces::Model,
    enabled: bool,
    interval_seconds: i32,
) -> AppResult<()> {
    device_interfaces::ActiveModel {
        id: Set(interface.id),
        admin_status: Set(Some(if enabled { "up" } else { "down" }.into())),
        ..Default::default()
    }
    .update(db)
    .await?;
    sync_monitor(
        db,
        device_id,
        &interface_monitor_name(&interface.name),
        enabled,
        serde_json::json!({
            "host": config.host.clone(),
            "version": version_name(config.version),
            "community": config.community.clone(),
            "port": config.port,
            "ifIndex": interface.snmp_index,
            "ifName": interface.name,
            "metric": "traffic",
        }),
        interface.oper_status.as_deref() == Some("up"),
        interval_seconds,
    )
    .await?;
    if !enabled {
        metrics::Entity::delete_many()
            .filter(metrics_entity::Column::InterfaceId.eq(Some(interface.id)))
            .exec(db)
            .await?;
    }
    Ok(())
}

/// Inclui ou remove uma interface do monitoramento sem refazer a descoberta.
///
/// A tela de descoberta continua existindo para achar portas novas; este
/// caminho é para quem já as tem listadas e só quer ligar mais uma — refazer a
/// varredura inteira (e reenviar a seleção completa) só para isso é o que
/// tornava a operação penosa.
pub async fn set_interface_monitoring(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
    interface_id: i64,
    enabled: bool,
) -> AppResult<()> {
    let interface = device_interfaces::Entity::find_by_id(interface_id)
        .one(&ctx.db)
        .await?
        .filter(|row| row.device_id == device.id)
        .ok_or_else(|| AppError::not_found("Interface não encontrada neste dispositivo"))?;
    if interface.snmp_index.is_none() {
        return Err(AppError::validation(
            "Interface sem índice SNMP: execute uma coleta antes de monitorá-la",
        ));
    }
    set_monitoring(
        &ctx.db,
        device.id,
        &config,
        &interface,
        enabled,
        device.snmp_poll_interval_seconds,
    )
    .await?;
    // Sem uma coleta agora o gráfico abriria vazio até o próximo ciclo do
    // agendador, e o operador leria isso como falha. Desligar não tem o que
    // coletar. Falha de coleta não invalida a escolha já gravada.
    if enabled {
        let _ = poll_device(ctx, device, config).await;
    }
    Ok(())
}

/// Uma interface como a aba "Interfaces SNMP" a lê: o registro local mais o que
/// só o banco sabe — se existe monitor habilitado para ela.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInterfaceView {
    pub id: i64,
    pub device_id: i64,
    pub snmp_index: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub alias: Option<String>,
    pub mac_address: Option<String>,
    pub speed: Option<i64>,
    pub admin_status: Option<String>,
    pub oper_status: Option<String>,
    pub is_monitored: bool,
}

/// Interfaces já conhecidas do equipamento.
///
/// Lê do banco de propósito: a listagem é carregada a cada abertura da tela de
/// detalhe, e resolvê-la com uma varredura SNMP ao vivo — como era antes —
/// custava um `walk` inteiro por visita e ainda devolvia registros sem `id`,
/// sem o qual nada consegue ligar a interface às suas métricas.
pub async fn list_interfaces(
    ctx: &loco_rs::app::AppContext,
    device_id: i64,
) -> AppResult<Vec<DeviceInterfaceView>> {
    let interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(device_id))
        .order_by_asc(device_interfaces_entity::Column::SnmpIndex)
        .all(&ctx.db)
        .await?;
    let monitored = monitors::Entity::find()
        .filter(monitors_entity::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors_entity::Column::Enabled.eq(true))
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|monitor| monitor.name)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(interfaces
        .into_iter()
        .map(|row| DeviceInterfaceView {
            is_monitored: monitored.contains(&interface_monitor_name(&row.name)),
            id: row.id,
            device_id: row.device_id,
            snmp_index: row.snmp_index,
            name: row.name,
            description: row.description,
            alias: row.alias,
            mac_address: row.mac_address,
            speed: row.speed,
            admin_status: row.admin_status,
            oper_status: row.oper_status,
        })
        .collect())
}

fn monitor_configuration(config: &SnmpConfig, metric: &str) -> serde_json::Value {
    serde_json::json!({
        "host": config.host,
        "version": version_name(config.version),
        "community": config.community,
        "port": config.port,
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
    interval_seconds: i32,
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
            interval_seconds: Set(interval_seconds),
            timeout_seconds: Set(calculate_smart_timeout_seconds("snmp", interval_seconds)
                .min((interval_seconds - 1).max(1))
                .max(1)),
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
            interval_seconds: Set(interval_seconds),
            timeout_seconds: Set(calculate_smart_timeout_seconds("snmp", interval_seconds)
                .min((interval_seconds - 1).max(1))
                .max(1)),
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
        ("memory_usage", scan.memory_info.used_percent, "percent"),
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
    let started = std::time::Instant::now();
    let mut candidates = Vec::new();
    if let Some(mut config) = preferred {
        config.host = host.to_string();
        config.port = port;
        candidates.push(config);
    }
    let timeout_ms = std::env::var("SNMP_DISCOVERY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(800)
        .clamp(100, 5_000);
    let communities =
        discovery_communities(std::env::var("SNMP_DISCOVERY_COMMUNITIES").ok().as_deref());
    for (version, community) in
        [SnmpVersion::V2c, SnmpVersion::V1]
            .into_iter()
            .flat_map(|version| {
                communities
                    .iter()
                    .map(move |community| (version, community))
            })
    {
        if !candidates.iter().any(|config| {
            config.version == version && config.community.as_str() == community.as_str()
        }) {
            let mut config = SnmpConfig::v2c(host, community, port);
            config.version = version;
            config.timeout_ms = timeout_ms;
            candidates.push(config);
        }
    }
    candidates.extend(discovery_v3_profiles(host, port, timeout_ms));
    let candidate_count = candidates.len();
    let attempts = candidates.into_iter().map(|config| async move {
        let label = (
            config.version,
            (config.version != SnmpVersion::V3).then(|| config.community.clone()),
        );
        test_connection(config).await.map(|result| (label, result))
    });
    let outcomes = future::join_all(attempts).await;
    // A ordem das tentativas é a de preferência, mas quem vence é quem
    // respondeu: pegar o primeiro `Ok` devolvia "não detectado" quando a
    // combinação preferida falava com o agente sem entregar OID nenhum, mesmo
    // havendo outra combinação logo atrás que funcionava.
    let mut answered = None;
    let mut fallback = None;
    for outcome in outcomes.into_iter().flatten() {
        if outcome.1.success {
            answered = Some(outcome);
            break;
        }
        fallback.get_or_insert(outcome);
    }
    let detected = answered.or(fallback);
    tracing::info!(
        target = host,
        port,
        attempts = candidate_count,
        success = detected.as_ref().is_some_and(|outcome| outcome.1.success),
        duration_ms = started.elapsed().as_millis(),
        "detecção SNMP concluída"
    );
    match detected {
        Some(((version, community), result)) => Ok(SnmpDetectResult {
            detected: result.success,
            version: Some(version_label(version).into()),
            community,
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

fn discovery_communities(value: Option<&str>) -> Vec<String> {
    let mut communities = value
        .unwrap_or("public")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    communities.sort();
    communities.dedup();
    communities
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryV3Profile {
    username: String,
    auth_protocol: Option<String>,
    auth_key: Option<String>,
    priv_protocol: Option<String>,
    priv_key: Option<String>,
}

fn discovery_v3_profiles(host: &str, port: u16, timeout_ms: u64) -> Vec<SnmpConfig> {
    let Some(raw) = std::env::var("SNMP_DISCOVERY_V3_PROFILES")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Vec::new();
    };
    let profiles = match serde_json::from_str::<Vec<DiscoveryV3Profile>>(&raw) {
        Ok(profiles) => profiles,
        Err(error) => {
            tracing::warn!(%error, "SNMP_DISCOVERY_V3_PROFILES inválido; perfis ignorados");
            return Vec::new();
        }
    };
    profiles
        .into_iter()
        .filter(|profile| !profile.username.trim().is_empty())
        .map(|profile| SnmpConfig {
            host: host.to_string(),
            version: SnmpVersion::V3,
            community: String::new(),
            username: Some(profile.username),
            auth_protocol: profile.auth_protocol,
            auth_key: profile.auth_key,
            priv_protocol: profile.priv_protocol,
            priv_key: profile.priv_key,
            port,
            timeout_ms,
        })
        .collect()
}
const fn version_label(version: SnmpVersion) -> &'static str {
    match version {
        SnmpVersion::V1 => "v1",
        SnmpVersion::V2c => "v2c",
        SnmpVersion::V3 => "v3",
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
            collector_errors: Default::default(),
            has_cpu_monitor: true,
            has_memory_monitor: false,
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
        ] {
            assert!(json.get(campo).is_some(), "campo ausente: {campo}");
        }
        assert_eq!(json["interfaces"][0]["isMonitored"], true);
        assert_eq!(json["hasCpuMonitor"], true);
    }

    #[test]
    fn resultado_consolidado_respeita_o_estado_da_interface() {
        assert_eq!(interface_monitor_status(2, 2), MonitorStatus::Disabled);
        assert_eq!(interface_monitor_status(1, 1), MonitorStatus::Up);
        assert_eq!(interface_monitor_status(1, 2), MonitorStatus::Down);
        assert_eq!(interface_monitor_status(1, 7), MonitorStatus::Warning);
    }
}
