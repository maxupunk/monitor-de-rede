//! O peer de VPN de um dispositivo como as telas o veem — a VPN o lista e a
//! lista de dispositivos o embute. Declarado campo a campo: a chave
//! pré-compartilhada não tem lugar aqui.

use serde::Serialize;
use ts_rs::TS;

use crate::models::vpn_peers;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnPeerResponse {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub vpn_server_id: i64,
    #[ts(type = "number")]
    pub device_id: i64,
    pub public_key: String,
    pub device_profile: String,
    pub persistent_keepalive: i32,
    pub last_handshake_at: Option<String>,
    /// Último keepalive contabilizado — é o sinal de vida que sustenta o status.
    pub last_seen_at: Option<String>,
    #[ts(type = "number")]
    pub bytes_rx: i64,
    #[ts(type = "number")]
    pub bytes_tx: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub connection_status: vpn_peers::VpnPeerConnectionStatus,
}

impl From<&vpn_peers::Model> for VpnPeerResponse {
    fn from(row: &vpn_peers::Model) -> Self {
        Self {
            id: row.id,
            vpn_server_id: row.vpn_server_id,
            device_id: row.device_id,
            public_key: row.public_key.clone(),
            device_profile: row.device_profile.clone(),
            persistent_keepalive: row.persistent_keepalive,
            last_handshake_at: row.last_handshake_at.map(|value| value.to_rfc3339()),
            last_seen_at: row.last_seen_at.map(|value| value.to_rfc3339()),
            bytes_rx: row.bytes_rx,
            bytes_tx: row.bytes_tx,
            enabled: row.enabled,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
            connection_status: row.connection_status(),
        }
    }
}
