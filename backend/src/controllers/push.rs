//! Controlador de Notificações Web Push (PWA).
//!
//! Gerencia a chave pública VAPID, subscrições de navegadores e testes de entrega em segundo plano.

use axum::http::HeaderMap;
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::{
    controllers::auth_guard::AUTHENTICATED_USER_HEADER,
    models::{_entities::push_subscriptions, users},
    services::{
        shared::errors::{AppError, AppResult},
        webpush::{
            crypto::SubscriptionKeys, get_or_create_vapid_keys, get_public_key, send_push,
            PushOutcome,
        },
    },
};

#[derive(Debug, Deserialize)]
pub struct SubscriptionKeysDto {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSubscriptionDto {
    pub endpoint: String,
    pub keys: SubscriptionKeysDto,
    pub user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteSubscriptionDto {
    pub endpoint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VapidPublicKeyResponse {
    pub public_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushStatusResponse {
    pub configured: bool,
    pub public_key: String,
    pub total_subscriptions: u64,
    pub user_subscriptions: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPushResponse {
    pub success: bool,
    pub sent: u64,
    pub expired_pruned: u64,
    pub message: String,
}

/// `GET /api/push/vapid-public-key` — Obtém a chave pública VAPID para registro no navegador.
async fn public_key(State(ctx): State<AppContext>) -> AppResult<Response> {
    let key = get_public_key(&ctx.db).await?;
    Ok(format::json(VapidPublicKeyResponse { public_key: key })?)
}

/// `GET /api/push/status` — Status das notificações Web Push e contagem de subscrições.
async fn status(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let key = get_public_key(&ctx.db).await?;
    let user_id = get_current_user_id(&ctx, &headers).await;

    let total = push_subscriptions::Entity::find().count(&ctx.db).await?;
    let user_subs = match user_id {
        Some(uid) => {
            push_subscriptions::Entity::find()
                .filter(push_subscriptions::Column::UserId.eq(uid))
                .count(&ctx.db)
                .await?
        }
        None => 0,
    };

    Ok(format::json(PushStatusResponse {
        configured: true,
        public_key: key,
        total_subscriptions: total,
        user_subscriptions: user_subs,
    })?)
}

/// `POST /api/push/subscriptions` — Salva ou atualiza a subscrição Web Push deste dispositivo.
async fn save_subscription(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<SaveSubscriptionDto>,
) -> AppResult<Response> {
    let endpoint = input.endpoint.trim();
    if endpoint.is_empty() {
        return Err(AppError::bad_request(
            "Endpoint de push não pode ser vazio.",
        ));
    }
    if input.keys.p256dh.trim().is_empty() || input.keys.auth.trim().is_empty() {
        return Err(AppError::bad_request(
            "Chaves de criptografia da subscrição (p256dh e auth) são obrigatórias.",
        ));
    }

    let user_id = get_current_user_id(&ctx, &headers).await;
    let now = chrono::Utc::now().into();

    let existing = push_subscriptions::Entity::find()
        .filter(push_subscriptions::Column::Endpoint.eq(endpoint))
        .one(&ctx.db)
        .await?;

    let model = if let Some(sub) = existing {
        let mut active: push_subscriptions::ActiveModel = sub.into();
        active.p256dh = Set(input.keys.p256dh.trim().to_string());
        active.auth = Set(input.keys.auth.trim().to_string());
        active.user_agent = Set(input.user_agent);
        if user_id.is_some() {
            active.user_id = Set(user_id);
        }
        active.updated_at = Set(now);
        active.update(&ctx.db).await?
    } else {
        push_subscriptions::ActiveModel {
            user_id: Set(user_id),
            endpoint: Set(endpoint.to_string()),
            p256dh: Set(input.keys.p256dh.trim().to_string()),
            auth: Set(input.keys.auth.trim().to_string()),
            user_agent: Set(input.user_agent),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await?
    };

    info!(
        subscription_id = model.id,
        user_id = ?model.user_id,
        "Subscrição Web Push registrada com sucesso"
    );

    Ok(format::json(json!({
        "success": true,
        "id": model.id,
        "endpoint": model.endpoint,
    }))?)
}

/// `DELETE /api/push/subscriptions` — Remove uma subscrição Web Push pelo endpoint.
async fn delete_subscription(
    State(ctx): State<AppContext>,
    Json(input): Json<DeleteSubscriptionDto>,
) -> AppResult<Response> {
    let endpoint = input.endpoint.trim();
    let res = push_subscriptions::Entity::delete_many()
        .filter(push_subscriptions::Column::Endpoint.eq(endpoint))
        .exec(&ctx.db)
        .await?;

    info!(
        endpoint,
        rows_affected = res.rows_affected,
        "Subscrição Web Push cancelada"
    );

    Ok(format::json(json!({
        "success": true,
        "rowsAffected": res.rows_affected
    }))?)
}

/// `POST /api/push/test` — Dispara uma notificação Web Push de teste para os dispositivos do usuário.
async fn test_push(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let user_id = get_current_user_id(&ctx, &headers).await;
    let vapid = get_or_create_vapid_keys(&ctx.db).await?;

    // Busca subscrições do usuário ou todas se for teste global
    let mut query =
        push_subscriptions::Entity::find().order_by_desc(push_subscriptions::Column::Id);
    if let Some(uid) = user_id {
        query = query.filter(push_subscriptions::Column::UserId.eq(uid));
    }
    let subs = query.all(&ctx.db).await?;

    if subs.is_empty() {
        return Ok(format::json(TestPushResponse {
            success: false,
            sent: 0,
            expired_pruned: 0,
            message: "Nenhum dispositivo inscrito para receber notificações Web Push. Ative as notificações no navegador primeiro.".to_string(),
        })?);
    }

    let payload = json!({
        "title": "🔔 Teste de Notificação Web Push",
        "body": "Sucesso! Seu dispositivo está configurado para receber alertas em segundo plano do NetMonitor.",
        "icon": "/pwa-192x192.png",
        "badge": "/pwa-192x192.png",
        "tag": format!("test-push-{}", chrono::Utc::now().timestamp()),
        "data": {
            "url": "/settings",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    });

    let mut sent = 0;
    let mut expired = Vec::new();

    for sub in &subs {
        let keys = SubscriptionKeys {
            p256dh: sub.p256dh.clone(),
            auth: sub.auth.clone(),
        };

        match send_push(&sub.endpoint, &keys, &vapid, &payload).await {
            Ok(PushOutcome::Success) => {
                sent += 1;
            }
            Ok(PushOutcome::Expired) => {
                expired.push(sub.id);
            }
            Err(e) => {
                warn!(subscription_id = sub.id, error = %e, "Falha ao enviar notificação de teste Web Push");
            }
        }
    }

    // Limpa subscrições expiradas
    let expired_count = expired.len() as u64;
    if !expired.is_empty() {
        let _ = push_subscriptions::Entity::delete_many()
            .filter(push_subscriptions::Column::Id.is_in(expired))
            .exec(&ctx.db)
            .await;
    }

    Ok(format::json(TestPushResponse {
        success: sent > 0,
        sent,
        expired_pruned: expired_count,
        message: if sent > 0 {
            format!("Notificação de teste enviada com sucesso para {sent} dispositivo(s).")
        } else {
            "Não foi possível entregar a notificação de teste aos dispositivos inscritos."
                .to_string()
        },
    })?)
}

async fn get_current_user_id(ctx: &AppContext, headers: &HeaderMap) -> Option<i64> {
    let pid = headers
        .get(AUTHENTICATED_USER_HEADER)
        .and_then(|value| value.to_str().ok())?;

    users::Model::find_by_pid(&ctx.db, pid)
        .await
        .ok()
        .map(|u| u.id)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/push")
        .add("/vapid-public-key", get(public_key))
        .add("/status", get(status))
        .add(
            "/subscriptions",
            post(save_subscription).delete(delete_subscription),
        )
        .add("/test", post(test_push))
}
