//! DTOs de resposta dos endpoints de monitores.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::monitoring::contracts::CheckResult;

/// Query para série temporal de latência e perda de pacotes.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorTimeSeriesQuery {
    pub monitor_id: Option<i64>,
    pub monitor_type: Option<String>,
    pub timeframe: Option<String>,
}

/// Item de detalhe de monitor em uma amostra temporal.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorTimeSeriesDetailItem {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub target: String,
    #[serde(rename = "type")]
    pub monitor_type: String,
    pub device_name: Option<String>,
    pub status: String,
    #[ts(type = "number | null")]
    pub latency_ms: Option<f64>,
    #[ts(type = "number")]
    pub loss_pct: i32,
}

/// Ponto da série temporal de latência.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorTimeSeriesPoint {
    pub time: String,
    #[ts(type = "number")]
    pub timestamp: i64,
    #[ts(type = "number")]
    pub latency: f64,
    #[ts(type = "number")]
    pub loss: i32,
    pub monitors_detail: Vec<MonitorTimeSeriesDetailItem>,
}

/// Resposta completa do endpoint de série temporal.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorTimeSeriesResponse {
    pub timeframe: String,
    pub samples: Vec<MonitorTimeSeriesPoint>,
    #[ts(type = "number")]
    pub avg_latency: f64,
    #[ts(type = "number")]
    pub max_latency: f64,
    #[ts(type = "number")]
    pub min_latency: f64,
    #[ts(type = "number")]
    pub packet_loss_pct: i32,
    #[ts(type = "number")]
    pub total_checks: i64,
}

/// Estatísticas agregadas exibidas no detalhe de um monitor.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorStats {
    #[ts(type = "number | null")]
    pub avg_latency: Option<i64>,
    #[ts(type = "number | null")]
    pub min_latency: Option<f64>,
    #[ts(type = "number | null")]
    pub max_latency: Option<f64>,
    #[ts(type = "number | null")]
    pub last_latency: Option<f64>,
    #[ts(type = "number")]
    pub uptime_percentage: f64,
    #[ts(type = "number")]
    pub total_checks: usize,
    #[ts(type = "number")]
    pub up_checks: usize,
}

/// Resposta de uma execução manual de monitor (ping, tcp, http, dns, etc.).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorRunResponse {
    pub message: String,
    #[ts(type = "any")]
    pub result: CheckResult,
}

/// Resposta de uma execução SNMP consolidada do dispositivo.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorSnmpRunResponse {
    pub message: String,
    #[ts(type = "any")]
    pub result: CheckResult,
}

/// Monitor com estatísticas agregadas — corpo de `GET /api/monitors/:id`.
///
/// Não deriva `TS` porque o campo `monitor` usa `serde(flatten)` sobre um
/// `serde_json::Value`; o `ts-rs` não aceita atributos de tipo junto com
/// `flatten`. A serialização JSON permanece inalterada.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorWithStats {
    #[serde(flatten)]
    pub monitor: serde_json::Value,
    pub stats: MonitorStats,
}

/// Resposta de uptime de um monitor em uma janela de horas.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorUptimeResponse {
    #[ts(type = "number")]
    pub monitor_id: i64,
    #[ts(type = "number")]
    pub hours: i64,
    #[ts(type = "number")]
    pub uptime_percentage: f64,
    #[ts(type = "number")]
    pub total_checks: i64,
    #[ts(type = "number")]
    pub up_checks: i64,
    #[ts(type = "number")]
    pub down_checks: i64,
    #[ts(type = "number")]
    pub unknown_checks: i64,
    #[ts(type = "number | null")]
    pub avg_latency_ms: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn result() -> CheckResult {
        CheckResult {
            success: true,
            status: crate::services::monitoring::contracts::MonitorStatus::Up,
            started_at: Utc::now(),
            finished_at: Utc::now(),
            duration_ms: 12,
            message: Some("ok".into()),
            metrics: Vec::new(),
            data: serde_json::json!({}),
        }
    }

    #[test]
    fn monitor_stats_serializa_em_camel_case() {
        let stats = MonitorStats {
            avg_latency: Some(15),
            min_latency: Some(10.0),
            max_latency: Some(20.0),
            last_latency: Some(12.0),
            uptime_percentage: 99.9,
            total_checks: 100,
            up_checks: 99,
        };
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["avgLatency"], 15);
        assert_eq!(json["uptimePercentage"], 99.9);
        assert_eq!(json["totalChecks"], 100);
    }

    #[test]
    fn run_response_serializa_em_camel_case() {
        let response = MonitorRunResponse {
            message: "Executado".into(),
            result: result(),
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["message"], "Executado");
        assert!(json.get("result").is_some());
    }
}
