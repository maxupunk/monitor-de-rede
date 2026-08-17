//! Contrato da lista "por onde os equipamentos alcançam este servidor".
//!
//! Ver [`crate::services::server_addresses`] para o conceito.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Uma entrada já resolvida, pronta para a tela.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ServerAddressEntry {
    /// `lan`, `vpn`, `public` ou `custom:<uuid>`.
    pub id: String,
    /// `lan` | `vpn` | `public` | `custom`.
    pub kind: String,
    pub label: String,
    /// Quando usar este endereço. É esta frase que carrega o conceito na tela —
    /// sem ela a lista vira três IPs sem critério.
    pub description: String,
    /// O que vale hoje. Nulo quando não foi detectado nem definido.
    pub value: Option<String>,
    /// O que o servidor descobriu sozinho, para a tela oferecer "voltar ao
    /// detectado" depois de uma correção.
    pub detected: Option<String>,
    pub overridden: bool,
    /// De onde veio o valor, ou por que não há um. Palpite apresentado como
    /// certeza é pior do que campo vazio.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ServerAddressesResponse {
    pub data: Vec<ServerAddressEntry>,
    /// Qual usar quando nada indicar outro.
    pub preferred_id: Option<String>,
}

/// Corpo de `PUT /api/server-addresses`.
///
/// É o documento **do operador**, não a lista inteira: os detectados não são
/// enviados de volta porque gravá-los os congelaria — o IP da rede local muda,
/// e a tela continuaria mostrando o antigo com toda a confiança.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveServerAddressesInput {
    /// Correções sobre os detectados, por tipo. Valor em branco desfaz a
    /// correção e devolve o detectado.
    #[serde(default)]
    pub overrides: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub custom: Vec<CustomServerAddressInput>,
    #[serde(default)]
    pub preferred_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomServerAddressInput {
    /// Ausente num item novo; o servidor sorteia.
    #[serde(default)]
    pub id: String,
    pub label: String,
    pub value: String,
}
