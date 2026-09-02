//! Guarda adaptativa para alertas de latência em alvos externos.
//!
//! Uma latência absoluta não significa a mesma coisa em Fortaleza, São Paulo ou
//! numa filial atendida por satélite. Este módulo decide se uma leitura externa
//! é uma degradação acionável usando três evidências que já existem no sistema:
//!
//! 1. desvio em relação à baseline histórica do próprio monitor;
//! 2. repetição em checagens consecutivas, reconstruída de `monitor_results`;
//! 3. utilização da interface WAN/Uplink no instante de cada checagem.
//!
//! A ausência de telemetria de banda nunca é interpretada como saturação. Nesse
//! caso a decisão continua pela latência, evitando esconder falhas por falta de
//! SNMP ou por uma configuração incompleta.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    models::{
        _entities::{metrics as metrics_entity, monitor_results as monitor_results_entity},
        device_interfaces, devices, metrics, monitor_results, monitors, probes,
    },
    services::{
        alerts::{baseline::MonitorBaseline, fields},
        monitoring::link_speed::normalize_speed,
        shared::errors::AppResult,
    },
};

pub const DEFAULT_DEVIATION_PERCENT: f64 = 50.0;
pub const DEFAULT_CONSECUTIVE_CHECKS: usize = 3;
pub const DEFAULT_MIN_INCREASE_MS: f64 = 20.0;
pub const DEFAULT_SATURATION_THRESHOLD_PERCENT: f64 = 80.0;
const MAX_CONSECUTIVE_CHECKS: usize = 20;
const MIN_TELEMETRY_FRESHNESS_SECONDS: i64 = 180;
const MAX_TELEMETRY_FRESHNESS_SECONDS: i64 = 900;
const MAX_RESULT_GAP_INTERVALS: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Auto,
    Adaptive,
    Fixed,
}

#[derive(Debug, Clone, Copy)]
struct Policy {
    mode: Mode,
    deviation_percent: f64,
    consecutive_checks: usize,
    min_increase_ms: f64,
    suppress_on_saturation: bool,
    saturation_threshold_percent: f64,
    source_device_id: Option<i64>,
    download_capacity_bps: Option<f64>,
    upload_capacity_bps: Option<f64>,
}

impl Policy {
    fn from_monitor(monitor: &monitors::Model) -> Self {
        let raw = monitor
            .configuration
            .get("latencyAlertPolicy")
            .and_then(Value::as_object);
        let number = |key: &str| raw.and_then(|value| value.get(key)).and_then(Value::as_f64);
        let integer = |key: &str| {
            raw.and_then(|value| value.get(key))
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
        };
        let positive = |key: &str| number(key).filter(|value| value.is_finite() && *value > 0.0);
        let mode = match raw
            .and_then(|value| value.get("mode"))
            .and_then(Value::as_str)
        {
            Some("adaptive") => Mode::Adaptive,
            Some("fixed") => Mode::Fixed,
            _ => Mode::Auto,
        };
        Self {
            mode,
            deviation_percent: positive("deviationPercent")
                .unwrap_or(DEFAULT_DEVIATION_PERCENT)
                .clamp(5.0, 500.0),
            consecutive_checks: integer("consecutiveChecks")
                .unwrap_or(DEFAULT_CONSECUTIVE_CHECKS)
                .clamp(2, MAX_CONSECUTIVE_CHECKS),
            min_increase_ms: positive("minIncreaseMs")
                .unwrap_or(DEFAULT_MIN_INCREASE_MS)
                .clamp(1.0, 10_000.0),
            suppress_on_saturation: raw
                .and_then(|value| value.get("suppressOnSaturation"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            saturation_threshold_percent: positive("saturationThresholdPercent")
                .unwrap_or(DEFAULT_SATURATION_THRESHOLD_PERCENT)
                .clamp(50.0, 100.0),
            source_device_id: raw
                .and_then(|value| value.get("sourceDeviceId"))
                .and_then(Value::as_i64),
            download_capacity_bps: positive("downloadCapacityBps"),
            upload_capacity_bps: positive("uploadCapacityBps"),
        }
    }
}

/// Diagnóstico completo da guarda. Além de governar o motor, é devolvido pelo
/// endpoint de baseline para a tela explicar por que uma notificação foi ou não
/// liberada.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub applies: bool,
    pub alert_eligible: bool,
    pub reason: &'static str,
    pub deviation_percent: f64,
    pub required_consecutive_checks: usize,
    pub observed_consecutive_checks: usize,
    pub expected_latency_ms: Option<f64>,
    pub alert_threshold_ms: Option<f64>,
    pub current_latency_ms: Option<f64>,
    pub link_utilization_percent: Option<f64>,
    pub link_saturated: bool,
    pub source_device_id: Option<i64>,
    pub link_interface_id: Option<i64>,
    pub link_interface_name: Option<String>,
    pub capacity_source: Option<&'static str>,
}

impl Assessment {
    #[must_use]
    pub fn bypass(reason: &'static str, current_latency_ms: Option<f64>) -> Self {
        Self {
            applies: false,
            alert_eligible: true,
            reason,
            deviation_percent: DEFAULT_DEVIATION_PERCENT,
            required_consecutive_checks: DEFAULT_CONSECUTIVE_CHECKS,
            observed_consecutive_checks: 0,
            expected_latency_ms: None,
            alert_threshold_ms: None,
            current_latency_ms,
            link_utilization_percent: None,
            link_saturated: false,
            source_device_id: None,
            link_interface_id: None,
            link_interface_name: None,
            capacity_source: None,
        }
    }

    #[must_use]
    pub fn fail_open(current_latency_ms: Option<f64>) -> Self {
        let mut assessment = Self::bypass("evaluation_unavailable", current_latency_ms);
        assessment.applies = true;
        assessment
    }
}

#[derive(Debug, Clone)]
struct Uplink {
    device: devices::Model,
    interface: device_interfaces::Model,
}

#[derive(Debug, Clone, Copy, Default)]
struct TrafficAt {
    in_bps: Option<f64>,
    out_bps: Option<f64>,
}

/// Remove somente fatos de latência acionáveis. Disponibilidade, código HTTP,
/// perda e os campos históricos continuam no dataset e seguem alertáveis.
pub fn suppress_latency_facts(dataset: &mut Map<String, Value>) {
    for field in LATENCY_FACTS {
        dataset.remove(field);
    }
}

const LATENCY_FACTS: [&str; 5] = [
    fields::LATENCY_MS,
    fields::LATENCY_DEVIATION_PERCENT,
    fields::LATENCY_Z_SCORE,
    fields::CONNECT_TIME_MS,
    fields::RESOLUTION_TIME_MS,
];

#[must_use]
pub fn is_latency_field(field: &str) -> bool {
    LATENCY_FACTS.contains(&field)
}

/// Avalia a leitura mais recente já persistida por `process_result`.
pub async fn assess(
    db: &DatabaseConnection,
    monitor: &monitors::Model,
    baseline: &MonitorBaseline,
    current_latency_ms: Option<f64>,
    observed_at: DateTime<Utc>,
) -> AppResult<Assessment> {
    let policy = Policy::from_monitor(monitor);
    let applies = match policy.mode {
        Mode::Fixed => false,
        Mode::Adaptive => true,
        Mode::Auto => is_likely_external(monitor),
    };
    if !applies {
        return Ok(Assessment::bypass("fixed_or_internal", current_latency_ms));
    }

    let mut output = Assessment {
        applies: true,
        alert_eligible: false,
        reason: "learning_baseline",
        deviation_percent: policy.deviation_percent,
        required_consecutive_checks: policy.consecutive_checks,
        observed_consecutive_checks: 0,
        expected_latency_ms: baseline.latency_baseline_ms,
        alert_threshold_ms: None,
        current_latency_ms,
        link_utilization_percent: None,
        link_saturated: false,
        source_device_id: None,
        link_interface_id: None,
        link_interface_name: None,
        capacity_source: None,
    };

    let Some(expected) = baseline
        .latency_baseline_ms
        .filter(|value| value.is_finite() && *value >= 0.0)
    else {
        return Ok(output);
    };
    let relative = expected * (1.0 + policy.deviation_percent / 100.0);
    let absolute = expected + policy.min_increase_ms;
    let statistical = baseline.latency_upper_band_ms.unwrap_or(expected);
    let threshold = relative.max(absolute).max(statistical);
    output.alert_threshold_ms = Some(threshold);

    let Some(current) = current_latency_ms.filter(|value| value.is_finite() && *value >= 0.0)
    else {
        output.reason = "latency_unavailable";
        return Ok(output);
    };
    if current <= threshold {
        output.reason = "within_expected_range";
        return Ok(output);
    }

    let history = monitor_results::Entity::find()
        .filter(monitor_results_entity::Column::MonitorId.eq(monitor.id))
        .filter(monitor_results_entity::Column::StartedAt.lte(observed_at))
        .order_by_desc(monitor_results_entity::Column::StartedAt)
        .order_by_desc(monitor_results_entity::Column::Id)
        .limit(policy.consecutive_checks as u64)
        .all(db)
        .await?;

    let uplink = resolve_uplink(db, monitor, policy.source_device_id).await?;
    if let Some(uplink) = &uplink {
        output.source_device_id = Some(uplink.device.id);
        output.link_interface_id = Some(uplink.interface.id);
        output.link_interface_name = Some(uplink.interface.name.clone());
    }

    let speed = uplink
        .as_ref()
        .and_then(|uplink| normalize_speed(uplink.interface.speed))
        .map(|value| value as f64);
    let download_capacity = policy.download_capacity_bps.or(speed);
    let upload_capacity = policy.upload_capacity_bps.or(speed);
    output.capacity_source =
        if policy.download_capacity_bps.is_some() || policy.upload_capacity_bps.is_some() {
            Some("configured")
        } else if speed.is_some() {
            Some("negotiated_speed")
        } else {
            None
        };

    let traffic_rows = match (&uplink, history.last()) {
        (Some(uplink), Some(oldest))
            if download_capacity.is_some() || upload_capacity.is_some() =>
        {
            let freshness = telemetry_freshness_seconds(&uplink.device);
            metrics::Entity::find()
                .filter(metrics_entity::Column::InterfaceId.eq(Some(uplink.interface.id)))
                .filter(metrics_entity::Column::Name.is_in(["inBps", "outBps"]))
                .filter(
                    metrics_entity::Column::RecordedAt
                        .gte(oldest.started_at.with_timezone(&Utc) - Duration::seconds(freshness)),
                )
                .filter(metrics_entity::Column::RecordedAt.lte(observed_at))
                .order_by_asc(metrics_entity::Column::RecordedAt)
                .all(db)
                .await?
        }
        _ => Vec::new(),
    };

    let mut newer_at = observed_at;
    let max_gap =
        Duration::seconds(i64::from(monitor.interval_seconds.max(1)) * MAX_RESULT_GAP_INTERVALS);
    for row in &history {
        let row_at = row.started_at.with_timezone(&Utc);
        if newer_at - row_at > max_gap {
            break;
        }
        let Some(_) = row
            .latency_ms
            .filter(|value| value.is_finite() && *value > threshold)
        else {
            break;
        };

        let utilization = uplink.as_ref().and_then(|uplink| {
            utilization_at(
                &traffic_rows,
                row_at,
                telemetry_freshness_seconds(&uplink.device),
                download_capacity,
                upload_capacity,
            )
        });
        let saturated = utilization.is_some_and(|value| {
            value >= policy.saturation_threshold_percent && policy.suppress_on_saturation
        });
        if row.id == history.first().map_or(row.id, |first| first.id) {
            output.link_utilization_percent = utilization;
            output.link_saturated = saturated;
        }
        if saturated {
            output.reason = "link_saturated";
            break;
        }
        output.observed_consecutive_checks += 1;
        newer_at = row_at;
    }

    if output.link_saturated {
        return Ok(output);
    }
    if output.observed_consecutive_checks < policy.consecutive_checks {
        output.reason = "collecting_confirmations";
        return Ok(output);
    }
    output.alert_eligible = true;
    output.reason = "alert_ready";
    Ok(output)
}

fn telemetry_freshness_seconds(device: &devices::Model) -> i64 {
    (i64::from(device.snmp_poll_interval_seconds.max(1)) * 3).clamp(
        MIN_TELEMETRY_FRESHNESS_SECONDS,
        MAX_TELEMETRY_FRESHNESS_SECONDS,
    )
}

fn utilization_at(
    rows: &[metrics::Model],
    at: DateTime<Utc>,
    freshness_seconds: i64,
    download_capacity_bps: Option<f64>,
    upload_capacity_bps: Option<f64>,
) -> Option<f64> {
    let mut traffic = TrafficAt::default();
    let freshness = Duration::seconds(freshness_seconds);
    for row in rows {
        let recorded_at = row.recorded_at.with_timezone(&Utc);
        if recorded_at > at {
            break;
        }
        if at - recorded_at > freshness || !row.value.is_finite() || row.value < 0.0 {
            continue;
        }
        match row.name.as_str() {
            "inBps" => traffic.in_bps = Some(row.value),
            "outBps" => traffic.out_bps = Some(row.value),
            _ => {}
        }
    }
    let download = traffic
        .in_bps
        .zip(download_capacity_bps)
        .filter(|(_, capacity)| *capacity > 0.0)
        .map(|(value, capacity)| value * 100.0 / capacity);
    let upload = traffic
        .out_bps
        .zip(upload_capacity_bps)
        .filter(|(_, capacity)| *capacity > 0.0)
        .map(|(value, capacity)| value * 100.0 / capacity);
    match (download, upload) {
        (Some(download), Some(upload)) => Some(download.max(upload).clamp(0.0, 10_000.0)),
        (Some(value), None) | (None, Some(value)) => Some(value.clamp(0.0, 10_000.0)),
        (None, None) => None,
    }
}

async fn resolve_uplink(
    db: &DatabaseConnection,
    monitor: &monitors::Model,
    configured_source_device_id: Option<i64>,
) -> AppResult<Option<Uplink>> {
    if let Some(id) = configured_source_device_id {
        return uplink_for_device(db, id).await;
    }

    let associated_device = match monitor.device_id {
        Some(id) => devices::Entity::find_by_id(id).one(db).await?,
        None => None,
    };
    if let Some(device) = &associated_device {
        if device.link_interface_id.is_some() {
            return uplink_from_model(db, device.clone()).await;
        }
    }

    let site_id = if let Some(device) = &associated_device {
        device.site_id
    } else if let Some(probe_id) = monitor.probe_id {
        probes::Entity::find_by_id(probe_id)
            .one(db)
            .await?
            .and_then(|probe| probe.site_id)
    } else {
        None
    };

    let mut candidates =
        devices::Entity::find().filter(devices::Column::LinkInterfaceId.is_not_null());
    if let Some(site_id) = site_id {
        candidates = candidates.filter(devices::Column::SiteId.eq(Some(site_id)));
    }
    let candidates = candidates.all(db).await?;
    if candidates.len() != 1 {
        return Ok(None);
    }
    uplink_from_model(db, candidates.into_iter().next().expect("um candidato")).await
}

async fn uplink_for_device(db: &DatabaseConnection, device_id: i64) -> AppResult<Option<Uplink>> {
    let Some(device) = devices::Entity::find_by_id(device_id).one(db).await? else {
        return Ok(None);
    };
    uplink_from_model(db, device).await
}

async fn uplink_from_model(
    db: &DatabaseConnection,
    device: devices::Model,
) -> AppResult<Option<Uplink>> {
    let Some(interface_id) = device.link_interface_id else {
        return Ok(None);
    };
    let Some(interface) = device_interfaces::Entity::find_by_id(interface_id)
        .one(db)
        .await?
        .filter(|interface| interface.device_id == device.id)
    else {
        return Ok(None);
    };
    Ok(Some(Uplink { device, interface }))
}

fn is_likely_external(monitor: &monitors::Model) -> bool {
    if monitor.configuration.get("isSaas").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    if !matches!(
        monitor.r#type.to_ascii_lowercase().as_str(),
        "ping" | "http" | "https" | "dns" | "tcp"
    ) {
        return false;
    }
    let target = monitor.target();
    let host = reqwest::Url::parse(&target)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or(target);
    external_host(&host)
}

fn external_host(raw: &str) -> bool {
    let host = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || !host.contains('.')
        || [
            ".local",
            ".lan",
            ".internal",
            ".home.arpa",
            ".localhost",
            ".test",
            ".invalid",
        ]
        .iter()
        .any(|suffix| host.ends_with(suffix))
    {
        return false;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => external_ipv4(ip),
        Ok(IpAddr::V6(ip)) => external_ipv6(ip),
        Err(_) => true,
    }
}

fn external_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || octets[0] == 0
        || octets[0] >= 224
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 169 && octets[1] == 254)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113))
}

fn external_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xff00) == 0xff00
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn monitor(kind: &str, target_key: &str, target: &str, policy: Value) -> monitors::Model {
        let mut configuration = serde_json::Map::new();
        configuration.insert(target_key.into(), json!(target));
        if !policy.is_null() {
            configuration.insert("latencyAlertPolicy".into(), policy);
        }
        let now = Utc::now().into();
        monitors::Model {
            id: 1,
            device_id: None,
            probe_id: None,
            r#type: kind.into(),
            name: "teste".into(),
            configuration: Value::Object(configuration),
            interval_seconds: 60,
            timeout_seconds: 5,
            retry_count: 2,
            enabled: true,
            next_run_at: None,
            last_run_at: None,
            status: "up".into(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn auto_distingue_alvo_externo_de_endereco_privado() {
        assert!(is_likely_external(&monitor(
            "http",
            "url",
            "https://chatgpt.com",
            Value::Null
        )));
        assert!(is_likely_external(&monitor(
            "ping",
            "host",
            "1.1.1.1",
            Value::Null
        )));
        assert!(!is_likely_external(&monitor(
            "ping",
            "host",
            "192.168.1.1",
            Value::Null
        )));
        assert!(!is_likely_external(&monitor(
            "http",
            "url",
            "http://servidor.local",
            Value::Null
        )));
    }

    #[test]
    fn modo_explicito_vence_a_inferencia() {
        let internal = monitor("ping", "host", "192.168.1.1", json!({ "mode": "adaptive" }));
        assert_eq!(Policy::from_monitor(&internal).mode, Mode::Adaptive);
        let external = monitor("ping", "host", "1.1.1.1", json!({ "mode": "fixed" }));
        assert_eq!(Policy::from_monitor(&external).mode, Mode::Fixed);
    }

    #[test]
    fn politica_limita_valores_perigosos() {
        let item = monitor(
            "ping",
            "host",
            "1.1.1.1",
            json!({
                "deviationPercent": 1,
                "consecutiveChecks": 999,
                "minIncreaseMs": -10,
                "saturationThresholdPercent": 1
            }),
        );
        let policy = Policy::from_monitor(&item);
        assert_eq!(policy.deviation_percent, 5.0);
        assert_eq!(policy.consecutive_checks, MAX_CONSECUTIVE_CHECKS);
        assert_eq!(policy.min_increase_ms, DEFAULT_MIN_INCREASE_MS);
        assert_eq!(policy.saturation_threshold_percent, 50.0);
    }

    #[test]
    fn utilizacao_considera_o_pior_sentido_e_descarta_amostra_velha() {
        let t0 = Utc::now();
        let row = |id, name: &str, value, seconds_ago| metrics::Model {
            id,
            device_id: 1,
            interface_id: Some(2),
            monitor_id: None,
            name: name.into(),
            value,
            unit: "bps".into(),
            recorded_at: (t0 - Duration::seconds(seconds_ago)).into(),
            created_at: (t0 - Duration::seconds(seconds_ago)).into(),
        };
        let rows = vec![row(1, "inBps", 70.0, 10), row(2, "outBps", 90.0, 10)];
        assert_eq!(
            utilization_at(&rows, t0, 60, Some(100.0), Some(100.0)),
            Some(90.0)
        );
        assert_eq!(utilization_at(&rows, t0, 5, Some(100.0), Some(100.0)), None);
    }

    #[test]
    fn supressao_remove_so_fatos_de_latencia() {
        let mut dataset = Map::from_iter([
            (fields::LATENCY_MS.into(), json!(300)),
            (fields::LATENCY_Z_SCORE.into(), json!(4)),
            (fields::PACKET_LOSS.into(), json!(12)),
            (fields::STATUS_CODE.into(), json!(503)),
        ]);
        suppress_latency_facts(&mut dataset);
        assert!(!dataset.contains_key(fields::LATENCY_MS));
        assert!(!dataset.contains_key(fields::LATENCY_Z_SCORE));
        assert_eq!(dataset[fields::PACKET_LOSS], json!(12));
        assert_eq!(dataset[fields::STATUS_CODE], json!(503));
    }
}
