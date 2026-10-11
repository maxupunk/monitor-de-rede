//! Canal de notificação Web Push (PWA / segundo plano).
//!
//! Entrega alertas em tempo real para os navegadores e dispositivos móveis inscritos,
//! funcionando mesmo quando a aba ou o aplicativo PWA estiver fechado.

use loco_rs::app::AppContext;
use serde_json::json;
use tracing::{debug, warn};

use super::super::contracts::{NotificationChannel, NotificationMessage};
use crate::services::webpush::{deliver, Audience};

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

        match deliver(&ctx.db, Audience::Everyone, &payload).await {
            Ok(delivery) => {
                debug!(
                    delivered = delivery.sent,
                    total = delivery.subscriptions,
                    pruned = delivery.expired_pruned,
                    title = %message.title,
                    "Notificações Web Push despachadas"
                );
                true
            }
            Err(error) => {
                warn!(%error, "Falha ao despachar Web Push");
                false
            }
        }
    }
}
