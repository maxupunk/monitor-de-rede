//! Histórico de tráfego do túnel WireGuard (§8.10.2 / §9.2).
//!
//! Persistido em `metrics` — o mesmo modelo já usado para tráfego de interface
//! SNMP. Permite exibir um gráfico de RX/TX ao longo do tempo na aba VPN de
//! `/devices/:id`, e não apenas o contador acumulado exposto por `wg show dump`.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::json;

use crate::{
    models::{_entities::metrics as metrics_entity, metrics, monitors, vpn_peers, vpn_servers},
    services::{
        events::EventBus,
        shared::errors::AppResult,
        vpn::{
            config_writer::FileConfigSink, peer_hints::compute_peer_hints, peer_status,
            state_watcher,
        },
    },
};

pub const VPN_METRIC_BYTES_RX: &str = "vpn_bytes_rx";
pub const VPN_METRIC_BYTES_TX: &str = "vpn_bytes_tx";
pub const VPN_METRIC_RX_BPS: &str = "vpn_rx_bps";
pub const VPN_METRIC_TX_BPS: &str = "vpn_tx_bps";

/// Último quadro publicado por servidor, para não repetir um estado idêntico.
///
/// Um túnel parado produz exatamente o mesmo snapshot a cada ciclo de 10 s;
/// sem esta comparação, o SSE inundaria a tela com eventos que não repintam
/// nada.
fn last_published() -> &'static Mutex<HashMap<i64, String>> {
    static LAST: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Sincroniza o status de todos os servidores ativos e publica o quadro atual.
///
/// Separado de [`record_all`] de propósito: o status precisa de cadência fina
/// para a tela acompanhar o túnel em tempo real, enquanto o histórico de
/// tráfego não justifica quatro linhas em `metrics` por peer a cada ciclo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn sync_all(ctx: &AppContext) -> AppResult<usize> {
    let mut synced = 0;
    for server in active_servers(ctx).await? {
        sync_and_watch(ctx, &server).await;
        synced += publish_snapshot(ctx, server.id).await?;
    }
    Ok(synced)
}

/// Sincroniza o status e grava um snapshot de tráfego por peer.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn record_all(ctx: &AppContext) -> AppResult<usize> {
    let mut recorded = 0;
    for server in active_servers(ctx).await? {
        sync_and_watch(ctx, &server).await;
        let peers = enabled_peers(ctx, server.id).await?;
        for peer in &peers {
            record_peer(ctx, peer).await?;
        }
        publish_snapshot(ctx, server.id).await?;
        recorded += peers.len();
    }
    Ok(recorded)
}

async fn active_servers(ctx: &AppContext) -> AppResult<Vec<vpn_servers::Model>> {
    Ok(vpn_servers::Entity::find_active().all(&ctx.db).await?)
}

async fn enabled_peers(ctx: &AppContext, vpn_server_id: i64) -> AppResult<Vec<vpn_peers::Model>> {
    Ok(vpn_peers::Entity::find_enabled_for_server(vpn_server_id)
        .all(&ctx.db)
        .await?)
}

/// A avaliação de alertas não pode derrubar a coleta de telemetria: uma regra
/// mal formada ou uma notificação com falha deixaria o painel de VPN congelado,
/// que é justamente o oposto do que o alerta existe para evitar.
async fn sync_and_watch(ctx: &AppContext, server: &vpn_servers::Model) {
    if let Err(error) = peer_status::sync_peers(
        &ctx.db,
        &FileConfigSink::default(),
        &server.interface_name,
        server.id,
    )
    .await
    {
        tracing::warn!(%error, server_id = server.id, "falha ao sincronizar telemetria da VPN");
    }
    if let Err(error) = state_watcher::evaluate_server_peers(ctx, server.id).await {
        tracing::warn!(%error, server_id = server.id, "falha ao avaliar o estado dos túneis");
    }
}

/// Recalcula os mesmos avisos de firewall/ping do `GET /api/vpn/peers` para que
/// o snapshot publicado aqui nunca fique defasado em relação à carga inicial.
async fn publish_snapshot(ctx: &AppContext, vpn_server_id: i64) -> AppResult<usize> {
    let peers = enabled_peers(ctx, vpn_server_id).await?;
    if peers.is_empty() {
        return Ok(0);
    }

    let device_ids: Vec<i64> = peers.iter().map(|peer| peer.device_id).collect();
    let ping_monitors = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.is_in(device_ids))
        .filter(monitors::Column::Type.eq("ping"))
        .all(&ctx.db)
        .await?;

    let probe_external = super::probe_is_external();
    let snapshot: Vec<_> = peers
        .iter()
        .map(|peer| {
            let monitor = ping_monitors
                .iter()
                .find(|monitor| monitor.device_id == Some(peer.device_id));
            let hints = compute_peer_hints(peer, monitor, probe_external);
            json!({
                "id": peer.id,
                "deviceId": peer.device_id,
                "connectionStatus": peer.connection_status(),
                "lastHandshakeAt": peer.last_handshake_at.map(|value| value.to_rfc3339()),
                "lastSeenAt": peer.last_seen_at.map(|value| value.to_rfc3339()),
                "bytesRx": peer.bytes_rx,
                "bytesTx": peer.bytes_tx,
                "needsFirewallHint": hints.needs_firewall_hint,
                "pingOutsideTunnel": hints.ping_outside_tunnel,
                "pingMonitorId": hints.ping_monitor_id,
            })
        })
        .collect();

    let fingerprint = serde_json::to_string(&snapshot).unwrap_or_default();
    if let Ok(mut last) = last_published().lock() {
        if last.get(&vpn_server_id) == Some(&fingerprint) {
            return Ok(peers.len());
        }
        last.insert(vpn_server_id, fingerprint);
    }

    if let Ok(bus) = EventBus::from_context(ctx) {
        let payload = json!({ "vpnServerId": vpn_server_id, "peers": snapshot });
        if let Err(error) = bus.publish(&ctx.db, "vpn:peers_updated", payload).await {
            tracing::warn!(%error, "falha ao publicar vpn:peers_updated");
        }
    }
    Ok(peers.len())
}

async fn record_peer(ctx: &AppContext, peer: &vpn_peers::Model) -> AppResult<()> {
    let now = Utc::now();
    let last_rx = latest_metric(ctx, peer.device_id, VPN_METRIC_BYTES_RX).await?;
    let last_tx = latest_metric(ctx, peer.device_id, VPN_METRIC_BYTES_TX).await?;

    #[allow(clippy::cast_precision_loss)]
    let rx_bps = compute_rate(
        last_rx.as_ref().map(|(value, at)| (*value, *at)),
        peer.bytes_rx as f64,
        now,
    );
    #[allow(clippy::cast_precision_loss)]
    let tx_bps = compute_rate(
        last_tx.as_ref().map(|(value, at)| (*value, *at)),
        peer.bytes_tx as f64,
        now,
    );

    #[allow(clippy::cast_precision_loss)]
    for (name, value, unit) in [
        (VPN_METRIC_BYTES_RX, peer.bytes_rx as f64, "bytes"),
        (VPN_METRIC_BYTES_TX, peer.bytes_tx as f64, "bytes"),
        (VPN_METRIC_RX_BPS, rx_bps, "bps"),
        (VPN_METRIC_TX_BPS, tx_bps, "bps"),
    ] {
        metrics::ActiveModel {
            device_id: Set(peer.device_id),
            name: Set(name.into()),
            value: Set(value),
            unit: Set(unit.into()),
            recorded_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?;
    }
    Ok(())
}

async fn latest_metric(
    ctx: &AppContext,
    device_id: i64,
    name: &str,
) -> AppResult<Option<(f64, DateTime<Utc>)>> {
    Ok(metrics::Entity::find()
        .filter(metrics_entity::Column::DeviceId.eq(device_id))
        .filter(metrics_entity::Column::Name.eq(name))
        .order_by_desc(metrics_entity::Column::RecordedAt)
        .one(&ctx.db)
        .await?
        .map(|metric| (metric.value, metric.recorded_at.to_utc())))
}

/// Calcula bps a partir do delta de bytes acumulados, com o mesmo critério de
/// reset usado no coletor de tráfego SNMP.
fn compute_rate(previous: Option<(f64, DateTime<Utc>)>, current: f64, now: DateTime<Utc>) -> f64 {
    let Some((previous_value, previous_at)) = previous else {
        return 0.0;
    };
    #[allow(clippy::cast_precision_loss)]
    let elapsed_seconds = (now - previous_at).num_milliseconds() as f64 / 1_000.0;
    if elapsed_seconds <= 0.0 {
        return 0.0;
    }
    let delta = if current < previous_value {
        // Contador reiniciado (ex.: interface WireGuard subiu de novo) — assume
        // que o valor atual é o total acumulado desde o reinício.
        current
    } else {
        current - previous_value
    };
    (delta * 8.0 / elapsed_seconds).round()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn sem_amostra_anterior_a_taxa_e_zero() {
        assert_eq!(compute_rate(None, 1_000.0, Utc::now()), 0.0);
    }

    #[test]
    fn a_taxa_vem_do_delta_em_bits_por_segundo() {
        let now = Utc::now();
        // 1.000 bytes em 10 s = 800 bps.
        let rate = compute_rate(Some((0.0, now - Duration::seconds(10))), 1_000.0, now);
        assert_eq!(rate, 800.0);
    }

    #[test]
    fn contador_reiniciado_usa_o_valor_atual_como_delta() {
        let now = Utc::now();
        // Caiu de 10.000 para 400: a interface subiu de novo.
        let rate = compute_rate(Some((10_000.0, now - Duration::seconds(10))), 400.0, now);
        assert_eq!(rate, 320.0);
    }

    #[test]
    fn amostras_no_mesmo_instante_nao_produzem_taxa_infinita() {
        let now = Utc::now();
        assert_eq!(compute_rate(Some((0.0, now)), 1_000.0, now), 0.0);
        // Relógio que andou para trás também não pode gerar taxa negativa.
        assert_eq!(
            compute_rate(Some((0.0, now + Duration::seconds(5))), 1_000.0, now),
            0.0
        );
    }
}
