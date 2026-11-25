//! O que é do próprio usuário (`/api/me`): o menu (Fase 16) e o aviso do
//! navegador que ficou sem resposta. Todo papel mexe aqui — nada disso muda o
//! sistema.

use axum::http::{HeaderMap, StatusCode};
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::authenticated_user,
    dtos::client_failures::ClientFailureInput,
    services::{
        client_failures,
        events::EventBus,
        menu_prefs::{self, MenuPrefs, MENU_UPDATED_EVENT},
        shared::errors::AppResult,
    },
};

/// `PUT /api/me/menu` — grava o menu e avisa as outras abas e aparelhos do
/// usuário pelo SSE.
async fn update_menu(
    headers: HeaderMap,
    State(ctx): State<AppContext>,
    Json(input): Json<MenuPrefs>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let user_id = user.id;
    let saved = menu_prefs::save(&ctx.db, user, input).await?;
    if let Ok(bus) = EventBus::from_context(&ctx) {
        bus.publish_ephemeral(
            MENU_UPDATED_EVENT,
            serde_json::json!({ "userId": user_id, "pinned": saved.pinned, "hidden": saved.hidden }),
        );
    }
    Ok(format::json(saved)?)
}

/// `POST /api/me/client-failures` — o navegador não recebeu a resposta de
/// uma requisição; o aviso vai para o log da aplicação.
async fn report_client_failure(
    headers: HeaderMap,
    State(ctx): State<AppContext>,
    Json(input): Json<ClientFailureInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let failure = client_failures::validate(input)?;
    client_failures::record(user.id, &failure);
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/me")
        .add("/menu", put(update_menu))
        .add("/client-failures", post(report_client_failure))
}
