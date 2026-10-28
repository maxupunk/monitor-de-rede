//! A leitura das estações: um equipamento ou todos, em segundo plano.
//!
//! O pedido responde na hora com quem vai ser lido; cada equipamento sai
//! pelo SSE (`wifi:snapshot`) primeiro como "lendo" — com as estações da
//! leitura anterior — e de novo quando a leitura termina. O snapshot fica no
//! barramento: quem abre a tela depois recebe o último estado ao conectar,
//! sem consulta HTTP (AGENTS §9).
//!
//! Um equipamento é lido por um pedido de cada vez: dois cliques (ou duas
//! abas, ou o vigia dos clientes vigiados) não abrem duas sessões SSH no
//! mesmo AP.

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock, PoisonError},
};

use chrono::Utc;
use futures::StreamExt;
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use super::{
    contract,
    source::{self, Requester, WifiSource},
    station,
};
use crate::{
    dtos::wifi::{WifiDeviceSnapshot, WifiReadStatus, WifiRefreshStarted, WifiStation},
    models::devices,
    services::{
        events::EventBus,
        plugins::{actions, fleet},
        shared::errors::{AppError, AppResult},
        vendors,
    },
};

pub const SNAPSHOT_EVENT: &str = "wifi:snapshot";

/// A frota lida de ponta a ponta (o vigia espera o resultado).
pub struct FleetReading {
    pub snapshots: Vec<WifiDeviceSnapshot>,
    /// Todos os equipamentos com plugin Wi-Fi foram lidos com sucesso. Só
    /// assim a ausência de um aparelho prova que ele saiu do Wi-Fi.
    pub complete: bool,
}

/// Um equipamento a ler.
struct Target {
    device: devices::Model,
    source: WifiSource,
}

/// Começa a leitura de um equipamento (`Some`) ou de todos (`None`).
///
/// # Errors
///
/// Equipamento inexistente ou sem plugin Wi-Fi ativo; erro do banco.
pub async fn refresh(
    ctx: &AppContext,
    device_id: Option<i64>,
    user_id: Option<i64>,
) -> AppResult<WifiRefreshStarted> {
    let targets = match device_id {
        Some(id) => vec![target(ctx, id).await?],
        None => every_target(ctx).await?,
    };
    let device_ids: Vec<i64> = targets.iter().map(|target| target.device.id).collect();
    if device_id.is_none() {
        forget_others(ctx, &device_ids);
    }

    let claimed = claim(ctx, targets);
    if !claimed.is_empty() {
        let ctx = ctx.clone();
        tokio::spawn(async move { read_all(&ctx, claimed, Requester::user(user_id)).await });
    }
    Ok(WifiRefreshStarted { device_ids })
}

/// Lê todos os equipamentos com plugin Wi-Fi e espera o fim. Cada um também
/// sai pelo SSE, como na leitura pela tela.
///
/// # Errors
///
/// Erro do banco ao listar os equipamentos.
pub async fn read_fleet(ctx: &AppContext, requester: Requester) -> AppResult<FleetReading> {
    let targets = every_target(ctx).await?;
    let device_ids: Vec<i64> = targets.iter().map(|target| target.device.id).collect();
    forget_others(ctx, &device_ids);

    let snapshots = read_all(ctx, claim(ctx, targets), requester).await;
    let complete = !device_ids.is_empty()
        && snapshots.len() == device_ids.len()
        && snapshots
            .iter()
            .all(|snapshot| snapshot.status == WifiReadStatus::Ok);
    Ok(FleetReading {
        snapshots,
        complete,
    })
}

/// Reserva a vez de cada equipamento (quem já está sendo lido fica de fora) e
/// avisa a tela de que a leitura começou.
fn claim(ctx: &AppContext, targets: Vec<Target>) -> Vec<(Target, ReadingGuard)> {
    let claimed: Vec<(Target, ReadingGuard)> = targets
        .into_iter()
        .filter_map(|target| ReadingGuard::claim(target.device.id).map(|guard| (target, guard)))
        .collect();
    for (target, _) in &claimed {
        publish(ctx, &reading(ctx, target));
    }
    claimed
}

async fn target(ctx: &AppContext, device_id: i64) -> AppResult<Target> {
    let device = actions::device(ctx, device_id).await?;
    let source = source::for_device(&ctx.db, device_id)
        .await?
        .ok_or_else(|| {
            AppError::business_rule(format!(
                "{} não tem um plugin de Wi-Fi ativo. Instale um na aba Plugins do equipamento.",
                device.name
            ))
        })?;
    Ok(Target { device, source })
}

async fn every_target(ctx: &AppContext) -> AppResult<Vec<Target>> {
    let mut sources = source::all(&ctx.db).await?;
    let devices = devices::Entity::find()
        .filter(devices::Column::Id.is_in(sources.keys().copied().collect::<Vec<_>>()))
        .all(&ctx.db)
        .await?;
    Ok(devices
        .into_iter()
        .filter_map(|device| {
            sources
                .remove(&device.id)
                .map(|source| Target { device, source })
        })
        .collect())
}

async fn read_all(
    ctx: &AppContext,
    targets: Vec<(Target, ReadingGuard)>,
    requester: Requester,
) -> Vec<WifiDeviceSnapshot> {
    // O fabricante de cada MAC sai da memória do processo.
    vendors::service::ensure_loaded(&ctx.db).await;
    futures::stream::iter(targets)
        .map(|(target, guard)| async move {
            let snapshot = read(ctx, &target, requester).await;
            publish(ctx, &snapshot);
            drop(guard);
            snapshot
        })
        .buffer_unordered(fleet::CONCURRENCY)
        .collect()
        .await
}

async fn read(ctx: &AppContext, target: &Target, requester: Requester) -> WifiDeviceSnapshot {
    let (status, error, stations) = match stations(ctx, target, requester).await {
        Ok(stations) => (WifiReadStatus::Ok, None, stations),
        Err(error) => {
            tracing::warn!(%error, device_id = target.device.id, "falha ao ler os clientes Wi-Fi");
            (WifiReadStatus::Failed, Some(error.to_string()), Vec::new())
        }
    };
    snapshot(
        target,
        status,
        Some(Utc::now().to_rfc3339()),
        error,
        stations,
    )
}

async fn stations(
    ctx: &AppContext,
    target: &Target,
    requester: Requester,
) -> AppResult<Vec<WifiStation>> {
    let Target { device, source } = target;
    let output = source
        .run(
            ctx,
            device.id,
            &source.roles.clients,
            json!({}),
            requester,
            "Leitura dos clientes Wi-Fi".into(),
        )
        .await?;
    let readings = contract::parse_clients(output).map_err(|error| {
        AppError::service_unavailable(format!("O plugin `{}`: {error}.", source.slug))
    })?;
    let mut stations: Vec<_> = readings.into_iter().filter_map(station::build).collect();
    station::sort_worst_first(&mut stations);
    Ok(stations)
}

/// O estado "lendo", com o que a leitura anterior trouxe.
fn reading(ctx: &AppContext, target: &Target) -> WifiDeviceSnapshot {
    let previous = EventBus::from_context(ctx)
        .ok()
        .and_then(|bus| previous(&bus, target.device.id));
    let (read_at, stations) = previous
        .map(|snapshot| (snapshot.read_at, snapshot.stations))
        .unwrap_or_default();
    snapshot(target, WifiReadStatus::Reading, read_at, None, stations)
}

fn snapshot(
    target: &Target,
    status: WifiReadStatus,
    read_at: Option<String>,
    error: Option<String>,
    stations: Vec<WifiStation>,
) -> WifiDeviceSnapshot {
    WifiDeviceSnapshot {
        device_id: target.device.id,
        device_name: target.device.name.clone(),
        status,
        read_at,
        error,
        stations,
        can_ping: target.source.roles.ping.is_some(),
    }
}

fn snapshot_key(device_id: i64) -> String {
    format!("wifi:{device_id}")
}

/// Os snapshots Wi-Fi guardados no barramento.
fn stored(bus: &EventBus) -> impl Iterator<Item = WifiDeviceSnapshot> {
    bus.current_snapshots()
        .into_iter()
        .filter(|event| event.event_type == SNAPSHOT_EVENT)
        .filter_map(|event| serde_json::from_value(event.payload).ok())
}

/// O último estado de cada equipamento, como a tela o vê.
pub(super) fn current_snapshots(ctx: &AppContext) -> Vec<WifiDeviceSnapshot> {
    EventBus::from_context(ctx)
        .map(|bus| stored(&bus).collect())
        .unwrap_or_default()
}

fn previous(bus: &EventBus, device_id: i64) -> Option<WifiDeviceSnapshot> {
    stored(bus).find(|snapshot| snapshot.device_id == device_id)
}

/// Equipamento que perdeu o plugin Wi-Fi não volta a quem conectar depois.
fn forget_others(ctx: &AppContext, keep: &[i64]) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    let gone: Vec<i64> = stored(&bus)
        .map(|snapshot| snapshot.device_id)
        .filter(|device_id| !keep.contains(device_id))
        .collect();
    for device_id in gone {
        bus.forget_snapshots(&snapshot_key(device_id));
    }
}

fn publish(ctx: &AppContext, snapshot: &WifiDeviceSnapshot) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    match serde_json::to_value(snapshot) {
        Ok(payload) => {
            bus.publish_snapshot(SNAPSHOT_EVENT, snapshot_key(snapshot.device_id), payload);
        }
        Err(error) => tracing::warn!(%error, "snapshot Wi-Fi não serializou"),
    }
}

fn in_flight() -> &'static Mutex<HashSet<i64>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(Mutex::default)
}

/// A vez de ler um equipamento; ao sair de escopo (fim ou pânico), libera.
struct ReadingGuard(i64);

impl ReadingGuard {
    /// `None` quando outro pedido já está lendo o equipamento.
    fn claim(device_id: i64) -> Option<Self> {
        // O guarda só nasce depois de soltar o mutex: criado e descartado com
        // ele travado, o `Drop` travaria de novo (e liberaria a vez de outro).
        let claimed = in_flight()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(device_id);
        claimed.then(|| Self(device_id))
    }
}

impl Drop for ReadingGuard {
    fn drop(&mut self) {
        in_flight()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn um_equipamento_e_lido_por_um_pedido_de_cada_vez() {
        let primeiro = ReadingGuard::claim(-41).expect("livre");
        assert!(ReadingGuard::claim(-41).is_none(), "já está sendo lido");
        drop(primeiro);
        assert!(ReadingGuard::claim(-41).is_some(), "liberado ao terminar");
    }
}
