//! Serialização dos recursos de VPN (§7.13).
//!
//! O peer é declarado campo a campo em vez de espalhar o `Model` inteiro: além
//! de tornar a resposta verificável pelo compilador, garante que material
//! sensível — a chave pré-compartilhada — não escape por descuido em rota
//! nenhuma. `preshared_key_encrypted` simplesmente não tem lugar aqui.
//!
//! **Bindings (F7).** Todo struct daqui é fonte da verdade do tipo TypeScript
//! equivalente em `frontend/src/bindings/`: `frontend/src/stores/vpn.ts`
//! reexporta o que o `ts-rs` gera em vez de redigitar os campos à mão. Trocar o
//! tipo de um campo aqui passa a quebrar o `vue-tsc`, e não a tela em produção.
//! Campos `i64` levam `#[ts(type = "number")]` porque o padrão do `ts-rs` para
//! inteiros de 64 bits é `bigint` — e `JSON.parse` nunca produz `bigint`.

use serde::Serialize;
use ts_rs::TS;

use crate::{
    models::{vpn_peers, vpn_servers},
    services::vpn::{peer_hints::PeerHints, profiles::registry::ProfileCard, GeneratedArtifact},
};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnServerResponse {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
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

/// Artefato entregue ao frontend — perfis móveis já vêm com o QR Code
/// renderizado, porque a chave privada não sobrevive a uma segunda requisição.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SerializedVpnArtifact {
    #[serde(flatten)]
    pub artifact: GeneratedArtifact,
    pub qr_svg: Option<String>,
}

/// Recorte do `device` que as telas de VPN leem da linha do peer.
///
/// O corpo em runtime é o `devices::present` inteiro — este struct **não** é
/// serializado, existe só para dar nome ao que a tabela de dispositivos VPN
/// consome. Descrever menos campos do que a resposta traz é seguro (o
/// TypeScript é estrutural); descrever campos que não existem não seria, e é
/// justamente isso que o tipo redigitado à mão arriscava.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnPeerDeviceView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub ip_address: Option<String>,
    pub snmp_enabled: bool,
    pub status: String,
}

/// Linha de `GET /api/vpn/peers`: o peer, os avisos de diagnóstico e o
/// dispositivo, achatados num objeto só — que é como a tela sempre leu.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnPeerListItem {
    #[serde(flatten)]
    pub peer: VpnPeerResponse,
    #[serde(flatten)]
    pub hints: PeerHints,
    /// `null` quando o dispositivo do peer já foi removido — a linha continua
    /// aparecendo para que o operador consiga revogá-la.
    #[ts(as = "Option<VpnPeerDeviceView>")]
    pub device: Option<serde_json::Value>,
}

/// Corpo de `PATCH /api/vpn/peers/:id` — peer + dispositivo renomeado.
///
/// Sem os avisos de diagnóstico: quem os calcula é a listagem, que tem o
/// contexto dos outros peers para isso. Um `PATCH` enxerga um peer só, então
/// emitir os avisos aqui significaria emiti-los errados.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnPeerWithDevice {
    #[serde(flatten)]
    pub peer: VpnPeerResponse,
    #[ts(as = "Option<VpnPeerDeviceView>")]
    pub device: Option<serde_json::Value>,
}

/// Corpo de `GET /api/vpn/server` — o painel inteiro numa requisição.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnServerStateResponse {
    pub configured: bool,
    pub server: Option<VpnServerResponse>,
    pub cidr: Option<String>,
    pub server_address: Option<String>,
    #[ts(type = "number")]
    pub peers_total: usize,
    #[ts(type = "number")]
    pub peers_connected: usize,
    #[ts(type = "number")]
    pub bytes_rx: i64,
    #[ts(type = "number")]
    pub bytes_tx: i64,
    pub persistent_keepalive: i32,
    pub profiles: Vec<ProfileCard>,
}

/// Resposta de `GET /api/vpn/peers/next-ip`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnNextIpResponse {
    pub ip_address: String,
    pub cidr: String,
}

/// Resposta de `POST /api/vpn/peers`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnPeerCreatedResponse {
    pub peer: VpnPeerResponse,
    #[ts(as = "Option<VpnPeerDeviceView>")]
    pub device: Option<serde_json::Value>,
    pub artifact: SerializedVpnArtifact,
}

/// Resposta de `GET /api/vpn/peers/:id/qrcode`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnQrCodeResponse {
    pub profile: String,
    pub file_name: String,
    pub svg: String,
}

/// Resposta de `POST /api/vpn/peers/:id/rotate`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnKeyRotationResponse {
    pub message: String,
    pub peer: VpnPeerResponse,
    pub artifact: SerializedVpnArtifact,
}

/// Resposta de `POST /api/vpn/peers/:id/firewall-hints`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnFirewallHintsResponse {
    pub profile: String,
    pub label: String,
    pub content: String,
    pub message: String,
}

/// Resposta de `DELETE /api/vpn/peers/:id`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct VpnPeerRevokedResponse {
    pub message: String,
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

    #[test]
    fn novos_dtos_vpn_serializam_em_camel_case() {
        let next_ip = serde_json::to_value(VpnNextIpResponse {
            ip_address: "10.8.0.11".into(),
            cidr: "10.8.0.0/24".into(),
        })
        .unwrap();
        assert_eq!(next_ip["ipAddress"], "10.8.0.11");

        let hints = serde_json::to_value(VpnFirewallHintsResponse {
            profile: "mikrotik".into(),
            label: "MikroTik".into(),
            content: "/ip/firewall...".into(),
            message: "Copie as regras".into(),
        })
        .unwrap();
        assert_eq!(hints["fileName"], serde_json::Value::Null);
        assert_eq!(hints["message"], "Copie as regras");

        let revoked = serde_json::to_value(VpnPeerRevokedResponse {
            message: "revogado".into(),
        })
        .unwrap();
        assert_eq!(revoked["message"], "revogado");
    }
}
