//! O que é do próprio usuário (`/api/me`): hoje, o menu (Fase 16). Todo papel
//! mexe aqui — personalizar o próprio menu não muda nada do sistema.

use axum::http::HeaderMap;
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::authenticated_user,
    services::{
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

pub fn routes() -> Routes {
    Routes::new().prefix("/me").add("/menu", put(update_menu))
}
