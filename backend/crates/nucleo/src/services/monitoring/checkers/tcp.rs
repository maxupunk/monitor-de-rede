//! Checker TCP sem processos filhos e com timeout explícito.

use std::time::Duration;

use crate::services::{
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
    network_tools::tcp_probe::{probe_tcp, TcpProbeState},
};
use chrono::Utc;
use serde::Deserialize;

/// Configuração de conexão TCP.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpConfig {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

const fn default_timeout_ms() -> u64 {
    5_000
}

/// Implementação nativa do teste de porta TCP.
pub struct TcpChecker;

#[async_trait::async_trait]
impl Checker for TcpChecker {
    type Config = TcpConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let target = format!("{}:{}", config.host, config.port);
        let observation = probe_tcp(
            (&*config.host, config.port),
            Duration::from_millis(config.timeout_ms.max(1)),
        )
        .await;
        let finished_at = Utc::now();
        let duration_ms = (finished_at - started_at).num_milliseconds().max(0);
        let (success, status, message) = match observation.state {
            TcpProbeState::Open => (
                true,
                MonitorStatus::Up,
                format!("Conexão TCP para {target} estabelecida em {duration_ms}ms"),
            ),
            TcpProbeState::Filtered => (
                false,
                MonitorStatus::Down,
                format!(
                    "Timeout na conexão TCP para {target} ({}ms)",
                    config.timeout_ms
                ),
            ),
            TcpProbeState::Closed | TcpProbeState::Unreachable | TcpProbeState::Error => (
                false,
                MonitorStatus::Down,
                format!(
                    "Erro na conexão TCP para {target}: {}",
                    observation
                        .error
                        .as_deref()
                        .unwrap_or(observation.state.as_str())
                ),
            ),
        };
        CheckResult {
            success,
            status,
            started_at,
            finished_at,
            duration_ms,
            message: Some(message),
            metrics: vec![CheckMetric {
                name: "connect_time".into(),
                value: duration_ms as f64,
                unit: "ms".into(),
            }],
            data: serde_json::json!({}),
        }
    }
}
