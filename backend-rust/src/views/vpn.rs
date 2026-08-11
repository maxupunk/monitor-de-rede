//! Serialização dos recursos de VPN (§7.13).
//!
//! O peer é declarado campo a campo em vez de espalhar o `Model` inteiro: além
//! de tornar a resposta verificável pelo compilador, garante que material
//! sensível — a chave pré-compartilhada — não escape por descuido em rota
//! nenhuma. `preshared_key_encrypted` simplesmente não tem lugar aqui.

use serde::Serialize;

use crate::{
    models::{vpn_peers, vpn_servers},
    services::vpn::GeneratedArtifact,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnServerResponse {
    pub id: i64,
    pub network_id: i64,
    pub interface_name: String,
    pub listen_port: i32,
    pub public_endpoint: Option<String>,
    pub public_key: String,
    pub allow_peer_to_peer: bool,
    pub mtu: i32,
    pub dns_servers: Option<String>,
    pub active: bool,
    pub last_synced_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<vpn_servers::Model> for VpnServerResponse {
    fn from(row: vpn_servers::Model) -> Self {
        Self {
            id: row.id,
            network_id: row.network_id,
            interface_name: row.interface_name,
            listen_port: row.listen_port,
            public_endpoint: row.public_endpoint,
            public_key: row.public_key,
            allow_peer_to_peer: row.allow_peer_to_peer,
            mtu: row.mtu,
            dns_servers: row.dns_servers,
            active: row.active,
            last_synced_at: row.last_synced_at.map(|value| value.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnPeerResponse {
    pub id: i64,
    pub vpn_server_id: i64,
    pub device_id: i64,
    pub public_key: String,
    pub device_profile: String,
    pub persistent_keepalive: i32,
    pub last_handshake_at: Option<String>,
    /// Último keepalive contabilizado — é o sinal de vida que sustenta o status.
    pub last_seen_at: Option<String>,
    pub bytes_rx: i64,
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

/// Artefato entregue ao frontend — perfis móveis já vêm com o QR Code
/// renderizado, porque a chave privada não sobrevive a uma segunda requisição.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedVpnArtifact {
    #[serde(flatten)]
    pub artifact: GeneratedArtifact,
    pub qr_svg: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn peer() -> vpn_peers::Model {
        let now = Utc::now();
        vpn_peers::Model {
            id: 1,
            vpn_server_id: 1,
            device_id: 2,
            public_key: "PUB".into(),
            preshared_key_encrypted: Some("CRIPTOGRAMA-DA-PSK".into()),
            device_profile: "mobile".into(),
            persistent_keepalive: 25,
            last_handshake_at: None,
            last_seen_at: None,
            bytes_rx: 10,
            bytes_tx: 20,
            enabled: true,
            last_connection_status: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    #[test]
    fn a_chave_pre_compartilhada_nunca_sai_na_resposta() {
        let json = serde_json::to_string(&VpnPeerResponse::from(&peer())).unwrap();
        assert!(!json.contains("CRIPTOGRAMA-DA-PSK"));
        assert!(!json.contains("presharedKey"));
        assert!(!json.contains("preshared_key_encrypted"));
    }

    #[test]
    fn o_peer_serializa_em_camel_case_com_o_status_derivado() {
        let json = serde_json::to_value(VpnPeerResponse::from(&peer())).unwrap();
        assert_eq!(json["vpnServerId"], 1);
        assert_eq!(json["persistentKeepalive"], 25);
        assert_eq!(json["deviceProfile"], "mobile");
        // Sem sinal algum, o peer nasce "aguardando primeira conexão".
        assert_eq!(json["connectionStatus"], "awaiting");
    }

    #[test]
    fn a_chave_privada_do_servidor_nao_tem_lugar_na_resposta() {
        let now = Utc::now();
        let json = serde_json::to_string(&VpnServerResponse::from(vpn_servers::Model {
            id: 1,
            network_id: 1,
            interface_name: "wg0".into(),
            listen_port: 51_820,
            public_endpoint: None,
            public_key: "PUB".into(),
            private_key_encrypted: "CRIPTOGRAMA-DO-SERVIDOR".into(),
            allow_peer_to_peer: false,
            mtu: 1_420,
            dns_servers: None,
            active: true,
            last_synced_at: None,
            created_at: now.into(),
            updated_at: now.into(),
        }))
        .unwrap();
        assert!(!json.contains("CRIPTOGRAMA-DO-SERVIDOR"));
        assert!(!json.contains("privateKey"));
    }
}
