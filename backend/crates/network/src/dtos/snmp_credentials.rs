//! Contratos HTTP das credenciais SNMP (`/api/snmp/credentials`) e das
//! consultas avulsas (`/api/snmp/test`, `/api/snmp/interfaces-query`).
//!
//! Segredo nunca volta para a tela: a resposta traz [`SECRET_MASK`] no lugar
//! de cada segredo gravado, e a gravação que recebe a máscara (ou vazio)
//! mantém o que estava.

use serde::{Deserialize, Serialize};

pub use crate::plugins::params::SECRET_MASK;

/// Corpo de criação e edição de uma credencial.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpCredentialInput {
    pub name: String,
    /// `v1`, `v2c` ou `v3`.
    pub version: String,
    #[serde(default)]
    pub community: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub auth_protocol: Option<String>,
    #[serde(default)]
    pub auth_key: Option<String>,
    #[serde(default)]
    pub priv_protocol: Option<String>,
    #[serde(default)]
    pub priv_key: Option<String>,
    #[serde(default)]
    pub context_name: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

/// Uma credencial como a tela a vê: segredos mascarados.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpCredentialView {
    pub id: i64,
    pub name: String,
    pub version: String,
    /// `noAuthNoPriv`, `authNoPriv` ou `authPriv` (v3); `community` no v1/v2c.
    pub security_level: String,
    /// [`SECRET_MASK`] quando gravada; `null` no v3.
    pub community: Option<String>,
    pub username: Option<String>,
    pub auth_protocol: Option<String>,
    /// [`SECRET_MASK`] quando gravada.
    pub auth_key: Option<String>,
    pub priv_protocol: Option<String>,
    /// [`SECRET_MASK`] quando gravada.
    pub priv_key: Option<String>,
    pub context_name: Option<String>,
    pub is_default: bool,
    /// Quantos dispositivos apontam para ela.
    pub device_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

/// `POST /api/snmp/credentials/{id}/test`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpCredentialTestInput {
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
}

/// Consulta avulsa a um alvo, antes de haver dispositivo salvo.
///
/// A credencial é, nesta ordem: o perfil `profileId`; a avulsa (quando
/// `version` vem preenchida — o formulário de uma credencial ainda não
/// gravada); a padrão.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpTargetInput {
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub profile_id: Option<i64>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub community: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub auth_protocol: Option<String>,
    #[serde(default)]
    pub auth_key: Option<String>,
    #[serde(default)]
    pub priv_protocol: Option<String>,
    #[serde(default)]
    pub priv_key: Option<String>,
    #[serde(default)]
    pub context_name: Option<String>,
    /// Tenta também os perfis cadastrados e diz qual respondeu.
    #[serde(default)]
    pub auto_detect: Option<bool>,
}
