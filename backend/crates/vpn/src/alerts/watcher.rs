//! Observa o estado dos túneis WireGuard (§8.10.4).
//!
//! Roda logo depois da sincronização da telemetria, quando `last_seen_at` e
//! `last_handshake_at` já refletem o que o container publicou. Compara o estado
//! persistido no ciclo anterior com o atual, publica a transição no feed em
//! tempo real e entrega os fatos ao motor de alertas, que a central oferece
//! pelo [`CentralServices`](crate::central::CentralServices).
//!
//! Não decide o que é alerta: a política ("túnel caído é crítico", "instável só
//! avisa depois de 5 minutos") vive nas regras do catálogo `vpn_*`.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;

use netmonitor_core::{
    alerts::{
        contracts::{
            AlertDataset, AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey, AlertTarget,
        },
        fields,
    },
    events::EventBus,
    models::{devices, vpn_peers},
    shared::errors::AppResult,
};

use super::vpn_peer::{self, VpnPeerDataset, VpnPeerFacts};
use crate::central::Central;

/// Avalia todos os peers habilitados de um servidor.
///
/// Devolve quantos mudaram de estado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn evaluate_server_peers(ctx: &AppContext, vpn_server_id: i64) -> AppResult<usize> {
    let central = Central::from_context(ctx)?;
    let peers = vpn_peers::Entity::find_enabled_for_server(vpn_server_id)
        .all(&ctx.db)
        .await?;
    let mut transitions = 0;
    for peer in peers {
        if evaluate_peer(ctx, &central, peer).await? {
            transitions += 1;
        }
    }
    Ok(transitions)
}

/// `true` quando o túnel mudou de estado neste ciclo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn evaluate_peer(
    ctx: &AppContext,
    central: &Central,
    peer: vpn_peers::Model,
) -> AppResult<bool> {
    let device = devices::Entity::find_by_id(peer.device_id)
        .one(&ctx.db)
        .await?;
    let peer_name = device.as_ref().map_or_else(
        || format!("Peer #{}", peer.id),
        |device| device.name.clone(),
    );
    let status = peer.connection_status();
    let previous_status = peer
        .last_connection_status
        .as_deref()
        .and_then(parse_status);

    let dataset = VpnPeerDataset::new(&VpnPeerFacts {
        peer_name: &peer_name,
        status,
        previous_status,
        seconds_since_activity: seconds_since_activity(&peer),
    });
    let changed = vpn_peer::has_transition(&dataset.0);
    let message = dataset.describe();
    let peer_id = peer.id;
    let vpn_server_id = peer.vpn_server_id;
    let device_id = peer.device_id;

    // O estado é gravado **antes** da avaliação: se a notificação falhar, o
    // ciclo seguinte não repete a mesma transição como se fosse nova.
    if previous_status != Some(status) {
        let mut active: vpn_peers::ActiveModel = peer.into();
        active.last_connection_status = Set(Some(vpn_peer::status_label(status).to_string()));
        active.update(&ctx.db).await?;
    }

    if !changed {
        return Ok(false);
    }

    publish_transition(
        ctx,
        peer_id,
        vpn_server_id,
        device_id,
        device.as_ref(),
        &dataset.0,
        &message,
    )
    .await;

    let mut data = serde_json::Map::new();
    data.insert("vpnPeerId".into(), json!(peer_id));
    data.insert("vpnServerId".into(), json!(vpn_server_id));
    // Túnel não tem "warning" (`degraded` sai `false`): a transição
    // `destabilized` já é fato.
    let target = AlertTarget {
        scope: AlertEvaluationScope {
            site_id: device.as_ref().and_then(|device| device.site_id),
            device_id: Some(device_id),
            monitor_id: None,
        },
        scope_key: AlertScopeKey::vpn_peer(peer_id),
        label: peer_name,
        data,
    };
    central
        .evaluate_alert(ctx, &AlertEvaluationContext::of(&dataset, target))
        .await?;

    Ok(true)
}

/// Lê o status persistido no ciclo anterior. Valor desconhecido vira `None` —
/// e sem estado anterior não há transição, que é o comportamento seguro.
fn parse_status(raw: &str) -> Option<vpn_peers::VpnPeerConnectionStatus> {
    use vpn_peers::VpnPeerConnectionStatus as Status;
    match raw {
        "connected" => Some(Status::Connected),
        "unstable" => Some(Status::Unstable),
        "disconnected" => Some(Status::Disconnected),
        "awaiting" => Some(Status::Awaiting),
        _ => None,
    }
}

fn seconds_since_activity(peer: &vpn_peers::Model) -> Option<i64> {
    peer.last_activity_at()
        .map(|last| (Utc::now() - last.to_utc()).num_seconds().max(0))
}

/// Feed em tempo real: a transição observada, independentemente de alertar.
#[allow(clippy::too_many_arguments)]
async fn publish_transition(
    ctx: &AppContext,
    peer_id: i64,
    vpn_server_id: i64,
    device_id: i64,
    device: Option<&devices::Model>,
    dataset: &serde_json::Map<String, serde_json::Value>,
    message: &str,
) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    let payload = json!({
        "vpnPeerId": peer_id,
        "vpnServerId": vpn_server_id,
        "deviceId": device_id,
        "deviceName": device.map(|device| device.name.clone()),
        "previousStatus": dataset.get(fields::VPN_PREVIOUS_STATUS),
        "currentStatus": dataset.get(fields::VPN_PEER_STATUS),
        "transition": dataset.get(fields::VPN_STATUS_TRANSITION),
        "message": message,
    });
    if let Err(error) = bus
        .publish(&ctx.db, "vpn:peer_status_change", payload)
        .await
    {
        tracing::warn!(%error, peer_id, "falha ao publicar vpn:peer_status_change");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netmonitor_core::models::vpn_peers::VpnPeerConnectionStatus as Status;

    #[test]
    fn o_status_persistido_e_lido_de_volta() {
        assert_eq!(parse_status("connected"), Some(Status::Connected));
        assert_eq!(parse_status("unstable"), Some(Status::Unstable));
        assert_eq!(parse_status("disconnected"), Some(Status::Disconnected));
        assert_eq!(parse_status("awaiting"), Some(Status::Awaiting));
    }

    #[test]
    fn status_desconhecido_no_banco_vira_ausencia_de_estado_anterior() {
        // Uma linha antiga (ou corrompida) não pode produzir transição falsa.
        assert_eq!(parse_status("conectado"), None);
        assert_eq!(parse_status(""), None);
    }

    #[test]
    fn o_rotulo_gravado_bate_com_o_que_e_lido() {
        for status in [
            Status::Connected,
            Status::Unstable,
            Status::Disconnected,
            Status::Awaiting,
        ] {
            assert_eq!(parse_status(vpn_peer::status_label(status)), Some(status));
        }
    }
}
