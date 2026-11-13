//! Telemetria Wi-Fi: ler as estações e medir a latência até uma delas.
//!
//! Extrai, valida, delega (`services::wifi`) e serializa. As estações, os
//! aparelhos vigiados e as trocas de AP ao vivo não têm rota de consulta:
//! chegam pelo stream global (`wifi:snapshot`, `wifi:watched`,
//! `wifi:roamed`). As consultas daqui são sob demanda — o histórico de roaming
//! de um aparelho ao abrir o painel dele e as Configurações de Wi-Fi. Escrever
//! ou rodar ação no equipamento é de `operator`/`admin` pela regra padrão de
//! `users::request_is_allowed`.

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;

use crate::{
    self as wifi,
    dtos::{
        WifiPingInput, WifiRefreshInput, WifiRoamingQuery, WifiSettings, WifiWatchCreate,
        WifiWatchUpdate,
    },
};
use netmonitor_core::{
    audit::authenticated_user,
    shared::{body::optional_body, errors::AppResult},
};

/// `POST /api/wifi/refresh` — lê um equipamento (`deviceId`) ou todos. O
/// resultado chega pelo SSE; a resposta diz quem vai ser lido.
async fn refresh(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: String,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let input: WifiRefreshInput = optional_body(&body);
    let started = wifi::refresh(&ctx, input.device_id, Some(user.id)).await?;
    Ok((StatusCode::ACCEPTED, Json(started)).into_response())
}

/// `POST /api/wifi/ping` — latência do equipamento até o IP de uma estação.
async fn ping(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<WifiPingInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    Ok(format::json(
        wifi::ping(&ctx, &input, Some(user.id)).await?,
    )?)
}

/// `POST /api/wifi/watched` — passa a vigiar um aparelho (alerta se sumir).
async fn watch(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<WifiWatchCreate>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let created = wifi::watchlist::create(&ctx, &input, Some(user.id)).await?;
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

/// `PATCH /api/wifi/watched/{id}` — renomeia.
async fn rename(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<WifiWatchUpdate>,
) -> AppResult<Response> {
    authenticated_user(&ctx, &headers).await?;
    Ok(format::json(
        wifi::watchlist::rename(&ctx, id, &input).await?,
    )?)
}

/// `DELETE /api/wifi/watched/{id}` — deixa de vigiar e fecha o alerta aberto.
async fn unwatch(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    authenticated_user(&ctx, &headers).await?;
    wifi::watchlist::remove(&ctx, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `GET /api/wifi/roaming?mac=…` — as trocas de AP de um aparelho, da mais
/// recente para a mais antiga (o painel dele no mapa).
async fn roaming(
    State(ctx): State<AppContext>,
    Query(query): Query<WifiRoamingQuery>,
) -> AppResult<Response> {
    Ok(format::json(
        wifi::roaming::history(&ctx.db, &query.mac, query.limit).await?,
    )?)
}

/// `GET /api/wifi/settings` — as Configurações de Wi-Fi em vigor.
async fn get_settings(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(wifi::settings::load(&ctx.db).await?)?)
}

/// `PUT /api/wifi/settings` — grava as Configurações de Wi-Fi.
async fn put_settings(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<WifiSettings>,
) -> AppResult<Response> {
    authenticated_user(&ctx, &headers).await?;
    Ok(format::json(wifi::settings::save(&ctx.db, input).await?)?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/wifi")
        .add("/refresh", post(refresh))
        .add("/ping", post(ping))
        .add("/watched", post(watch))
        .add("/watched/{id}", patch(rename).delete(unwatch))
        .add("/roaming", get(roaming))
        .add("/settings", get(get_settings).put(put_settings))
}
