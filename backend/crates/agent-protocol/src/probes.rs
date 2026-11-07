//! Tarefas que a central entrega a um probe ou agente e os resultados de
//! descoberta que voltam (§8.11).

use serde::{Deserialize, Serialize};

/// Contrato de fio entre o servidor e o agente do probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeTask {
    pub id: String,
    pub monitor_id: i64,
    #[serde(rename = "type")]
    pub task_type: String,
    pub timeout_ms: i32,
    pub payload: serde_json::Value,
}

/// Contrato separado para discovery. Mantém compatibilidade com agentes que
/// conhecem apenas tarefas de monitor e evita monitor fictício no banco.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeDiscoveryTask {
    pub id: String,
    pub run_id: i64,
    pub cidr: String,
    pub timeout_ms: u64,
}

/// Um host visto por um scanner de descoberta, já reconciliado por IP.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredHost {
    pub ip_address: String,
    pub mac_address: Option<String>,
    pub hostname: Option<String>,
    pub mdns_name: Option<String>,
    pub vendor: Option<String>,
    pub device_type: Option<String>,
    #[serde(default)]
    pub open_ports: Vec<u16>,
    pub confidence: i32,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// O resultado de uma tarefa de descoberta.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeDiscoveryResultPayload {
    pub run_id: i64,
    pub task_id: Option<String>,
    #[serde(default)]
    pub hosts: Vec<DiscoveredHost>,
    pub error: Option<String>,
}
