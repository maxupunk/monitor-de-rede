//! Contratos de saída dos agentes remotos e dos hosts Docker.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use netmonitor_core::agents::views::{AgentCapabilityView, AgentHostInfo};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    /// Chave do host Docker deste agente (`agent-<id>`).
    pub host_key: String,
    #[ts(type = "number | null")]
    pub site_id: Option<i64>,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    pub device_name: Option<String>,
    pub device_ip: Option<String>,
    pub status: String,
    /// Canal aberto com este processo agora.
    pub connected: bool,
    pub version: Option<String>,
    pub last_seen_at: Option<String>,
    pub registered_at: Option<String>,
    pub enforce_tunnel_ip: bool,
    /// O agente local, ao lado desta central (ADR 014): sobe com a
    /// instalação e não é revogado nem removido pela tela.
    pub local: bool,
    pub host: AgentHostInfo,
    pub created_at: String,
}

/// Comandos de instalação para um endereço desta central.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentInstallCommands {
    /// Entrada de "Endereços deste servidor" usada; `None` quando
    /// `AGENT_SERVER_URL` fixa o endereço ou quando não há nenhum cadastrado.
    pub address_id: Option<String>,
    pub server_url: String,
    pub docker_command: String,
    pub systemd_command: String,
}

/// Instruções entregues uma única vez, junto com o código de enrollment.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentEnrollmentView {
    pub code: String,
    #[ts(type = "number")]
    pub expires_in_seconds: u64,
    pub commands: AgentInstallCommands,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentWithEnrollment {
    pub agent: AgentView,
    pub enrollment: AgentEnrollmentView,
}

/// Resposta ao agente que trocou o código.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentEnrollResponse {
    #[ts(type = "number")]
    pub probe_id: i64,
    pub name: String,
    pub token: String,
}
