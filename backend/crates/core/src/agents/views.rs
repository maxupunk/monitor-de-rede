//! O que a tela lê de um agente e da rota que ele oferece.

use agent_protocol::policy::Permission;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

/// Agente remoto por onde um acesso pode sair (plugin de dispositivo, ponte
/// de banco). A tela desabilita quem não serve e diz por quê.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentRouteOption {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub connected: bool,
    /// O agente anunciou a permissão do recurso no `AGENT_ALLOW`.
    pub allowed: bool,
}
