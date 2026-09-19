//! DTOs de diagnóstico de rede: traceroute, speedtest e playbooks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct TracerouteInput {
    pub host: String,
    pub max_hops: Option<u8>,
    pub timeout_ms: Option<u64>,
    pub probes_per_hop: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct TracerouteHop {
    pub hop: u8,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub rtt_ms: Vec<Option<f64>>,
    pub avg_rtt_ms: Option<f64>,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct SpeedTestInput {
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct SpeedTestProgress {
    pub phase: String,
    pub progress_pct: f64,
    pub current_mbps: Option<f64>,
    pub ping_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub download_mbps: Option<f64>,
    pub upload_mbps: Option<f64>,
    pub server_name: Option<String>,
    pub server_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct SpeedTestResult {
    pub ping_ms: f64,
    pub jitter_ms: f64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub server_name: Option<String>,
    pub server_location: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct PlaybookRunInput {
    pub playbook_type: String,
    pub target: Option<String>,
    pub device_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct PlaybookStepResult {
    pub step_index: u8,
    pub step_name: String,
    pub description: String,
    pub status: String,
    pub message: Option<String>,
    #[ts(type = "Record<string, unknown>")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "diagnostics/")]
pub struct PlaybookSummary {
    pub playbook_type: String,
    pub target: Option<String>,
    pub status: String,
    pub diagnosis: String,
    pub recommendations: Vec<String>,
    pub steps: Vec<PlaybookStepResult>,
}
