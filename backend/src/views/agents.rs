//! Contratos de saída dos agentes remotos e dos hosts Docker.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::agents::policy::Permission;

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentCapabilityView {
    pub available: bool,
    pub version: Option<String>,
    pub reason: Option<String>,
}

/// O que a última conexão do agente anunciou (guardado em
/// `probes.configuration`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentHostInfo {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub in_container: bool,
    pub policy: Vec<Permission>,
    pub docker: AgentCapabilityView,
    pub compose: AgentCapabilityView,
}

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

/// Um host Docker disponível para a tela: a própria central ou um agente.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerHostView {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub online: bool,
    #[ts(type = "number | null")]
    pub agent_id: Option<i64>,
    pub docker_available: bool,
    pub compose_available: bool,
    pub policy: Vec<Permission>,
}

/// Uma operação longa (pull, update, compose) aceita pela central.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerOperationAccepted {
    pub operation_id: String,
    pub host_key: String,
    pub kind: String,
    pub target: String,
}

/// Progresso publicado no SSE como `docker:operation`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerOperationEvent {
    pub operation_id: String,
    pub host_key: String,
    pub kind: String,
    pub target: String,
    /// `running`, `succeeded` ou `failed`.
    pub state: String,
    pub line: Option<String>,
    pub message: Option<String>,
}

/// Linha de log em follow publicada no SSE como `docker:log`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerLogStreamEvent {
    pub stream_id: String,
    pub host_key: String,
    pub container_id: String,
    pub entries: Vec<crate::views::docker::DockerLogEntry>,
    /// Presente quando o stream terminou.
    pub ended: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerLogStreamStarted {
    pub stream_id: String,
}
