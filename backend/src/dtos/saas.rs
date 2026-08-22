//! DTOs para catálogo de SaaS e visualização de Heatmap Horário de Latência.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Limites sugeridos de latência e perda de pacotes para o serviço SaaS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SaasThresholds {
    pub warning_latency_ms: f64,
    pub critical_latency_ms: f64,
    pub max_packet_loss_percent: Option<f64>,
}

/// Item do catálogo de serviços SaaS pré-configurados.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SaasPreset {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub category: String,
    pub icon: String,
    pub color: String,
    pub description: String,
    pub check_type: String,
    pub target: String,
    pub port: Option<i64>,
    pub http_method: Option<String>,
    pub accepted_status_codes: Option<Vec<u16>>,
    pub interval_seconds: i32,
    pub timeout_seconds: i32,
    pub suggested_thresholds: SaasThresholds,
    pub is_provisioned: bool,
    pub monitor_id: Option<i64>,
    pub current_status: Option<String>,
    pub current_latency_ms: Option<f64>,
}

/// Resposta da listagem de presets SaaS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SaasPresetsResponse {
    pub presets: Vec<SaasPreset>,
    pub total_presets: usize,
    pub provisioned_count: usize,
}

/// Requisição de provisionamento de serviços SaaS em lote ou individual.
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SaasProvisionRequest {
    pub preset_ids: Vec<String>,
    pub interval_seconds: Option<i32>,
    pub timeout_seconds: Option<i32>,
}

/// Resposta do provisionamento de serviços SaaS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SaasProvisionResponse {
    pub provisioned_count: usize,
    pub created_monitor_ids: Vec<i64>,
    pub existing_monitor_ids: Vec<i64>,
    pub message: String,
}

/// Filtros para consulta da matriz de heatmap horário.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HourlyHeatmapQuery {
    pub monitor_id: Option<i64>,
    pub is_saas: Option<bool>,
    pub days: Option<i64>,
}

/// Célula individual da matriz de calor horária (Data x Hora).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct HourlyHeatmapCell {
    pub date: String,
    #[ts(type = "number")]
    pub day_of_week: u32,
    #[ts(type = "number")]
    pub hour: u32,
    #[ts(type = "number | null")]
    pub avg_latency_ms: Option<f64>,
    #[ts(type = "number | null")]
    pub min_latency_ms: Option<f64>,
    #[ts(type = "number | null")]
    pub max_latency_ms: Option<f64>,
    #[ts(type = "number")]
    pub uptime_percentage: f64,
    #[ts(type = "number")]
    pub total_checks: i64,
    #[ts(type = "number")]
    pub up_checks: i64,
    #[ts(type = "number")]
    pub down_checks: i64,
}

/// Resumo agregado por hora do dia (0h..23h) ao longo do período selecionado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct HourOfDaySummary {
    #[ts(type = "number")]
    pub hour: u32,
    #[ts(type = "number | null")]
    pub avg_latency_ms: Option<f64>,
    #[ts(type = "number | null")]
    pub min_latency_ms: Option<f64>,
    #[ts(type = "number | null")]
    pub max_latency_ms: Option<f64>,
    #[ts(type = "number")]
    pub uptime_percentage: f64,
    #[ts(type = "number")]
    pub total_checks: i64,
}

/// Metadados resumidos dos monitores abrangidos pelo heatmap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct HourlyHeatmapMonitorSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub target: String,
    pub check_type: String,
    pub is_saas: bool,
    pub saas_service: Option<String>,
    pub current_status: String,
}

/// Resposta completa com a matriz de calor horária e indicadores de pico.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct HourlyHeatmapResponse {
    pub matrix: Vec<HourlyHeatmapCell>,
    pub by_hour_of_day: Vec<HourOfDaySummary>,
    pub monitors: Vec<HourlyHeatmapMonitorSummary>,
    #[ts(type = "number | null")]
    pub overall_avg_latency_ms: Option<f64>,
    #[ts(type = "number | null")]
    pub peak_hour: Option<u32>,
    #[ts(type = "number | null")]
    pub best_hour: Option<u32>,
    #[ts(type = "number")]
    pub overall_uptime_percentage: f64,
    #[ts(type = "number")]
    pub total_checks: i64,
    pub start_date: String,
    pub end_date: String,
}
