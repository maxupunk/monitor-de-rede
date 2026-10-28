//! Telemetria Wi-Fi: ler as estações e medir a latência até uma delas.
//!
//! Extrai, valida, delega (`services::wifi`) e serializa. As estações e os
//! aparelhos vigiados não têm rota de consulta: chegam pelo stream global
//! (`wifi:snapshot`, `wifi:watched`). Toda rota daqui escreve ou roda ação no
//! equipamento — `operator`/`admin` pela regra padrão de
//! `users::request_is_allowed`.

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::authenticated_user,
    dtos::{
        optional_body,
        wifi::{WifiPingInput, WifiRefreshInput, WifiWatchCreate, WifiWatchUpdate},
    },
    services::{shared::errors::AppResult, wifi},
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/wifi")
        .add("/refresh", post(refresh))
        .add("/ping", post(ping))
        .add("/watched", post(watch))
        .add("/watched/{id}", patch(rename).delete(unwatch))
}
