//! Conversas salvas do Assistente IA, sempre do usuário autenticado.

use axum::http::{HeaderMap, StatusCode};
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::authenticated_user,
    dtos::ai::AiConversationInput,
    services::{ai::conversations, shared::errors::AppResult},
};

/// `GET /api/ai/conversations` — lista do usuário, sem as mensagens.
async fn index(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    Ok(format::json(conversations::list(&ctx.db, user.id).await?)?)
}

/// `GET /api/ai/conversations/{id}` — conversa completa.
async fn show(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    Ok(format::json(
        conversations::get(&ctx.db, user.id, id).await?,
    )?)
}

/// `POST /api/ai/conversations` — cria e devolve o resumo com o id.
async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<AiConversationInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let created = conversations::create(&ctx.db, user.id, input).await?;
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

/// `PUT /api/ai/conversations/{id}` — substitui título e mensagens.
async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<AiConversationInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    Ok(format::json(
        conversations::update(&ctx.db, user.id, id, input).await?,
    )?)
}

/// `DELETE /api/ai/conversations/{id}`.
async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    conversations::delete(&ctx.db, user.id, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/ai/conversations")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
}
