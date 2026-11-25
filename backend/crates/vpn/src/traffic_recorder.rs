//! Histórico de tráfego do túnel WireGuard (§8.10.2 / §9.2).
//!
//! Persistido em `metrics` — o mesmo modelo já usado para tráfego de interface
//! SNMP. Permite exibir um gráfico de RX/TX ao longo do tempo na aba VPN de
//! `/devices/:id`, e não apenas o contador acumulado exposto por `wg show dump`.

use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock, PoisonError},
};

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::json;

use netmonitor_core::{
    events::EventBus,
    models::{_entities::metrics as metrics_entity, metrics, monitors, vpn_peers, vpn_servers},
    shared::{errors::AppResult, transaction::begin_write},
};

use crate::{
    alerts::watcher, config_writer::FileConfigSink, peer_hints::compute_peer_hints, peer_status,
};

pub const VPN_METRIC_BYTES_RX: &str = "vpn_bytes_rx";
pub const VPN_METRIC_BYTES_TX: &str = "vpn_bytes_tx";
pub const VPN_METRIC_RX_BPS: &str = "vpn_rx_bps";
pub const VPN_METRIC_TX_BPS: &str = "vpn_tx_bps";
pub const PEERS_UPDATED_EVENT: &str = "vpn:peers_updated";

/// Lote de linhas inseridas por vez em `metrics` (mantém parâmetros abaixo do
/// teto de 999 variáveis do SQLite: 100 linhas × 5 colunas = 500 variáveis).
const METRICS_INSERT_CHUNK: usize = 100;

fn snapshot_key(vpn_server_id: i64) -> String {
    format!("vpn:peers:{vpn_server_id}")
}

/// Último quadro publicado por servidor, para não repetir um estado idêntico.
///
/// Um túnel parado produz exatamente o mesmo snapshot a cada ciclo de 10 s;
/// sem esta comparação, o SSE inundaria a tela com eventos que não repintam
/// nada.
fn last_published() -> &'static Mutex<HashMap<i64, String>> {
    static LAST: OnceLock<Mutex<HashMap<i64, String>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Última amostra de bytes gravada por dispositivo em memória, evitando `2*N`
/// consultas em `metrics` a cada 30 s no processo único.
#[derive(Clone, Copy)]
struct LastCounters {
    rx: f64,
    tx: f64,
    at: DateTime<Utc>,
}

fn last_counters() -> &'static Mutex<HashMap<i64, LastCounters>> {
    static COUNTERS: OnceLock<Mutex<HashMap<i64, LastCounters>>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Invalida o cache do último snapshot publicado e avisa as abas conectadas
/// pelo SSE efêmero (sem gravar em `event_outbox`) que a lista de peers mudou.
///
/// Esquece também o snapshot guardado no barramento: sem isso, a aba aberta
/// logo depois da mudança receberia, ao conectar, a lista de antes dela.
pub fn notify_peers_changed(ctx: &AppContext) {
    let bus = EventBus::from_context(ctx).ok();
    forget_published(bus.as_ref());
    if let Some(bus) = bus {
        bus.publish_ephemeral(PEERS_UPDATED_EVENT, json!({}));
    }
}

/// Esvazia o cache de quadros publicados e tira do barramento o snapshot de
/// cada servidor que estava nele.
fn forget_published(bus: Option<&EventBus>) {
    let servers: Vec<i64> = last_published()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .drain()
        .map(|(server_id, _)| server_id)
        .collect();
    if let Some(bus) = bus {
        for server_id in servers {
            bus.forget_snapshots(&snapshot_key(server_id));
        }
    }
}

/// Despeja da memória a última amostra de quem não foi lido nesta passada.
fn evict_absent_counters(present: &HashSet<i64>) {
    last_counters()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|device_id, _| present.contains(device_id));
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
        let peers = sync_and_watch(ctx, &server).await?;
        synced += publish_snapshot(ctx, server.id, &peers).await?;
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
    let mut present = HashSet::new();
    for server in active_servers(ctx).await? {
        let peers = sync_and_watch(ctx, &server).await?;
        record_peers(ctx, &peers).await?;
        publish_snapshot(ctx, server.id, &peers).await?;
        present.extend(peers.iter().map(|peer| peer.device_id));
        recorded += peers.len();
    }
    // Peer revogado (ou servidor desativado) não volta a ser lido: sem o
    // despejo, a última amostra dele ficaria na memória para sempre.
    evict_absent_counters(&present);
    Ok(recorded)
}

async fn active_servers(ctx: &AppContext) -> AppResult<Vec<vpn_servers::Model>> {
    Ok(vpn_servers::Entity::find_active().all(&ctx.db).await?)
}

/// A avaliação de alertas não pode derrubar a coleta de telemetria: uma regra
/// mal formada ou uma notificação com falha deixaria o painel de VPN congelado,
/// que é justamente o oposto do que o alerta existe para evitar.
async fn sync_and_watch(
    ctx: &AppContext,
    server: &vpn_servers::Model,
) -> AppResult<Vec<vpn_peers::Model>> {
    let peers = match peer_status::sync_peers_loaded(
        &ctx.db,
        &FileConfigSink::default(),
        &server.interface_name,
        server.id,
    )
    .await
    {
        Ok((_, peers)) => peers,
        Err(error) => {
            tracing::warn!(%error, server_id = server.id, "falha ao sincronizar telemetria da VPN");
            vpn_peers::Entity::find_enabled_for_server(server.id)
                .all(&ctx.db)
                .await?
        }
    };
    if let Err(error) = watcher::evaluate_server_peers_loaded(ctx, peers.clone()).await {
        tracing::warn!(%error, server_id = server.id, "falha ao avaliar o estado dos túneis");
    }
    Ok(peers)
}

/// Recalcula os mesmos avisos de firewall/ping do `GET /api/vpn/peers` e
/// publica como snapshot em memória no barramento SSE (AGENTS §9).
///
/// O snapshot só chega a quem assina **neste** processo. O ciclo pode rodar à
/// parte (`SCHEDULER_ENABLED=false` + `task scheduler_loop`), num processo
/// sem cliente SSE: aí o quadro que mudou também vai para o `event_outbox`,
/// que o relay do servidor HTTP repassa. Com assinante local, nada é gravado.
async fn publish_snapshot(
    ctx: &AppContext,
    vpn_server_id: i64,
    peers: &[vpn_peers::Model],
) -> AppResult<usize> {
    if peers.is_empty() {
        last_published()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&vpn_server_id);
        if let Ok(bus) = EventBus::from_context(ctx) {
            bus.forget_snapshots(&snapshot_key(vpn_server_id));
        }
        return Ok(0);
    }

    let device_ids: Vec<i64> = peers.iter().map(|peer| peer.device_id).collect();
    let ping_monitors: HashMap<i64, monitors::Model> = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.is_in(device_ids))
        .filter(monitors::Column::Type.eq("ping"))
        .all(&ctx.db)
        .await?
        .into_iter()
        .filter_map(|monitor| monitor.device_id.map(|id| (id, monitor)))
        .collect();

    let probe_external = super::probe_is_external();
    let snapshot: Vec<_> = peers
        .iter()
        .map(|peer| {
            let monitor = ping_monitors.get(&peer.device_id);
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
    {
        let mut last = last_published()
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if last.get(&vpn_server_id) == Some(&fingerprint) {
            return Ok(peers.len());
        }
        last.insert(vpn_server_id, fingerprint);
    }

    if let Ok(bus) = EventBus::from_context(ctx) {
        let payload = json!({ "vpnServerId": vpn_server_id, "peers": snapshot });
        let relay_to_http = !bus.has_subscribers();
        let event = bus.publish_snapshot(PEERS_UPDATED_EVENT, snapshot_key(vpn_server_id), payload);
        if relay_to_http {
            // Sem receptor local, o `publish_local` do `publish` não entrega
            // nada aqui: a linha existe só para o relay do outro processo.
            if let Err(error) = bus.publish(&ctx.db, event.event_type, event.payload).await {
                tracing::warn!(%error, "falha ao gravar vpn:peers_updated no outbox");
            }
        }
    }
    Ok(peers.len())
}

async fn record_peers(ctx: &AppContext, peers: &[vpn_peers::Model]) -> AppResult<()> {
    if peers.is_empty() {
        return Ok(());
    }
    let now = Utc::now();
    let cached: HashMap<i64, LastCounters> = last_counters()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();

    let mut rows = Vec::with_capacity(peers.len() * 4);
    let mut updated_cache = Vec::with_capacity(peers.len());

    for peer in peers {
        let (last_rx, last_tx) = if let Some(prev) = cached.get(&peer.device_id) {
            (Some((prev.rx, prev.at)), Some((prev.tx, prev.at)))
        } else {
            (
                latest_metric(ctx, peer.device_id, VPN_METRIC_BYTES_RX).await?,
                latest_metric(ctx, peer.device_id, VPN_METRIC_BYTES_TX).await?,
            )
        };

        #[allow(clippy::cast_precision_loss)]
        let current_rx = peer.bytes_rx as f64;
        #[allow(clippy::cast_precision_loss)]
        let current_tx = peer.bytes_tx as f64;
        let rx_bps = compute_rate(last_rx, current_rx, now);
        let tx_bps = compute_rate(last_tx, current_tx, now);

        for (name, value, unit) in [
            (VPN_METRIC_BYTES_RX, current_rx, "bytes"),
            (VPN_METRIC_BYTES_TX, current_tx, "bytes"),
            (VPN_METRIC_RX_BPS, rx_bps, "bps"),
            (VPN_METRIC_TX_BPS, tx_bps, "bps"),
        ] {
            rows.push(metrics::ActiveModel {
                device_id: Set(peer.device_id),
                name: Set(name.into()),
                value: Set(value),
                unit: Set(unit.into()),
                recorded_at: Set(now.into()),
                ..Default::default()
            });
        }
        updated_cache.push((
            peer.device_id,
            LastCounters {
                rx: current_rx,
                tx: current_tx,
                at: now,
            },
        ));
    }

    let txn = begin_write(&ctx.db).await?;
    for chunk in rows.chunks(METRICS_INSERT_CHUNK) {
        metrics::Entity::insert_many(chunk.to_vec())
            .exec(&txn)
            .await?;
    }
    txn.commit().await?;

    let mut cache = last_counters()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    for (device_id, counters) in updated_cache {
        cache.insert(device_id, counters);
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
    #[serial_test::serial]
    fn mudanca_de_peers_esquece_o_snapshot_guardado_no_barramento() {
        let bus = EventBus::create();
        bus.publish_snapshot(
            PEERS_UPDATED_EVENT,
            snapshot_key(-7),
            json!({ "peers": [1] }),
        );
        last_published()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(-7, "quadro".into());

        forget_published(Some(&bus));

        assert!(
            bus.current_snapshots().is_empty(),
            "a aba aberta depois não pode receber a lista de antes"
        );
        assert!(last_published()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn amostra_de_peer_revogado_sai_da_memoria() {
        let sample = LastCounters {
            rx: 1.0,
            tx: 2.0,
            at: Utc::now(),
        };
        {
            let mut cache = last_counters()
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            cache.insert(-1, sample);
            cache.insert(-2, sample);
        }
        evict_absent_counters(&HashSet::from([-1]));
        let cache = last_counters()
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        assert!(cache.contains_key(&-1));
        assert!(!cache.contains_key(&-2));
    }

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
