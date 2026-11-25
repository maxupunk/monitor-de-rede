use std::sync::Arc;

use chrono::Utc;
use loco_rs::app::AppContext;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{
    fanout::{EventFanout, Standalone},
    notice::Notice,
};
use crate::shared::errors::{AppError, AppResult};

/// Evento de domínio no formato exato que o `EventSource` do frontend lê.
///
/// Os três nomes são contrato (§11.1): `stores/events.ts` faz
/// `raw.type`, `raw.data` e `raw.timestamp` — um `payload`/`occurredAt`
/// idiomático em Rust chegaria como `undefined` e todo `case` do despachante
/// receberia `{}`, sem erro visível em lugar nenhum.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Um snapshot guardado: o evento e de que processo ele veio (`None` = deste).
#[derive(Clone)]
struct StoredSnapshot {
    encoded: Arc<EncodedEvent>,
    origin: Option<String>,
}

/// Último estado de cada recurso em tempo real, por `(tipo, chave)`.
type SnapshotMap = std::collections::BTreeMap<(String, String), StoredSnapshot>;

/// O barramento de eventos do processo.
///
/// A entrega local é um `broadcast` em memória. O que atravessa para os outros
/// processos da central — e como — é do [`EventFanout`] escolhido no boot
/// (ADR 024): no PostgreSQL, `NOTIFY` no commit; no SQLite (um processo só),
/// nada.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<DomainEvent>,
    encoded_sender: broadcast::Sender<Arc<EncodedEvent>>,
    origin: String,
    snapshots: Arc<std::sync::Mutex<SnapshotMap>>,
    fanout: Arc<dyn EventFanout>,
}

impl EventBus {
    /// Barramento de um processo só (SQLite, testes).
    #[must_use]
    pub fn create() -> Self {
        Self::with_fanout(Arc::new(Standalone))
    }

    /// Barramento que entrega aos outros processos pelo `fanout` dado.
    #[must_use]
    pub fn with_fanout(fanout: Arc<dyn EventFanout>) -> Self {
        let (sender, _) = broadcast::channel(1024);
        let (encoded_sender, _) = broadcast::channel(1024);
        Self {
            sender,
            encoded_sender,
            origin: Uuid::new_v4().to_string(),
            snapshots: Arc::default(),
            fanout,
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
        // A linha do outbox e o aviso aos outros processos saem no mesmo
        // commit (ADR 024): nenhum dos dois existe sem o outro.
        self.fanout.persist(db, &self.origin, &event).await?;
        self.publish_local(event.clone());
        Ok(event)
    }

    /// Publica telemetria efêmera: sem linha no `event_outbox`.
    ///
    /// Amostras frequentes não pertencem ao outbox: persistir cada leitura
    /// faria o banco crescer apenas para alimentar clientes que podem nem
    /// estar conectados. Os outros processos da central recebem o aviso, se
    /// ele chegar; quem estava sem escuta naquela hora não o recebe depois.
    /// Eventos de domínio continuam usando `publish`.
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
        self.fanout.announce(Notice::Event {
            origin: self.origin.clone(),
            id: None,
            event: Some(event.clone()),
        });
        event
    }

    /// Publica o estado completo de um recurso e o guarda como o mais
    /// recente. Quem assinar depois recebe esse estado logo após
    /// `stream:connected` (ver [`Self::current_snapshots`]) — sem esperar o
    /// próximo ciclo e sem hidratação por HTTP (AGENTS §9).
    ///
    /// O snapshot viaja **como snapshot** para os outros processos da central:
    /// lá ele também entra no cache, e a aba que conectar depois nesses
    /// processos recebe o estado atual.
    pub fn publish_snapshot(
        &self,
        event_type: impl Into<String>,
        key: impl Into<String>,
        payload: serde_json::Value,
    ) -> DomainEvent {
        let key = key.into();
        let event = DomainEvent {
            event_type: event_type.into(),
            payload,
            occurred_at: Utc::now().to_rfc3339(),
        };
        self.store_snapshot(key.clone(), event.clone(), None);
        self.fanout.announce(Notice::Snapshot {
            origin: self.origin.clone(),
            key,
            id: None,
            event: Some(event.clone()),
        });
        event
    }

    /// Esquece os snapshots de uma chave (host desconectado, por exemplo), aqui
    /// e nos outros processos.
    pub fn forget_snapshots(&self, key: &str) {
        self.lock_snapshots()
            .retain(|(_, stored_key), _| stored_key != key);
        self.fanout.announce(Notice::Forget {
            origin: self.origin.clone(),
            key: key.to_string(),
        });
    }

    /// Snapshot que chegou de outro processo: entra no cache e vai às abas
    /// deste, sem voltar a ser anunciado.
    pub fn apply_remote_snapshot(&self, origin: &str, key: String, event: DomainEvent) {
        self.store_snapshot(key, event, Some(origin.to_string()));
    }

    /// Outro processo esqueceu uma chave: saem daqui só os snapshots que
    /// vieram **dele**. Um snapshot desta chave produzido aqui depois (o agente
    /// que caiu lá e reconectou aqui) não pode sumir por um aviso atrasado.
    pub fn apply_remote_forget(&self, origin: &str, key: &str) {
        self.lock_snapshots().retain(|(_, stored_key), stored| {
            stored_key != key || stored.origin.as_deref() != Some(origin)
        });
    }

    /// Reanuncia os snapshots produzidos **neste** processo — a resposta ao
    /// `Resync` de um processo que acabou de (re)começar a escutar.
    pub fn reannounce_local_snapshots(&self) {
        let local: Vec<(String, DomainEvent)> = self
            .lock_snapshots()
            .iter()
            .filter(|(_, stored)| stored.origin.is_none())
            .map(|((_, key), stored)| (key.clone(), stored.encoded.event.clone()))
            .collect();
        for (key, event) in local {
            self.fanout.announce(Notice::Snapshot {
                origin: self.origin.clone(),
                key,
                id: None,
                event: Some(event),
            });
        }
    }

    /// Anuncia um aviso aos outros processos (o `Resync` do ouvinte).
    pub fn announce(&self, notice: Notice) {
        self.fanout.announce(notice);
    }

    #[must_use]
    pub fn current_snapshots(&self) -> Vec<DomainEvent> {
        self.lock_snapshots()
            .values()
            .map(|stored| stored.encoded.event.clone())
            .collect()
    }

    /// Snapshots atuais já serializados em `Arc<EncodedEvent>`.
    #[must_use]
    pub fn current_encoded_snapshots(&self) -> Vec<Arc<EncodedEvent>> {
        self.lock_snapshots()
            .values()
            .map(|stored| Arc::clone(&stored.encoded))
            .collect()
    }

    fn store_snapshot(&self, key: String, event: DomainEvent, origin: Option<String>) {
        let encoded = EncodedEvent::new(event);
        // Grava o snapshot **antes** de difundir: garante que o cache nunca
        // fique atrás do que foi entregue ao vivo em chamadas concorrentes.
        self.lock_snapshots().insert(
            (encoded.event.event_type.clone(), key),
            StoredSnapshot {
                encoded: Arc::clone(&encoded),
                origin,
            },
        );
        self.broadcast_encoded(&encoded);
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
