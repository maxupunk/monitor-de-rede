//! Contratos do histórico de métricas de host e containers.

use chrono::Duration;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::telemetry::rollup::DiskUsage;

/// Janelas oferecidas pela tela. Cada uma tem um passo que mantém a série
/// entre ~60 e ~360 pontos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum MetricsRange {
    #[serde(rename = "1h")]
    #[default]
    OneHour,
    #[serde(rename = "6h")]
    SixHours,
    #[serde(rename = "24h")]
    OneDay,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
}

impl MetricsRange {
    #[must_use]
    pub fn window(self) -> Duration {
        match self {
            Self::OneHour => Duration::hours(1),
            Self::SixHours => Duration::hours(6),
            Self::OneDay => Duration::hours(24),
            Self::SevenDays => Duration::days(7),
            Self::ThirtyDays => Duration::days(30),
        }
    }

    /// Minutos agregados por ponto.
    #[must_use]
    pub const fn step_minutes(self) -> usize {
        match self {
            Self::OneHour => 1,
            Self::SixHours => 2,
            Self::OneDay => 5,
            Self::SevenDays => 30,
            Self::ThirtyDays => 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct HostHistoryPoint {
    pub at: String,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_used: f64,
    #[ts(type = "number")]
    pub memory_total: i64,
    pub load1: f64,
    pub net_rx_bps: f64,
    pub net_tx_bps: f64,
    pub disks: Vec<DiskUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ContainerHistoryPoint {
    pub at: String,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_avg: f64,
    #[ts(type = "number")]
    pub memory_max: i64,
    #[ts(type = "number")]
    pub net_rx_bytes: i64,
    #[ts(type = "number")]
    pub net_tx_bytes: i64,
    #[ts(type = "number")]
    pub block_read_bytes: i64,
    #[ts(type = "number")]
    pub block_write_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ContainerUsageSummary {
    pub name: String,
    pub project: Option<String>,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    #[ts(type = "number")]
    pub memory_max: i64,
    #[ts(type = "number")]
    pub net_rx_bytes: i64,
    #[ts(type = "number")]
    pub net_tx_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct HostHistoryResponse {
    pub host_key: String,
    pub range: MetricsRange,
    pub step_minutes: usize,
    pub host: Vec<HostHistoryPoint>,
    pub containers: Vec<ContainerUsageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ContainerHistoryResponse {
    pub host_key: String,
    pub container_name: String,
    pub range: MetricsRange,
    pub step_minutes: usize,
    pub points: Vec<ContainerHistoryPoint>,
}
