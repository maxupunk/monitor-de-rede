//! Proteções dos endpoints sensíveis de VPN (§8.10.4).
//!
//! Rate limit por solicitante e registro de auditoria de todo download de
//! configuração — os artefatos são credencial de acesso à rede, não payload
//! comum. O rate limit e o IP de origem são do núcleo (a autenticação usa os
//! mesmos); o caminho da área continua valendo.

use serde::Serialize;

pub use netmonitor_core::shared::rate_limit::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VpnAuditAction {
    ConfigDownload,
    QrcodeDownload,
    KeyRotation,
    PeerRevoked,
    PeerCreated,
}

impl VpnAuditAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigDownload => "config_download",
            Self::QrcodeDownload => "qrcode_download",
            Self::KeyRotation => "key_rotation",
            Self::PeerRevoked => "peer_revoked",
            Self::PeerCreated => "peer_created",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VpnAuditEntry {
    pub action: VpnAuditAction,
    pub peer_id: Option<i64>,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub details: serde_json::Value,
}

/// Trilha de auditoria: quem acessou, quando e qual peer.
///
/// Vai para o log estruturado (e não para uma tabela) porque o destino natural
/// é o coletor de logs da operação, onde já vivem as demais trilhas.
pub fn audit(entry: &VpnAuditEntry) {
    tracing::info!(
        audit = "vpn",
        action = entry.action.as_str(),
        peer_id = entry.peer_id,
        user_id = entry.user_id.as_deref().unwrap_or("anônimo"),
        request_ip = entry.ip_address.as_deref().unwrap_or("-"),
        details = %entry.details,
        "[VPN][auditoria] {} peer={}",
        entry.action.as_str(),
        entry.peer_id.map_or_else(|| "-".to_string(), |id| id.to_string())
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_acoes_serializam_no_vocabulario_do_roadmap() {
        assert_eq!(VpnAuditAction::ConfigDownload.as_str(), "config_download");
        assert_eq!(VpnAuditAction::QrcodeDownload.as_str(), "qrcode_download");
        assert_eq!(VpnAuditAction::KeyRotation.as_str(), "key_rotation");
        assert_eq!(VpnAuditAction::PeerRevoked.as_str(), "peer_revoked");
        assert_eq!(VpnAuditAction::PeerCreated.as_str(), "peer_created");
    }
}
