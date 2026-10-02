//! Contrato HTTP dos plugins de dispositivo.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::services::plugins::{
    compat::Compat,
    credentials::CredentialView,
    manifest::{
        FleetSpec, MatchRule, PluginAction, PluginPanel, SettingsSpec, Surface, TransportKind,
    },
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
    /// Onde aparece: aba do equipamento, página da frota (Aplicativos).
    pub surfaces: Vec<Surface>,
    /// Esquemas da configuração guardada.
    pub settings: Option<SettingsSpec>,
    pub fleet: Option<FleetSpec>,
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
    /// Ajuste deste equipamento (segredos mascarados), quando o plugin tem.
    #[ts(type = "Record<string, unknown> | null")]
    pub device_settings: Option<Value>,
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

/// Um equipamento dentro de um lote de frota.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct BatchDevice {
    #[ts(type = "number")]
    pub device_id: i64,
    pub device_name: String,
    #[ts(type = "number | null")]
    pub run_id: Option<i64>,
    /// `pending`, `running`, `succeeded`, `failed` ou `skipped`.
    pub status: String,
    pub error: Option<String>,
    #[ts(type = "unknown")]
    pub output: Option<Value>,
}

/// Uma ação de frota em andamento ou concluída.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginBatchView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub plugin_id: i64,
    pub action: String,
    pub title: String,
    /// `running`, `succeeded`, `partial`, `failed` ou `cancelled`.
    pub status: String,
    pub devices: Vec<BatchDevice>,
    /// O consolidado da função `reduce`, quando a ação tem uma.
    #[ts(type = "unknown")]
    pub result: Option<Value>,
    /// O consolidado propõe ajustes por equipamento ainda não aplicados.
    pub has_patch: bool,
    pub error: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

/// Um equipamento da frota.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetMember {
    #[ts(type = "number")]
    pub device_id: i64,
    pub name: String,
    pub ip: Option<String>,
    pub platform: String,
    pub firmware: Option<String>,
    pub compat: Compat,
    /// Por que a compatibilidade é essa (para a tela explicar).
    pub reasons: Vec<String>,
    pub installed_at: String,
    /// Há credencial pronta para os transportes do plugin.
    pub credentials_ready: bool,
    /// Ajuste deste equipamento (segredos mascarados).
    #[ts(type = "Record<string, unknown> | null")]
    pub settings: Option<Value>,
}

/// Equipamento cadastrado fora da frota. O incompatível também vem, com o
/// porquê — escondê-lo deixaria o operador sem saber por que ele não aparece.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetCandidate {
    #[ts(type = "number")]
    pub device_id: i64,
    pub name: String,
    pub ip: Option<String>,
    pub compat: Compat,
    pub reasons: Vec<String>,
}

/// A página de um plugin de frota.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetView {
    pub plugin: PluginSummary,
    /// Configuração da frota (segredos mascarados).
    #[ts(type = "Record<string, unknown> | null")]
    pub settings: Option<Value>,
    pub members: Vec<FleetMember>,
    pub candidates: Vec<FleetCandidate>,
    pub batches: Vec<PluginBatchView>,
}

/// Um aplicativo (plugin de frota) para o menu.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginApp {
    #[ts(type = "number")]
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub icon: String,
    pub description: Option<String>,
    pub members: u32,
}

/// Corpo de "Verificar sistema".
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetIdentifyInput {
    /// Só estes; vazio = os da frota e os candidatos ainda em dúvida.
    #[serde(default)]
    #[ts(type = "Array<number>")]
    pub device_ids: Vec<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetRunInput {
    /// Só estes membros; vazio = todos.
    #[serde(default)]
    #[ts(type = "Array<number>")]
    pub device_ids: Vec<i64>,
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub params: Value,
    /// Parâmetros de um membro, por cima dos comuns.
    #[serde(default)]
    #[ts(optional, type = "Record<number, Record<string, unknown>>")]
    pub device_params: std::collections::HashMap<i64, Value>,
    #[serde(default)]
    pub confirm_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct BatchStarted {
    #[ts(type = "number")]
    pub batch_id: i64,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SettingsInput {
    #[ts(type = "Record<string, unknown>")]
    pub value: Value,
}
