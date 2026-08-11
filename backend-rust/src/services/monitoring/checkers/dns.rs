//! Checker DNS baseado no mesmo serviço wire-format do benchmark.

use chrono::Utc;
use serde::Deserialize;

use crate::services::{
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
    network_tools::dns::{
        latency::{measure_dns_lookup, DnsLookupOptions, DnsProtocol, DEFAULT_DNS_TIMEOUT_MS},
        wire,
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsConfig {
    #[serde(alias = "hostname")]
    pub domain: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub record_type: Option<String>,
    #[serde(default)]
    pub dns_server: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub doh_url: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub warning_threshold_ms: Option<f64>,
}
const fn default_timeout() -> u64 {
    DEFAULT_DNS_TIMEOUT_MS
}

pub struct DnsChecker;
#[async_trait::async_trait]
impl Checker for DnsChecker {
    type Config = DnsConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let record_type = match wire::parse_record_type(config.record_type.as_deref()) {
            Ok(value) => value,
            Err(error) => return failed(started_at, error.to_string()),
        };
        let protocol = match DnsProtocol::parse(config.protocol.as_deref()) {
            Ok(value) => value,
            Err(error) => return failed(started_at, error.to_string()),
        };
        let mut domains = vec![config.domain];
        domains.extend(
            config
                .domains
                .into_iter()
                .filter(|domain| !domain.trim().is_empty()),
        );
        domains.truncate(10);
        let mut samples = Vec::with_capacity(domains.len());
        // Série intencional: taxa de sucesso e tempo médio pertencem à mesma
        // janela, sem a concorrência local distorcer o monitor.
        for hostname in domains {
            samples.push(
                measure_dns_lookup(DnsLookupOptions {
                    hostname,
                    record_type,
                    server: config.dns_server.clone(),
                    protocol,
                    doh_url: config.doh_url.clone(),
                    timeout_ms: config.timeout_ms,
                })
                .await,
            );
        }
        let successful: Vec<f64> = samples
            .iter()
            .filter_map(|sample| sample.success.then_some(sample.lookup_time_ms).flatten())
            .collect();
        let total = samples.len().max(1);
        let rate = successful.len() as f64 * 100.0 / total as f64;
        let average = (!successful.is_empty())
            .then(|| successful.iter().sum::<f64>() / successful.len() as f64)
            .unwrap_or(0.0);
        let min = successful.iter().copied().reduce(f64::min).unwrap_or(0.0);
        let max = successful.iter().copied().reduce(f64::max).unwrap_or(0.0);
        let threshold = config.warning_threshold_ms.unwrap_or(f64::INFINITY);
        let status = if successful.is_empty() {
            MonitorStatus::Down
        } else if successful.len() < total || average > threshold {
            MonitorStatus::Warning
        } else {
            MonitorStatus::Up
        };
        let finished_at = Utc::now();
        let server = samples
            .first()
            .map(|sample| sample.server.clone())
            .unwrap_or_default();
        CheckResult {
            success: status != MonitorStatus::Down,
            status,
            started_at,
            finished_at,
            duration_ms: (finished_at - started_at).num_milliseconds().max(0),
            message: Some(format!(
                "DNS: {}/{} consultas responderam",
                successful.len(),
                total
            )),
            metrics: vec![
                CheckMetric {
                    name: "dns_lookup_time".into(),
                    value: average,
                    unit: "ms".into(),
                },
                CheckMetric {
                    name: "resolution_time".into(),
                    value: average,
                    unit: "ms".into(),
                },
                CheckMetric {
                    name: "dns_lookup_time_min".into(),
                    value: min,
                    unit: "ms".into(),
                },
                CheckMetric {
                    name: "dns_lookup_time_max".into(),
                    value: max,
                    unit: "ms".into(),
                },
                CheckMetric {
                    name: "dns_success_rate".into(),
                    value: rate,
                    unit: "%".into(),
                },
            ],
            data: serde_json::json!({ "server": server, "protocol": protocol.as_str(), "avgLookupTimeMs": average, "samples": samples }),
        }
    }
}

fn failed(started_at: chrono::DateTime<Utc>, error: String) -> CheckResult {
    let finished_at = Utc::now();
    CheckResult {
        success: false,
        status: MonitorStatus::Down,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message: Some(format!("Falha na consulta DNS: {error}")),
        metrics: vec![],
        data: serde_json::json!({}),
    }
}
