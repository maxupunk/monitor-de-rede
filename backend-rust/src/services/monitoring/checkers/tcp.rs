//! Checker TCP sem processos filhos e com timeout explícito.

use std::time::Duration;

use chrono::Utc;
use serde::Deserialize;
use tokio::{net::TcpStream, time::timeout};

use crate::services::monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus};

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
        let outcome = timeout(
            Duration::from_millis(config.timeout_ms.max(1)),
            TcpStream::connect(&target),
        )
        .await;
        let finished_at = Utc::now();
        let duration_ms = (finished_at - started_at).num_milliseconds().max(0);
        let (success, status, message) = match outcome {
            Ok(Ok(_)) => (
                true,
                MonitorStatus::Up,
                format!("Conexão TCP para {target} estabelecida em {duration_ms}ms"),
            ),
            Err(_) => (
                false,
                MonitorStatus::Down,
                format!(
                    "Timeout na conexão TCP para {target} ({}ms)",
                    config.timeout_ms
                ),
            ),
            Ok(Err(error)) => (
                false,
                MonitorStatus::Down,
                format!("Erro na conexão TCP para {target}: {error}"),
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
