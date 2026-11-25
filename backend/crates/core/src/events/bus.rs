use std::sync::Arc;

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, QueryOrder, QuerySelect, Set};
use tokio::sync::{broadcast, Mutex, MutexGuard};
use uuid::Uuid;

use crate::{
    models::{_entities::event_outbox as event_outbox_entity, event_outbox},
    shared::errors::{AppError, AppResult},
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

/// Evento pré-serializado para fan-out SSE zero-copy: serializa uma única vez
/// no produtor e compartilha via `Arc` entre todas as conexões abertas.
#[derive(Debug)]
pub struct EncodedEvent {
    pub event: DomainEvent,
    pub json: Arc<str>,
}

impl EncodedEvent {
    #[must_use]
    pub fn new(event: DomainEvent) -> Arc<Self> {
        let json: Arc<str> = serde_json::to_string(&event)
            .unwrap_or_else(|_| "{}".to_string())
            .into();
        Arc::new(Self { event, json })
    }
}

/// Último estado de cada recurso em tempo real, por `(tipo, chave)`.
type SnapshotMap = std::collections::BTreeMap<(String, String), Arc<EncodedEvent>>;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<DomainEvent>,
    encoded_sender: broadcast::Sender<Arc<EncodedEvent>>,
    origin: String,
    last_relayed_id: Arc<Mutex<i64>>,
    snapshots: Arc<std::sync::Mutex<SnapshotMap>>,
}

impl EventBus {
    #[must_use]
    pub fn create() -> Self {
        let (sender, _) = broadcast::channel(1024);
        let (encoded_sender, _) = broadcast::channel(1024);
        Self {
            sender,
            encoded_sender,
            origin: Uuid::new_v4().to_string(),
            last_relayed_id: Arc::new(Mutex::new(0)),
            snapshots: Arc::default(),
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

    /// Assina o barramento recebendo eventos já serializados em `Arc<EncodedEvent>`,
    /// evitando clonar `serde_json::Value` e reserializar JSON por assinante SSE.
    #[must_use]
    pub fn subscribe_encoded(&self) -> broadcast::Receiver<Arc<EncodedEvent>> {
        self.encoded_sender.subscribe()
    }

    #[must_use]
    pub fn has_subscribers(&self) -> bool {
        self.sender.receiver_count() > 0 || self.encoded_sender.receiver_count() > 0
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Posiciona o cursor do relay no último `id` já gravado em `event_outbox`.
    /// Chamado no boot do servidor para não retransmitir eventos históricos de
    /// execuções anteriores como se fossem ao vivo.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn seed_from_outbox<C: ConnectionTrait>(&self, db: &C) -> AppResult<i64> {
        let latest = event_outbox::Entity::find()
            .select_only()
            .column(event_outbox_entity::Column::Id)
            .order_by_desc(event_outbox_entity::Column::Id)
            .limit(1)
            .into_tuple::<i64>()
            .one(db)
            .await?
            .unwrap_or(0);
        let mut cursor = self.last_relayed_id.lock().await;
        if latest > *cursor {
            *cursor = latest;
        }
        Ok(*cursor)
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

    /// Publica telemetria efêmera diretamente no processo HTTP.
    ///
    /// Amostras frequentes não pertencem ao `event_outbox`: persistir cada
    /// leitura faria o banco crescer apenas para alimentar clientes que podem
    /// nem estar conectados. Eventos de domínio continuam usando `publish`.
    pub fn publish_ephemeral(
        &self,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> DomainEvent {
        let event = DomainEvent {
            event_type: event_type.into(),
            payload,
            occurred_at: Utc::now().to_rfc3339(),
        };
        self.publish_local(event.clone());
        event
    }

    /// Publica o estado completo de um recurso e o guarda como o mais
    /// recente. Quem assinar depois recebe esse estado logo após
    /// `stream:connected` (ver [`Self::current_snapshots`]) — sem esperar o
    /// próximo ciclo e sem hidratação por HTTP (AGENTS §9).
    pub fn publish_snapshot(
        &self,
        event_type: impl Into<String>,
        key: impl Into<String>,
        payload: serde_json::Value,
    ) -> DomainEvent {
        let event = DomainEvent {
            event_type: event_type.into(),
            payload,
            occurred_at: Utc::now().to_rfc3339(),
        };
        let encoded = EncodedEvent::new(event);
        // Grava o snapshot **antes** de difundir: garante que o cache nunca
        // fique atrás do que foi entregue ao vivo em chamadas concorrentes.
        self.lock_snapshots().insert(
            (encoded.event.event_type.clone(), key.into()),
            Arc::clone(&encoded),
        );
        self.broadcast_encoded(&encoded);
        encoded.event.clone()
    }

    /// Esquece os snapshots de uma chave (host desconectado, por exemplo).
    pub fn forget_snapshots(&self, key: &str) {
        self.lock_snapshots()
            .retain(|(_, stored_key), _| stored_key != key);
    }

    #[must_use]
    pub fn current_snapshots(&self) -> Vec<DomainEvent> {
        self.lock_snapshots()
            .values()
            .map(|encoded| encoded.event.clone())
            .collect()
    }

    /// Snapshots atuais já serializados em `Arc<EncodedEvent>`.
    #[must_use]
    pub fn current_encoded_snapshots(&self) -> Vec<Arc<EncodedEvent>> {
        self.lock_snapshots().values().cloned().collect()
    }

    fn lock_snapshots(&self) -> std::sync::MutexGuard<'_, SnapshotMap> {
        self.snapshots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn broadcast_encoded(&self, encoded: &Arc<EncodedEvent>) {
        if self.encoded_sender.receiver_count() > 0 {
            let _ = self.encoded_sender.send(Arc::clone(encoded));
        }
        if self.sender.receiver_count() > 0 {
            let _ = self.sender.send(encoded.event.clone());
        }
    }

    pub fn publish_local(&self, event: DomainEvent) {
        let has_encoded = self.encoded_sender.receiver_count() > 0;
        let has_plain = self.sender.receiver_count() > 0;
        if !has_encoded && !has_plain {
            return;
        }
        if has_encoded {
            if has_plain {
                let _ = self.sender.send(event.clone());
            }
            let _ = self.encoded_sender.send(EncodedEvent::new(event));
        } else {
            let _ = self.sender.send(event);
        }
    }

    pub(crate) async fn lock_last_relayed_id(&self) -> MutexGuard<'_, i64> {
        self.last_relayed_id.lock().await
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

    #[tokio::test]
    async fn telemetria_efemera_chega_ao_assinante_sem_outbox() {
        let bus = EventBus::create();
        let mut receiver = bus.subscribe();

        bus.publish_ephemeral("docker:snapshot", serde_json::json!({ "available": true }));

        let event = receiver.recv().await.unwrap();
        assert_eq!(event.event_type, "docker:snapshot");
        assert_eq!(event.payload["available"], true);
    }
}
