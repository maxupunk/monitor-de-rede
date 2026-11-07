//! Entrega de uma notificação a todos os navegadores inscritos — de todo
//! mundo (alerta) ou de um usuário só (teste, pedido de aprovação da IA).
//!
//! Inscrição que o serviço de push dá como expirada sai do banco aqui mesmo.

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;
use tracing::warn;

use super::{crypto::SubscriptionKeys, get_or_create_vapid_keys, send_push, PushOutcome};
use std::collections::HashSet;

use netmonitor_core::{areas::Area, roles::AreaAccess};

use crate::{
    models::_entities::{push_subscriptions, users},
    services::shared::errors::AppResult,
};

/// Quem recebe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Everyone,
    User(i64),
    /// Quem tem acesso à área (ADR 019): o alerta de uma área não chega a
    /// quem não a vê.
    WithAccessTo(Area),
}

/// Resultado da entrega.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Delivery {
    /// Inscrições encontradas para o público.
    pub subscriptions: u64,
    pub sent: u64,
    pub expired_pruned: u64,
}

/// Só as inscrições de usuários que enxergam a área.
async fn with_access<C: ConnectionTrait>(
    db: &C,
    subscriptions: Vec<push_subscriptions::Model>,
    area: Area,
) -> AppResult<Vec<push_subscriptions::Model>> {
    let user_ids: Vec<i64> = subscriptions.iter().filter_map(|s| s.user_id).collect();
    let allowed: HashSet<i64> = users::Entity::find()
        .filter(users::Column::Id.is_in(user_ids))
        .all(db)
        .await?
        .into_iter()
        .filter(|user| {
            user.active
                && AreaAccess::of_user(user).is_ok_and(|access| access.role_in(area).is_some())
        })
        .map(|user| user.id)
        .collect();
    Ok(subscriptions
        .into_iter()
        .filter(|subscription| subscription.user_id.is_some_and(|id| allowed.contains(&id)))
        .collect())
}

/// Envia `payload` às inscrições do público.
///
/// # Errors
///
/// Falha do banco ao listar as inscrições ou ao obter as chaves VAPID. Falha
/// de entrega a um navegador só vira log: as outras inscrições seguem.
pub async fn deliver<C: ConnectionTrait>(
    db: &C,
    audience: Audience,
    payload: &Value,
) -> AppResult<Delivery> {
    let mut query =
        push_subscriptions::Entity::find().order_by_desc(push_subscriptions::Column::Id);
    if let Audience::User(user_id) = audience {
        query = query.filter(push_subscriptions::Column::UserId.eq(user_id));
    }
    let mut subscriptions = query.all(db).await?;
    if let Audience::WithAccessTo(area) = audience {
        subscriptions = with_access(db, subscriptions, area).await?;
    }
    let mut delivery = Delivery {
        subscriptions: subscriptions.len() as u64,
        ..Delivery::default()
    };
    if subscriptions.is_empty() {
        return Ok(delivery);
    }

    let vapid = get_or_create_vapid_keys(db).await?;
    let mut expired = Vec::new();
    for subscription in &subscriptions {
        let keys = SubscriptionKeys {
            p256dh: subscription.p256dh.clone(),
            auth: subscription.auth.clone(),
        };
        match send_push(&subscription.endpoint, &keys, &vapid, payload).await {
            Ok(PushOutcome::Success) => delivery.sent += 1,
            Ok(PushOutcome::Expired) => expired.push(subscription.id),
            Err(error) => warn!(
                subscription_id = subscription.id,
                endpoint = %subscription.endpoint,
                %error,
                "Erro ao entregar Web Push"
            ),
        }
    }

    if !expired.is_empty() {
        delivery.expired_pruned = expired.len() as u64;
        if let Err(error) = push_subscriptions::Entity::delete_many()
            .filter(push_subscriptions::Column::Id.is_in(expired))
            .exec(db)
            .await
        {
            warn!(%error, "Falha ao expurgar subscrições Web Push expiradas");
        }
    }
    Ok(delivery)
}
