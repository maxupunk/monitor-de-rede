//! Contratos independentes de HTTP para as verificações de monitoramento.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Estado canônico de uma execução de monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum MonitorStatus {
    Up,
    Down,
    Warning,
    Unknown,
    Disabled,
}

impl MonitorStatus {
    /// Forma persistida no banco e publicada pela API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Warning => "warning",
            Self::Unknown => "unknown",
            Self::Disabled => "disabled",
        }
    }
}

/// Uma medida numérica produzida por um checker.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckMetric {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

/// Resultado degradável de uma medição. Erro de rede é dado de domínio, não panic.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckResult {
    pub success: bool,
    pub status: MonitorStatus,
    #[ts(type = "string")]
    pub started_at: DateTime<Utc>,
    #[ts(type = "string")]
    pub finished_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub message: Option<String>,
    pub metrics: Vec<CheckMetric>,
    #[ts(type = "any")]
    pub data: serde_json::Value,
}

/// Abstração para checkers puros e testáveis fora do HTTP.
#[async_trait::async_trait]
pub trait Checker {
    type Config: Send;

    /// Executa uma medição e sempre produz uma observação útil ao operador.
    async fn execute(&self, config: Self::Config) -> CheckResult;
}
