//! Canal de notificação Web Push (PWA / segundo plano).
//!
//! Entrega alertas em tempo real para os navegadores e dispositivos móveis inscritos,
//! funcionando mesmo quando a aba ou o aplicativo PWA estiver fechado.

use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tracing::{debug, warn};

use super::super::contracts::{NotificationChannel, NotificationMessage};
use crate::{
    models::_entities::push_subscriptions,
    services::webpush::{
        crypto::SubscriptionKeys, get_or_create_vapid_keys, send_push, PushOutcome,
    },
};

pub struct WebPushChannel;

impl Default for WebPushChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl WebPushChannel {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl NotificationChannel for WebPushChannel {
    fn name(&self) -> &str {
        "webpush"
    }

    async fn send(&self, ctx: &AppContext, message: &NotificationMessage) -> bool {
        let subs = match push_subscriptions::Entity::find().all(&ctx.db).await {
            Ok(subs) => subs,
            Err(error) => {
                warn!(%error, "Falha ao buscar subscrições Web Push no banco");
                return false;
            }
        };

        if subs.is_empty() {
            debug!("Nenhuma subscrição Web Push ativa no momento");
            return true;
        }

        let vapid = match get_or_create_vapid_keys(&ctx.db).await {
            Ok(keys) => keys,
            Err(error) => {
                warn!(%error, "Falha ao obter chaves VAPID do sistema");
                return false;
            }
        };

        let payload = json!({
            "title": message.title,
            "body": message.body,
            "severity": message.severity.as_str(),
            "icon": "/pwa-192x192.png",
            "badge": "/pwa-192x192.png",
            "tag": format!("alert-{}", chrono::Utc::now().timestamp_millis()),
            "data": {
                "url": "/alerts",
                "severity": message.severity.as_str(),
                "metadata": message.metadata
            }
        });

        let mut delivered = 0;
        let mut expired_ids = Vec::new();

        for sub in &subs {
            let keys = SubscriptionKeys {
                p256dh: sub.p256dh.clone(),
                auth: sub.auth.clone(),
            };

            match send_push(&sub.endpoint, &keys, &vapid, &payload).await {
                Ok(PushOutcome::Success) => {
                    delivered += 1;
                }
                Ok(PushOutcome::Expired) => {
                    expired_ids.push(sub.id);
                }
                Err(error) => {
                    warn!(
                        subscription_id = sub.id,
                        endpoint = %sub.endpoint,
                        %error,
                        "Erro ao entregar Web Push"
                    );
                }
            }
        }

        if !expired_ids.is_empty() {
            let count = expired_ids.len();
            if let Err(error) = push_subscriptions::Entity::delete_many()
                .filter(push_subscriptions::Column::Id.is_in(expired_ids))
                .exec(&ctx.db)
                .await
            {
                warn!(%error, "Falha ao expurgar subscrições Web Push expiradas");
            } else {
                debug!(pruned = count, "Subscrições Web Push expiradas expurgadas");
            }
        }

        debug!(
            delivered,
            total = subs.len(),
            title = %message.title,
            "Notificações Web Push despachadas"
        );
        true
    }
}
