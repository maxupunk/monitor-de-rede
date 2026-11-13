//! Roaming: quando um aparelho troca de AP.
//!
//! Toda rodada de leitura (a do mapa aberto, a rotina em segundo plano, o
//! "Ler agora") passa por [`track`]: a rodada diz onde cada aparelho está
//! ([`detect::locate`]), e quem mudou de AP desde a rodada anterior vira uma
//! linha do histórico (`wifi_roaming_events`), um evento ao vivo
//! (`wifi:roamed`, que o mapa anima) e um fato para o motor de alertas — o
//! ping-pong (trocas demais na janela) é a regra `wifi_roaming_ping_pong`.
//!
//! Onde cada aparelho estava fica na memória do processo: depois de um
//! restart, a primeira rodada só reaprende a posição de cada um.

pub mod detect;
pub mod facts;

use std::{
    collections::{BTreeSet, HashMap},
    sync::{LazyLock, Mutex, PoisonError},
};

use chrono::{DateTime, Duration, FixedOffset, Utc};
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde_json::{json, Map};

use self::{
    detect::{Location, Roam, Seen},
    facts::WifiRoamingFacts,
};
use crate::{
    dtos::{WifiDeviceSnapshot, WifiRoamEvent, WifiSettings},
    settings,
};
use netmonitor_core::{
    alerts::contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey, AlertTarget},
    events::EventBus,
    models::{devices, wifi_roaming_events, wifi_watched_clients},
    network_tools::neighbor_cache::normalize_mac,
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};
use netmonitor_network::central::Central;

/// Um aparelho trocou de AP — o mapa anima.
pub const ROAMED_EVENT: &str = "wifi:roamed";

/// O histórico do painel: as trocas mais recentes primeiro.
const HISTORY_DEFAULT: u64 = 50;
const HISTORY_MAX: u64 = 200;

/// Quem esteve em ping-pong há menos que isto é reavaliado a cada rodada
/// até estabilizar (o alerta precisa saber que ele parou de pular).
const PING_PONG_MEMORY: Duration = Duration::hours(24);

/// Onde cada aparelho foi visto por último (MAC → AP).
static KNOWN: LazyLock<Mutex<HashMap<String, Seen>>> = LazyLock::new(Mutex::default);

/// Quem está em ping-pong, para reavaliar até sair. `None` até a primeira
/// rodada, que o recupera do banco (sobrevive a restart).
static FLAGGED: LazyLock<Mutex<Option<BTreeSet<String>>>> = LazyLock::new(Mutex::default);

/// Uma rodada de leitura terminou: grava as trocas de AP, publica e avalia o
/// ping-pong. Devolve as trocas gravadas.
///
/// # Errors
///
/// Erro do banco.
pub async fn track(
    ctx: &AppContext,
    round: &[WifiDeviceSnapshot],
) -> AppResult<Vec<WifiRoamEvent>> {
    let now = Utc::now();
    let current = detect::locate(round);
    let roams = {
        let mut known = KNOWN.lock().unwrap_or_else(PoisonError::into_inner);
        detect::detect(&mut known, &current, now)
    };
    let flagged = flagged(&ctx.db, now).await?;
    if roams.is_empty() && flagged.is_empty() {
        return Ok(Vec::new());
    }

    let settings = settings::load(&ctx.db).await?;
    let mut recorded = Vec::with_capacity(roams.len());
    for roam in &roams {
        let event = record(&ctx.db, roam, &settings, now).await?;
        publish(ctx, &event);
        recorded.push(event);
    }
    if !roams.is_empty() {
        Central::from_context(ctx)?
            .ensure_wifi_roaming_rules(&ctx.db)
            .await?;
    }

    // Quem trocou agora e quem estava em ping-pong: o veredito de cada um.
    let mut evaluate_macs: BTreeSet<String> = flagged;
    evaluate_macs.extend(roams.iter().map(|roam| roam.mac.clone()));
    for mac in evaluate_macs {
        let last = roams.iter().find(|roam| roam.mac == mac);
        let location = last.map(|roam| &roam.to).or_else(|| current.get(&mac));
        let ping_pong = evaluate(ctx, &mac, location, last, &settings, now).await?;
        let mut guard = FLAGGED.lock().unwrap_or_else(PoisonError::into_inner);
        let set = guard.get_or_insert_with(BTreeSet::new);
        if ping_pong {
            set.insert(mac);
        } else {
            set.remove(&mac);
        }
    }
    Ok(recorded)
}

/// Os aparelhos em ping-pong; na primeira vez, os do banco (últimas 24 h).
async fn flagged(db: &DatabaseConnection, now: DateTime<Utc>) -> AppResult<BTreeSet<String>> {
    if let Some(set) = FLAGGED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        return Ok(set.clone());
    }
    let since: DateTime<FixedOffset> = (now - PING_PONG_MEMORY).into();
    let macs: Vec<String> = wifi_roaming_events::Entity::find()
        .select_only()
        .column(wifi_roaming_events::Column::Mac)
        .distinct()
        .filter(wifi_roaming_events::Column::PingPong.eq(true))
        .filter(wifi_roaming_events::Column::CreatedAt.gte(since))
        .into_tuple()
        .all(db)
        .await?;
    let set: BTreeSet<String> = macs.into_iter().collect();
    *FLAGGED.lock().unwrap_or_else(PoisonError::into_inner) = Some(set.clone());
    Ok(set)
}

/// Quantas trocas o aparelho fez na janela até `now`.
async fn count_in_window<C: ConnectionTrait>(
    db: &C,
    mac: &str,
    settings: &WifiSettings,
    now: DateTime<Utc>,
) -> AppResult<u64> {
    let since: DateTime<FixedOffset> =
        (now - Duration::minutes(i64::from(settings.ping_pong_window_min))).into();
    Ok(wifi_roaming_events::Entity::find()
        .filter(wifi_roaming_events::Column::Mac.eq(mac))
        .filter(wifi_roaming_events::Column::CreatedAt.gte(since))
        .count(db)
        .await?)
}

/// Grava a troca; a contagem e a gravação na mesma transação, para duas
/// rodadas simultâneas não marcarem o ping-pong duas vezes.
async fn record(
    db: &DatabaseConnection,
    roam: &Roam,
    settings: &WifiSettings,
    now: DateTime<Utc>,
) -> AppResult<WifiRoamEvent> {
    let txn = begin_write(db).await?;
    let before = count_in_window(&txn, &roam.mac, settings, now).await?;
    let ping_pong = detect::is_ping_pong(before + 1, settings.ping_pong_count);
    let row = wifi_roaming_events::ActiveModel {
        mac: Set(roam.mac.clone()),
        from_device_id: Set(Some(roam.from.device_id)),
        from_device_name: Set(roam.from.device_name.clone()),
        to_device_id: Set(Some(roam.to.device_id)),
        to_device_name: Set(roam.to.device_name.clone()),
        signal_before_dbm: Set(Some(roam.from.signal_dbm)),
        signal_after_dbm: Set(Some(roam.to.signal_dbm)),
        ping_pong: Set(ping_pong),
        created_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    txn.commit().await?;
    Ok(to_dto(row))
}

fn publish(ctx: &AppContext, event: &WifiRoamEvent) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    match serde_json::to_value(event) {
        Ok(payload) => {
            bus.publish_ephemeral(ROAMED_EVENT, payload);
        }
        Err(error) => tracing::warn!(%error, "troca de AP não serializou"),
    }
}

/// Entrega o veredito do aparelho ao motor de alertas; devolve se está em
/// ping-pong.
async fn evaluate(
    ctx: &AppContext,
    mac: &str,
    location: Option<&Location>,
    last: Option<&Roam>,
    settings: &WifiSettings,
    now: DateTime<Utc>,
) -> AppResult<bool> {
    let count = count_in_window(&ctx.db, mac, settings, now).await?;
    let ping_pong = detect::is_ping_pong(count, settings.ping_pong_count);
    // O nome de vigiado vale mais que o do DHCP.
    let watched = wifi_watched_clients::Entity::find()
        .filter(wifi_watched_clients::Column::Mac.eq(mac))
        .one(&ctx.db)
        .await?;
    let label = watched
        .map(|client| client.name)
        .or_else(|| location.map(|location| location.label.clone()))
        .unwrap_or_else(|| format!("Aparelho {mac}"));
    let device_id = location.map(|location| location.device_id);
    let device = match device_id {
        Some(id) => devices::Entity::find_by_id(id).one(&ctx.db).await?,
        None => None,
    };
    let facts = WifiRoamingFacts {
        label: &label,
        count,
        window_min: settings.ping_pong_window_min,
        ping_pong,
        last: last.map(|roam| (roam.from.device_name.as_str(), roam.to.device_name.as_str())),
    };
    let mut data = Map::new();
    data.insert("mac".into(), json!(mac));
    let target = AlertTarget {
        scope: AlertEvaluationScope {
            site_id: device.as_ref().and_then(|device| device.site_id),
            device_id,
            monitor_id: None,
        },
        scope_key: AlertScopeKey::wifi_roaming(mac),
        label: label.clone(),
        data,
    };
    Central::from_context(ctx)?
        .evaluate_alert(ctx, &AlertEvaluationContext::of(&facts, target))
        .await?;
    Ok(ping_pong)
}

/// O histórico de roaming de um aparelho, do mais recente para o mais antigo.
///
/// # Errors
///
/// `422` com MAC inválido; erro do banco.
pub async fn history<C: ConnectionTrait>(
    db: &C,
    mac: &str,
    limit: Option<u32>,
) -> AppResult<Vec<WifiRoamEvent>> {
    let mac = normalize_mac(mac).ok_or_else(|| AppError::validation("MAC inválido."))?;
    let limit = limit
        .map_or(HISTORY_DEFAULT, u64::from)
        .clamp(1, HISTORY_MAX);
    Ok(wifi_roaming_events::Entity::find()
        .filter(wifi_roaming_events::Column::Mac.eq(mac))
        .order_by_desc(wifi_roaming_events::Column::CreatedAt)
        .order_by_desc(wifi_roaming_events::Column::Id)
        .limit(limit)
        .all(db)
        .await?
        .into_iter()
        .map(to_dto)
        .collect())
}

/// Apaga o histórico mais velho que a retenção configurada.
///
/// # Errors
///
/// Erro do banco.
pub async fn prune(db: &DatabaseConnection, now: DateTime<Utc>) -> AppResult<u64> {
    let days = settings::load(db).await?.roaming_retention_days;
    let cutoff: DateTime<FixedOffset> = (now - Duration::days(i64::from(days))).into();
    let deleted = wifi_roaming_events::Entity::delete_many()
        .filter(wifi_roaming_events::Column::CreatedAt.lt(cutoff))
        .exec(db)
        .await?;
    Ok(deleted.rows_affected)
}

fn to_dto(row: wifi_roaming_events::Model) -> WifiRoamEvent {
    WifiRoamEvent {
        id: row.id,
        mac: row.mac,
        from_device_id: row.from_device_id,
        from_device_name: row.from_device_name,
        to_device_id: row.to_device_id,
        to_device_name: row.to_device_name,
        signal_before_dbm: row.signal_before_dbm,
        signal_after_dbm: row.signal_after_dbm,
        occurred_at: row.created_at.to_rfc3339(),
        ping_pong: row.ping_pong,
    }
}

/// Esquece onde cada aparelho estava (testes de integração, que simulam
/// rodadas seguidas no mesmo processo).
pub fn forget() {
    KNOWN.lock().unwrap_or_else(PoisonError::into_inner).clear();
    *FLAGGED.lock().unwrap_or_else(PoisonError::into_inner) = None;
}
