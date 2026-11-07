//! Contratos de saída da tela Docker que não atravessam o canal do agente
//! (os que atravessam são do `agent-protocol` e estão reexportados aqui).

use agent_protocol::policy::Permission;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use agent_protocol::docker::views::*;

/// Um host Docker disponível para a tela. Todo host é um agente (ADR 014):
/// `kind` é `local` para o desta central e `agent` para os remotos.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerHostView {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub online: bool,
    #[ts(type = "number")]
    pub agent_id: i64,
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
    pub entries: Vec<DockerLogEntry>,
    /// Presente quando o stream terminou.
    pub ended: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerLogStreamStarted {
    pub stream_id: String,
}
