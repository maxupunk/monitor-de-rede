//! O que a área VPN contribui com a IA (ADR 017): a seção `vpn` do
//! `get_platform_status` — servidor WireGuard e peers (handshake, tráfego).
//! Nada que seja segredo sai daqui (chave privada).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use netmonitor_core::{
    ai::platform::{mib, rfc3339, PlatformSection},
    shared::errors::AppResult,
};
use serde_json::{json, Value};

use crate::{peer_service, server_service};

/// A seção `vpn` do estado da plataforma.
pub struct VpnStatus;

#[async_trait]
impl PlatformSection for VpnStatus {
    fn key(&self) -> &'static str {
        "vpn"
    }

    async fn report(
        &self,
        ctx: &AppContext,
        _since: DateTime<Utc>,
        limit: usize,
    ) -> AppResult<Value> {
        let state = server_service::get_state(&ctx.db).await?;
        let Some(server) = state.server.as_ref() else {
            return Ok(json!({ "configured": false }));
        };
        let mut peers = peer_service::list(&ctx.db).await?;
        // Quem precisa de atenção primeiro.
        peers.sort_by_key(|item| {
            matches!(
                item.peer.connection_status(),
                netmonitor_core::models::vpn_peers::VpnPeerConnectionStatus::Connected
            )
        });
        Ok(json!({
            "configured": true,
            "interface": server.interface_name,
            "listen_port": server.listen_port,
            "endpoint": server.public_endpoint,
            "active": server.active,
            "cidr": state.cidr,
            "peers_total": state.peers_total,
            "peers_connected": state.peers_connected,
            "rx_mib": mib(state.bytes_rx),
            "tx_mib": mib(state.bytes_tx),
            "peers": peers.iter().take(limit).map(|item| json!({
                "device": item.device.as_ref().map(|device| device.name.clone()),
                "status": item.peer.connection_status(),
                "enabled": item.peer.enabled,
                "last_handshake_at": item.peer.last_handshake_at.as_ref().map(rfc3339),
                "rx_mib": mib(item.peer.bytes_rx),
                "tx_mib": mib(item.peer.bytes_tx),
                "needs_firewall_rule": item.hints.needs_firewall_hint,
            })).collect::<Vec<_>>(),
            "omitted": peers.len().saturating_sub(limit),
        }))
    }
}
