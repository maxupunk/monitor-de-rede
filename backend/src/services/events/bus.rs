use std::sync::Arc;

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, Set};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::{
    models::event_outbox,
    services::shared::errors::{AppError, AppResult},
};

/// Evento de domínio no formato exato que o `EventSource` do frontend lê.
///
/// Os três nomes são contrato (§11.1): `stores/events.ts` faz
/// `raw.type`, `raw.data` e `raw.timestamp` — um `payload`/`occurredAt`
/// idiomático em Rust chegaria como `undefined` e todo `case` do despachante
/// receberia `{}`, sem erro visível em lugar nenhum.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(rename = "data")]
    pub payload: serde_json::Value,
    #[serde(rename = "timestamp")]
    pub occurred_at: String,
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<DomainEvent>,
    origin: String,
    last_relayed_id: Arc<Mutex<i64>>,
}

impl EventBus {
    #[must_use]
    pub fn create() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self {
            sender,
            origin: Uuid::new_v4().to_string(),
            last_relayed_id: Arc::new(Mutex::new(0)),
        }
    }

    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store.get::<Self>().ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("Barramento de eventos não inicializado"))
        })
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.sender.subscribe()
    }

    #[must_use]
    pub fn has_subscribers(&self) -> bool {
        self.sender.receiver_count() > 0
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub async fn publish(
        &self,
        db: &sea_orm::DatabaseConnection,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> AppResult<DomainEvent> {
        let event = DomainEvent {
            event_type: event_type.into(),
            payload,
            occurred_at: Utc::now().to_rfc3339(),
        };
        event_outbox::ActiveModel {
            r#type: Set(event.event_type.clone()),
            origin: Set(self.origin.clone()),
            payload: Set(serde_json::to_value(&event)
                .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?),
            ..Default::default()
        }
        .insert(db)
        .await?;
        self.publish_local(event.clone());
        Ok(event)
    }

    pub fn publish_local(&self, event: DomainEvent) {
        let _ = self.sender.send(event);
    }

    pub(crate) async fn last_relayed_id(&self) -> i64 {
        *self.last_relayed_id.lock().await
    }

    pub(crate) async fn set_last_relayed_id(&self, id: i64) {
        *self.last_relayed_id.lock().await = id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializa_no_contrato_sse_do_frontend() {
        let event = DomainEvent {
            event_type: "monitor:result".into(),
            payload: serde_json::json!({ "id": 7 }),
            occurred_at: "2026-08-11T00:00:00Z".into(),
        };
        let json = serde_json::to_value(event).unwrap();
        // `stores/events.ts` lê exatamente estes três nomes.
        assert_eq!(json["type"], "monitor:result");
        assert_eq!(json["data"]["id"], 7);
        assert_eq!(json["timestamp"], "2026-08-11T00:00:00Z");
        assert!(json.get("payload").is_none());
        assert!(json.get("occurredAt").is_none());
    }
}
