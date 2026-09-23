//! Monitor de recursos do host: CPU, memória e disco contra limites.
//!
//! Lê `/proc` do lugar onde roda — a central ou o agente do servidor remoto.
//! Passar do limite é `warning`, não `down`: o host responde, só está
//! pressionado, e as regras de alerta decidem o que fazer com isso.

use std::time::Duration;

use chrono::Utc;
use serde::Deserialize;

use crate::services::{
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
    telemetry::{host_metrics::HostMetricsReader, rollup::HostSample},
};

/// Intervalo entre as duas leituras de `/proc/stat` que dão a CPU.
const CPU_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostResourcesConfig {
    #[serde(default = "default_limit")]
    pub cpu_max_percent: f64,
    #[serde(default = "default_limit")]
    pub memory_max_percent: f64,
    #[serde(default = "default_limit")]
    pub disk_max_percent: f64,
}

const fn default_limit() -> f64 {
    90.0
}

fn percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        let value = used as f64 / total as f64 * 100.0;
        value
    }
}

/// Estado e métricas a partir de uma amostra.
#[must_use]
pub fn evaluate(
    sample: &HostSample,
    config: &HostResourcesConfig,
) -> (MonitorStatus, String, Vec<CheckMetric>) {
    let memory = percent(sample.memory_used_bytes, sample.memory_total_bytes);
    let fullest = sample
        .disks
        .iter()
        .map(|disk| {
            (
                disk.mount.as_str(),
                percent(disk.used_bytes, disk.total_bytes),
            )
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let disk = fullest.map_or(0.0, |(_, value)| value);

    let mut exceeded = Vec::new();
    if sample.cpu_percent > config.cpu_max_percent {
        exceeded.push(format!("CPU {:.0}%", sample.cpu_percent));
    }
    if memory > config.memory_max_percent {
        exceeded.push(format!("memória {memory:.0}%"));
    }
    if let Some((mount, value)) = fullest.filter(|(_, value)| *value > config.disk_max_percent) {
        exceeded.push(format!("disco {mount} {value:.0}%"));
    }
    let metrics = vec![
        CheckMetric {
            name: "cpu".into(),
            value: sample.cpu_percent,
            unit: "%".into(),
        },
        CheckMetric {
            name: "memory".into(),
            value: memory,
            unit: "%".into(),
        },
        CheckMetric {
            name: "disk_max".into(),
            value: disk,
            unit: "%".into(),
        },
        CheckMetric {
            name: "load1".into(),
            value: sample.load1,
            unit: "load".into(),
        },
    ];
    if exceeded.is_empty() {
        (
            MonitorStatus::Up,
            format!(
                "CPU {:.0}%, memória {memory:.0}%, disco {disk:.0}%",
                sample.cpu_percent
            ),
            metrics,
        )
    } else {
        (
            MonitorStatus::Warning,
            format!("Acima do limite: {}", exceeded.join(", ")),
            metrics,
        )
    }
}

pub struct HostResourcesChecker;

#[async_trait::async_trait]
impl Checker for HostResourcesChecker {
    type Config = HostResourcesConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let mut reader = HostMetricsReader::from_env();
        // A primeira leitura só serve de base para a CPU.
        let _ = reader.sample();
        tokio::time::sleep(CPU_WINDOW).await;
        let (status, message, metrics) = match reader.sample() {
            Some(sample) => evaluate(&sample, &config),
            None => (
                MonitorStatus::Unknown,
                "Métricas do host indisponíveis (/proc não legível neste sistema)".to_string(),
                Vec::new(),
            ),
        };
        let finished_at = Utc::now();
        CheckResult {
            success: status == MonitorStatus::Up,
            status,
            started_at,
            finished_at,
            duration_ms: (finished_at - started_at).num_milliseconds().max(0),
            message: Some(message),
            metrics,
            data: serde_json::json!({}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::telemetry::rollup::DiskUsage;

    fn sample(cpu: f64, memory_used: u64, disk_used: u64) -> HostSample {
        HostSample {
            cpu_percent: cpu,
            memory_used_bytes: memory_used,
            memory_total_bytes: 100,
            load1: 0.3,
            net_rx_bps: 0.0,
            net_tx_bps: 0.0,
            disks: vec![DiskUsage {
                mount: "/".into(),
                used_bytes: disk_used,
                total_bytes: 100,
            }],
        }
    }

    fn limits() -> HostResourcesConfig {
        serde_json::from_value(serde_json::json!({ "cpuMaxPercent": 80 })).expect("config")
    }

    #[test]
    fn dentro_dos_limites_e_up_e_acima_e_warning() {
        let (status, _, metrics) = evaluate(&sample(50.0, 40, 30), &limits());
        assert_eq!(status, MonitorStatus::Up);
        assert!((metrics[1].value - 40.0).abs() < f64::EPSILON);

        let (status, message, _) = evaluate(&sample(85.0, 95, 99), &limits());
        assert_eq!(status, MonitorStatus::Warning);
        assert!(message.contains("CPU 85%"));
        assert!(message.contains("memória 95%"));
        assert!(message.contains("disco / 99%"));
    }
}
