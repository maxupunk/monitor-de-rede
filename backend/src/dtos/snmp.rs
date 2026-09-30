//! Contratos HTTP do SNMP que não são o escaneamento em si.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::ai::laya::{
    decisions::interfaces::{InterfaceCandidate, MonitorSuggestion},
    suggestion::LayaSuggestion,
};

/// `POST /api/snmp/interfaces/suggestions` — vem da tela, sem id: funciona
/// num cadastro que ainda não foi salvo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct InterfaceSuggestionsInput {
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub device_type: Option<String>,
    #[serde(default)]
    pub sys_descr: Option<String>,
    pub interfaces: Vec<InterfaceCandidate>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct InterfaceSuggestions {
    /// Falso com o Laya desligado, sem a frente liberada ou fora do ar — a
    /// tela simplesmente não mostra sugestão.
    pub available: bool,
    /// `value` é o ifIndex.
    pub uplink: Option<LayaSuggestion>,
    pub monitor: Vec<MonitorSuggestion>,
    pub model: Option<String>,
}
