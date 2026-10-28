//! Os aparelhos vigiados: cadastro, estado e a lista no SSE (`wifi:watched`).
//!
//! Vigiar é opt-in, por MAC: só câmera, IoT e o que mais deveria estar sempre
//! conectado. Celular que sai do local nunca entra aqui — e por isso nunca vira
//! alerta. O primeiro aparelho vigiado instala as regras de alerta de Wi-Fi.

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, Utc};
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use super::{collector, watch};
use crate::{
    dtos::wifi::{
        WifiWatchCreate, WifiWatchState, WifiWatchUpdate, WifiWatchedClient, WifiWatchedSnapshot,
    },
    models::{devices, wifi_watched_clients},
    services::{
        alerts::{catalog::once, contracts::AlertScopeKey, recovery},
        events::EventBus,
        network_tools::neighbor_cache::normalize_mac,
        shared::errors::{AppError, AppResult},
        vendors::builtin::is_locally_administered,
    },
};

pub const WATCHED_EVENT: &str = "wifi:watched";
const SNAPSHOT_KEY: &str = "wifi:watched";
const NAME_MAX: usize = 120;

const STATE_CONNECTED: &str = "connected";
const STATE_OFFLINE: &str = "offline";

/// Todos os vigiados, em ordem de nome.
///
/// # Errors
///
/// Erro do banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<WifiWatchedClient>> {
    let models = wifi_watched_clients::Entity::find()
        .order_by_asc(wifi_watched_clients::Column::Name)
        .all(db)
        .await?;
    let device_ids: Vec<i64> = models
        .iter()
        .filter_map(|model| model.last_device_id)
        .collect();
    let names: HashMap<i64, String> = devices::Entity::find()
        .filter(devices::Column::Id.is_in(device_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|device| (device.id, device.name))
        .collect();
    Ok(models
        .into_iter()
        .map(|model| {
            let device_name = model.last_device_id.and_then(|id| names.get(&id).cloned());
            to_dto(model, device_name)
        })
        .collect())
}

/// Passa a vigiar um aparelho. Se a tela já o mostra conectado, ele nasce
/// conectado — sem esperar o próximo ciclo.
///
/// # Errors
///
/// MAC ou nome inválidos (422); aparelho já vigiado (409); erro do banco.
pub async fn create(
    ctx: &AppContext,
    input: &WifiWatchCreate,
    user_id: Option<i64>,
) -> AppResult<WifiWatchedClient> {
    let mac = normalize_mac(&input.mac)
        .ok_or_else(|| AppError::validation("Informe um MAC válido, como aa:bb:cc:dd:ee:ff."))?;
    let name = valid_name(&input.name)?;
    if let Some(existing) = find_by_mac(&ctx.db, &mac).await? {
        return Err(AppError::conflict(format!(
            "Este aparelho já é vigiado como \"{}\".",
            existing.name
        )));
    }

    let now: DateTime<FixedOffset> = Utc::now().into();
    let snapshots = collector::current_snapshots(ctx);
    let seen = watch::find_station(&snapshots, &mac).map(|(snapshot, _)| snapshot);
    let model = wifi_watched_clients::ActiveModel {
        mac: Set(mac.clone()),
        name: Set(name),
        last_state: Set(seen.map(|_| STATE_CONNECTED.to_owned())),
        last_seen_at: Set(seen.map(|snapshot| {
            snapshot
                .read_at
                .as_deref()
                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                .unwrap_or(now)
        })),
        last_device_id: Set(seen.map(|snapshot| snapshot.device_id)),
        created_by: Set(user_id),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    // As regras de Wi-Fi entram com o primeiro vigiado, uma vez na vida.
    once::ensure_wifi_watch(&ctx.db).await?;
    publish(ctx).await;
    let device_name = seen.map(|snapshot| snapshot.device_name.clone());
    Ok(to_dto(model, device_name))
}

/// Renomeia um aparelho vigiado.
///
/// # Errors
///
/// Nome inválido (422); aparelho não vigiado (404); erro do banco.
pub async fn rename(
    ctx: &AppContext,
    id: i64,
    input: &WifiWatchUpdate,
) -> AppResult<WifiWatchedClient> {
    let name = valid_name(&input.name)?;
    let mut model: wifi_watched_clients::ActiveModel = find(&ctx.db, id).await?.into();
    model.name = Set(name);
    let model = model.update(&ctx.db).await?;
    publish(ctx).await;
    let device_name = match model.last_device_id {
        Some(device_id) => devices::Entity::find_by_id(device_id)
            .one(&ctx.db)
            .await?
            .map(|device| device.name),
        None => None,
    };
    Ok(to_dto(model, device_name))
}

/// Deixa de vigiar: o alerta aberto do aparelho fecha na hora — não há mais
/// quem observe a volta dele.
///
/// # Errors
///
/// Aparelho não vigiado (404); erro do banco.
pub async fn remove(ctx: &AppContext, id: i64) -> AppResult<()> {
    let model = find(&ctx.db, id).await?;
    recovery::close_scope(
        ctx,
        &AlertScopeKey::wifi_client(model.id),
        "O aparelho deixou de ser vigiado.",
    )
    .await?;
    wifi_watched_clients::Entity::delete_by_id(model.id)
        .exec(&ctx.db)
        .await?;
    publish(ctx).await;
    Ok(())
}

/// Grava o que a leitura viu: conectado em `device_id` agora (`Some`), ou fora.
///
/// # Errors
///
/// Erro do banco.
pub async fn record<C: ConnectionTrait>(
    db: &C,
    model: &wifi_watched_clients::Model,
    connected_on: Option<i64>,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let mut active: wifi_watched_clients::ActiveModel = model.clone().into();
    match connected_on {
        Some(device_id) => {
            active.last_state = Set(Some(STATE_CONNECTED.to_owned()));
            active.last_seen_at = Set(Some(now.into()));
            active.last_device_id = Set(Some(device_id));
        }
        None => active.last_state = Set(Some(STATE_OFFLINE.to_owned())),
    }
    active.update(db).await?;
    Ok(())
}

/// Publica a lista inteira no SSE; quem conectar depois recebe a última.
pub async fn publish(ctx: &AppContext) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    let snapshot = match list(&ctx.db).await {
        Ok(clients) => WifiWatchedSnapshot { clients },
        Err(error) => {
            tracing::warn!(%error, "lista de aparelhos vigiados não carregou");
            return;
        }
    };
    match serde_json::to_value(snapshot) {
        Ok(payload) => {
            bus.publish_snapshot(WATCHED_EVENT, SNAPSHOT_KEY, payload);
        }
        Err(error) => tracing::warn!(%error, "lista de aparelhos vigiados não serializou"),
    }
}

async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<wifi_watched_clients::Model> {
    wifi_watched_clients::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Este aparelho não está sendo vigiado."))
}

async fn find_by_mac<C: ConnectionTrait>(
    db: &C,
    mac: &str,
) -> AppResult<Option<wifi_watched_clients::Model>> {
    Ok(wifi_watched_clients::Entity::find()
        .filter(wifi_watched_clients::Column::Mac.eq(mac))
        .one(db)
        .await?)
}

fn valid_name(raw: &str) -> AppResult<String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(AppError::validation(
            "Dê um nome ao aparelho, como \"Câmera da portaria\".",
        ));
    }
    if name.chars().count() > NAME_MAX {
        return Err(AppError::validation(format!(
            "O nome pode ter até {NAME_MAX} caracteres."
        )));
    }
    Ok(name.to_owned())
}

fn state_of(raw: Option<&str>) -> WifiWatchState {
    match raw {
        Some(STATE_CONNECTED) => WifiWatchState::Connected,
        Some(STATE_OFFLINE) => WifiWatchState::Offline,
        _ => WifiWatchState::Pending,
    }
}

fn to_dto(model: wifi_watched_clients::Model, device_name: Option<String>) -> WifiWatchedClient {
    WifiWatchedClient {
        id: model.id,
        randomized_mac: is_locally_administered(&model.mac),
        state: state_of(model.last_state.as_deref()),
        device_id: model.last_device_id,
        device_name,
        last_seen_at: model.last_seen_at.map(|at| at.to_rfc3339()),
        mac: model.mac,
        name: model.name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_vazio_ou_longo_demais_e_recusado() {
        assert_eq!(valid_name("  Câmera  ").unwrap(), "Câmera");
        assert!(valid_name("   ").is_err());
        assert!(valid_name(&"x".repeat(NAME_MAX + 1)).is_err());
    }

    #[test]
    fn estado_desconhecido_e_aguardando_leitura() {
        assert_eq!(state_of(Some("connected")), WifiWatchState::Connected);
        assert_eq!(state_of(Some("offline")), WifiWatchState::Offline);
        assert_eq!(state_of(None), WifiWatchState::Pending);
    }
}
