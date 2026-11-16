//! Entradas HTTP dos agentes remotos (ADR 011).

use serde::Deserialize;
use ts_rs::TS;

pub use netmonitor_core::agents::{install_policy::InstallPolicyView, views::AgentRouteOption};

/// Cadastro de um agente pela central.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentCreateInput {
    pub name: String,
    #[ts(type = "number | null")]
    pub site_id: Option<i64>,
    /// Dispositivo (normalmente o peer da VPN) que hospeda o agente.
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    /// Exigir que a conexão venha do IP do túnel do dispositivo (padrão: sim).
    pub enforce_tunnel_ip: Option<bool>,
}

/// Alteração do cadastro.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentUpdateInput {
    pub name: Option<String>,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    pub enforce_tunnel_ip: Option<bool>,
}

/// Regera os comandos de instalação para outro endereço desta central ou
/// outra política.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentInstallCommandsInput {
    pub code: String,
    /// Id de uma entrada de "Endereços deste servidor"; sem ele, o padrão.
    pub address_id: Option<String>,
    /// O que o host vai liberar; sem ela, a sugestão (tudo).
    pub policy: Option<InstallPolicyView>,
}

/// Troca do código de enrollment pelo token (chamada pelo próprio agente).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentEnrollInput {
    pub code: String,
    pub hostname: Option<String>,
}
