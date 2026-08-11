//! Casos de uso SNMP para controllers, scheduler e checker.

use chrono::Utc;
use futures::future;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::services::{
    shared::errors::{AppError, AppResult},
    snmp::{
        client::{SnmpClient, SnmpConfig, SnmpError, SnmpVersion},
        collectors::{
            collect_cpu, collect_interfaces, collect_lldp, collect_memory, collect_system,
            collect_traffic, InterfaceTraffic, LldpNeighbor, SnmpCpuInfo, SnmpInterface,
            SnmpMemoryInfo, SnmpSystemInfo,
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
    pub system: SnmpSystemInfo,
    pub interfaces: Vec<SnmpInterface>,
    pub traffic: Vec<InterfaceTraffic>,
    pub cpu: SnmpCpuInfo,
    pub memory: SnmpMemoryInfo,
    pub neighbors: Vec<LldpNeighbor>,
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
    let success =
        system.sys_descr.is_some() || system.sys_name.is_some() || system.sys_up_time.is_some();
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
        snmp_responded: system.sys_descr.is_some()
            || system.sys_name.is_some()
            || system.sys_up_time.is_some(),
        system,
        interfaces: interfaces.unwrap_or_default(),
        traffic: traffic.unwrap_or_default(),
        cpu: cpu.unwrap_or_default(),
        memory: memory.unwrap_or_default(),
        neighbors: neighbors.unwrap_or_default(),
    })
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
        interfaces.insert(interface.if_index, saved);
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
            scan.system.sys_up_time,
        )
        .await?;
        metrics_recorded += recorded;
        reboot_detected |= reboot;
    }
    persist_system_metrics(&ctx.db, device.id, &scan).await?;
    crate::services::zabbix::collector::sync_zabbix_template_monitor(&ctx.db, device).await?;
    metrics_recorded +=
        crate::services::zabbix::collector::collect(&ctx.db, device, &client).await? as usize;
    devices::ActiveModel {
        id: Set(device.id),
        status: Set("online".into()),
        last_seen_at: Set(Some(Utc::now().into())),
        ..Default::default()
    }
    .update(&ctx.db)
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
        let interface = sync_interface(&ctx.db, device.id, source).await?;
        let enabled = selected.contains(&source.if_index);
        device_interfaces::ActiveModel {
            id: Set(interface.id),
            admin_status: Set(Some(if enabled { "up" } else { "down" }.into())),
            ..Default::default()
        }
        .update(&ctx.db)
        .await?;
        let name = format!("Interface {}", source.if_name);
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
            "Monitor de Uso de CPU",
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
            "Monitor de Uso de Memoria",
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

async fn sync_interface(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    source: &SnmpInterface,
) -> AppResult<device_interfaces::Model> {
    let existing = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(device_id))
        .filter(device_interfaces_entity::Column::SnmpIndex.eq(Some(source.if_index)))
        .one(db)
        .await?;
    let now = Utc::now();
    let admin_status = existing.as_ref().and_then(|row| row.admin_status.clone());
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
        // Uma escolha manual do usuÃ¡rio prevalece sobre o valor observado no poll.
        admin_status: Set(admin_status.or_else(|| source.if_admin_status.map(status_label))),
        oper_status: Set(source.if_oper_status.map(status_label)),
        last_seen_at: Set(Some(now.into())),
        ..Default::default()
    };
    Ok(if existing.is_some() {
        model.update(db).await?
    } else {
        model.insert(db).await?
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
        ("cpu_usage", scan.cpu.usage_percent, "percent"),
        ("memory_used", scan.memory.used_percent, "percent"),
        (
            "snmp_uptime",
            scan.system.sys_up_time.map(|uptime| uptime as f64),
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

fn status_label(value: u64) -> String {
    match value {
        1 => "up",
        2 => "down",
        3 => "testing",
        4 => "unknown",
        5 => "dormant",
        6 => "notPresent",
        7 => "lowerLayerDown",
        _ => "unknown",
    }
    .into()
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
