//! Entrada do cadastro de redes (as faixas que a descoberta varre).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInput {
    pub site_id: Option<i64>,
    pub probe_id: Option<i64>,
    pub name: String,
    pub cidr: String,
    pub gateway: Option<String>,
    pub vlan: Option<i32>,
    pub dns_servers: Option<serde_json::Value>,
    pub scan_enabled: Option<bool>,
    pub scan_interval: Option<i32>,
    pub active: Option<bool>,
}
