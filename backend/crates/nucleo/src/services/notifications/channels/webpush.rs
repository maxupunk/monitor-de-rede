//! Canal de notificação Web Push (PWA / segundo plano).
//!
//! Entrega alertas em tempo real para os navegadores e dispositivos móveis inscritos,
//! funcionando mesmo quando a aba ou o aplicativo PWA estiver fechado.

use loco_rs::app::AppContext;
use netmonitor_core::areas::Area;
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
        let target_url = message
            .metadata
            .get("url")
            .and_then(serde_json::Value::as_str)
            .filter(|url| !url.is_empty())
            .unwrap_or("/alerts");
        let tag = message
            .metadata
            .get("tag")
            .and_then(serde_json::Value::as_str)
            .filter(|tag| !tag.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("alert-{}", chrono::Utc::now().timestamp_millis()));

        let mut payload = json!({
            "title": message.title,
            "body": message.body,
            "severity": message.severity.as_str(),
            "icon": "/pwa-192x192.png",
            "badge": "/pwa-192x192.png",
            "tag": tag,
            "data": {
                "url": target_url,
                "severity": message.severity.as_str(),
                "metadata": message.metadata
            }
        });
        if let Some(actions) = message
            .metadata
            .get("actions")
            .filter(|value| value.is_array())
        {
            payload["actions"] = actions.clone();
        }

        // Alerta de área só a quem tem acesso a ela (ADR 019); o do Núcleo, a
        // todos.
        let audience = message
            .metadata
            .get("area")
            .and_then(serde_json::Value::as_str)
            .and_then(Area::from_id)
            .filter(|area| area.is_optional())
            .map_or(Audience::Everyone, Audience::WithAccessTo);
        match deliver(&ctx.db, audience, &payload).await {
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
