//! Monitor de container: está rodando? está saudável? reiniciou?
//!
//! Roda onde o monitor for executado — na central, contra o Docker local, ou
//! no agente de um servidor remoto (monitor com `probe_id` do agente). Assim
//! o motor de alertas cobre containers de qualquer host sem código novo.

use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;

use crate::services::{
    docker::{source::DockerEngine, DockerError},
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerConfig {
    /// Nome ou id do container.
    pub container: String,
    /// Exige healthcheck `healthy` (sem healthcheck vira alerta).
    #[serde(default)]
    pub require_healthy: bool,
}

pub struct ContainerChecker {
    engine: Arc<dyn DockerEngine>,
}

impl ContainerChecker {
    #[must_use]
    pub fn new(engine: Arc<dyn DockerEngine>) -> Self {
        Self { engine }
    }
}

/// Estado, mensagem e métricas a partir do `inspect` cru.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn evaluate(
    inspect: &Value,
    require_healthy: bool,
) -> (MonitorStatus, String, Vec<CheckMetric>) {
    let name = inspect
        .get("Name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_string();
    let state = inspect.get("State").cloned().unwrap_or(Value::Null);
    let status = state
        .get("Status")
        .and_then(Value::as_str)
        .unwrap_or("desconhecido");
    let running = state
        .get("Running")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let health = state
        .pointer("/Health/Status")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && *value != "none");
    let restarts = inspect
        .get("RestartCount")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let exit_code = state.get("ExitCode").and_then(Value::as_i64).unwrap_or(0);
    let metrics = vec![
        CheckMetric {
            name: "restart_count".into(),
            value: restarts as f64,
            unit: "count".into(),
        },
        CheckMetric {
            name: "running".into(),
            value: if running { 1.0 } else { 0.0 },
            unit: "bool".into(),
        },
    ];
    let verdict = match (running, health) {
        (false, _) => (
            MonitorStatus::Down,
            format!("Container {name} está {status} (código de saída {exit_code})"),
        ),
        (true, Some("unhealthy")) => (
            MonitorStatus::Down,
            format!("Container {name} está rodando, mas unhealthy"),
        ),
        (true, Some("starting")) => (
            MonitorStatus::Warning,
            format!("Container {name} ainda está iniciando o healthcheck"),
        ),
        (true, None) if require_healthy => (
            MonitorStatus::Warning,
            format!("Container {name} está rodando, mas não declara healthcheck"),
        ),
        (true, _) => (
            MonitorStatus::Up,
            format!("Container {name} rodando ({restarts} reinícios)"),
        ),
    };
    (verdict.0, verdict.1, metrics)
}

#[async_trait::async_trait]
impl Checker for ContainerChecker {
    type Config = ContainerConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let (status, message, metrics, data) = match self
            .engine
            .inspect_container_raw(config.container.trim())
            .await
        {
            Ok(inspect) => {
                let (status, message, metrics) = evaluate(&inspect, config.require_healthy);
                (
                    status,
                    message,
                    metrics,
                    serde_json::json!({ "container": config.container }),
                )
            }
            Err(DockerError::NotFound) => (
                MonitorStatus::Down,
                format!("Container {} não existe neste host", config.container),
                Vec::new(),
                serde_json::json!({ "failureKind": "not_found" }),
            ),
            Err(error) => (
                MonitorStatus::Unknown,
                format!("Docker Engine inacessível: {error}"),
                Vec::new(),
                serde_json::json!({ "failureKind": "engine_unavailable" }),
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
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn inspect(running: bool, health: Option<&str>) -> Value {
        let mut state = json!({ "Status": if running { "running" } else { "exited" }, "Running": running, "ExitCode": 137 });
        if let Some(health) = health {
            state["Health"] = json!({ "Status": health });
        }
        json!({ "Name": "/web", "RestartCount": 2, "State": state })
    }

    #[test]
    fn estado_do_container_vira_estado_do_monitor() {
        assert_eq!(evaluate(&inspect(true, None), false).0, MonitorStatus::Up);
        assert_eq!(
            evaluate(&inspect(true, Some("healthy")), true).0,
            MonitorStatus::Up
        );
        assert_eq!(
            evaluate(&inspect(true, None), true).0,
            MonitorStatus::Warning
        );
        assert_eq!(
            evaluate(&inspect(true, Some("starting")), false).0,
            MonitorStatus::Warning
        );
        assert_eq!(
            evaluate(&inspect(true, Some("unhealthy")), false).0,
            MonitorStatus::Down
        );
        let (status, message, metrics) = evaluate(&inspect(false, None), false);
        assert_eq!(status, MonitorStatus::Down);
        assert!(message.contains("137"));
        assert!((metrics[0].value - 2.0).abs() < f64::EPSILON);
    }
}
