//! Contrato HTTP dos plugins de dispositivo.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::services::plugins::{
    compat::Compat,
    credentials::CredentialView,
    manifest::{MatchRule, PluginAction, PluginPanel, TransportKind},
    package::PluginPackage,
    review::{ReviewReport, Severity},
    runtime::TranscriptEntry,
};

/// Um plugin na listagem — sem script nem testes.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    /// `model` (todo equipamento compatível) ou `device` (exclusivo).
    pub scope: String,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    /// `builtin`, `user`, `ai` ou `imported`.
    pub source: String,
    /// `quarantine`, `draft`, `tested`, `active` ou `disabled`.
    pub status: String,
    pub transports: Vec<TransportKind>,
    pub actions: Vec<PluginAction>,
    pub matcher: MatchRule,
    /// Tela própria do plugin instalado, quando ele declara uma.
    pub panel: Option<PluginPanel>,
    /// Risco da última revisão de segurança.
    pub risk: Option<Severity>,
    pub last_test_at: Option<String>,
    pub last_test_ok: Option<bool>,
    /// Equipamentos em que já passou no teste funcional.
    pub validated_count: u32,
    pub updated_at: String,
}

/// O plugin inteiro, para o editor.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginDetail {
    pub summary: PluginSummary,
    pub package: PluginPackage,
    pub review: Option<ReviewReport>,
}

/// Um plugin do ponto de vista de um equipamento.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DevicePluginItem {
    pub plugin: PluginSummary,
    pub compat: Compat,
    pub reasons: Vec<String>,
    /// Instalado neste equipamento: ganha a própria aba.
    pub installed: bool,
    pub installed_at: Option<String>,
}

/// Agente remoto por onde o acesso pode sair.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AgentRouteOption {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub connected: bool,
    /// O agente anunciou `device_io` no `AGENT_ALLOW`.
    pub allows_device_io: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginRunView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number | null")]
    pub plugin_id: Option<i64>,
    pub plugin_name: Option<String>,
    #[ts(type = "number")]
    pub device_id: i64,
    pub action: String,
    /// `user`, `ai` ou `validation`.
    pub origin: String,
    /// `running`, `succeeded`, `failed` ou `cancelled`.
    pub status: String,
    #[ts(type = "unknown")]
    pub params: Option<Value>,
    #[ts(type = "unknown")]
    pub output: Option<Value>,
    pub transcript: Vec<TranscriptEntry>,
    pub error: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

/// Tudo que a aba "Plugins" de `/devices/{id}` precisa.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DevicePluginsView {
    #[ts(type = "number")]
    pub device_id: i64,
    /// Sistema efetivo do equipamento (`openwrt`, `linux`, …).
    pub platform: String,
    pub firmware: Option<String>,
    pub plugins: Vec<DevicePluginItem>,
    pub credentials: Vec<CredentialView>,
    pub runs: Vec<PluginRunView>,
    pub agents: Vec<AgentRouteOption>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginSaveInput {
    pub package: PluginPackage,
    /// Grava como exclusivo deste equipamento.
    #[serde(default)]
    #[ts(optional, type = "number | null")]
    pub device_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RunActionInput {
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub params: Value,
    /// Confirmação explícita para ação que altera o equipamento.
    #[serde(default)]
    pub confirm_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RunStarted {
    #[ts(type = "number")]
    pub run_id: i64,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ApprovalInput {
    pub approved: bool,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ReviewAcceptInput {
    /// O operador declara ter lido os riscos (exigido em risco alto e sem IA).
    #[serde(default)]
    pub acknowledge_risk: bool,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SessionSecretInput {
    pub secret: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AutoAcceptInput {
    pub conversation_key: String,
    #[ts(type = "number")]
    pub device_id: i64,
    /// Marcou "Estou ciente" no termo.
    pub accept_terms: bool,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AutoAcceptQuery {
    pub conversation_key: String,
    #[ts(type = "number")]
    pub device_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AutoAcceptState {
    pub active: bool,
    pub expires_at: Option<String>,
    pub terms_version: String,
    /// O texto do termo, para a tela mostrar exatamente o que é aceito.
    pub terms: String,
}
