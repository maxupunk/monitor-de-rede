//! O lado de quem recebe o aviso de outro processo (ADR 024).
//!
//! Independe do transporte: recebe um [`Notice`] já decodificado e o entrega
//! ao barramento local — evento para as abas, snapshot para o cache. Assim
//! dá para provar o roteamento no SQLite, sem `LISTEN`.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    events::{
        notice::{Notice, RecentIds},
        DomainEvent, EventBus,
    },
    models::{_entities::event_outbox as event_outbox_entity, event_outbox},
    shared::errors::AppResult,
};

/// Folga ao reler o outbox depois de perder a escuta: cobre o relógio do
/// banco (`created_at` é o `now()` do início da transação, não do commit) e a
/// incerteza de quando a conexão caiu de fato.
pub const REPLAY_MARGIN: Duration = Duration::seconds(30);

/// Teto da releitura: uma queda longa não pode virar uma rajada que afoga as
/// abas — e evento velho demais já não é "ao vivo".
pub const REPLAY_LIMIT: u64 = 1_000;

/// Quantos `id` entregues o processo lembra para não repetir na releitura.
pub const RECENT_IDS: usize = 4_096;

/// Entrega um aviso recebido.
///
/// O aviso do próprio processo é ignorado: o emissor já entregou localmente
/// quando publicou.
///
/// # Errors
///
/// Erro do banco ao carregar a linha de um aviso que só trouxe o `id`.
pub async fn deliver<C: ConnectionTrait>(
    db: &C,
    bus: &EventBus,
    notice: Notice,
    seen: &mut RecentIds,
) -> AppResult<()> {
    if notice.origin() == bus.origin() {
        return Ok(());
    }
    match notice {
        Notice::Event { id, event, .. } => {
            if id.is_some_and(|id| !seen.insert(id)) {
                return Ok(());
            }
            if let Some(event) = resolve(db, id, event).await? {
                bus.publish_local(event);
            }
        }
        Notice::Snapshot {
            origin,
            key,
            id,
            event,
        } => {
            if let Some(event) = resolve(db, id, event).await? {
                bus.apply_remote_snapshot(&origin, key, event);
            }
        }
        Notice::Forget { origin, key } => bus.apply_remote_forget(&origin, &key),
        Notice::Resync { .. } => bus.reannounce_local_snapshots(),
    }
    Ok(())
}

/// Relê o que outros processos gravaram no outbox desde `since` (menos a
/// [`REPLAY_MARGIN`]) — o que pode ter chegado enquanto a escuta estava
/// fora. Devolve quantas linhas foram entregues.
///
/// Só alcança o que tem linha: eventos de domínio e snapshots grandes. Os
/// snapshots pequenos voltam pelo `Resync` que o ouvinte manda ao reconectar;
/// os efêmeros perdidos ficam perdidos, como numa aba que reconecta.
///
/// # Errors
///
/// Erro do banco.
pub async fn replay_since<C: ConnectionTrait>(
    db: &C,
    bus: &EventBus,
    since: DateTime<Utc>,
    seen: &mut RecentIds,
) -> AppResult<u64> {
    let rows = event_outbox::Entity::find()
        .filter(event_outbox_entity::Column::CreatedAt.gte(since - REPLAY_MARGIN))
        .filter(event_outbox_entity::Column::Origin.ne(bus.origin()))
        .order_by_asc(event_outbox_entity::Column::Id)
        .limit(REPLAY_LIMIT)
        .all(db)
        .await?;
    let mut delivered = 0_u64;
    for row in rows {
        let event = row_event(row.r#type, row.payload, row.created_at.with_timezone(&Utc));
        match row.snapshot_key {
            Some(key) => bus.apply_remote_snapshot(&row.origin, key, event),
            None => {
                if !seen.insert(row.id) {
                    continue;
                }
                bus.publish_local(event);
            }
        }
        delivered += 1;
    }
    Ok(delivered)
}

/// O evento do aviso, ou o da linha quando o aviso só trouxe o `id`. Linha que
/// já saiu do outbox (purga) não tem o que entregar.
async fn resolve<C: ConnectionTrait>(
    db: &C,
    id: Option<i64>,
    event: Option<DomainEvent>,
) -> AppResult<Option<DomainEvent>> {
    if event.is_some() {
        return Ok(event);
    }
    let Some(id) = id else {
        return Ok(None);
    };
    Ok(event_outbox::Entity::find_by_id(id)
        .one(db)
        .await?
        .map(|row| row_event(row.r#type, row.payload, row.created_at.with_timezone(&Utc))))
}

/// O evento gravado na linha. Linha ilegível vira o tipo sem dados, como o
/// relay antigo fazia: a aba ao menos sabe que algo mudou.
fn row_event(event_type: String, payload: serde_json::Value, at: DateTime<Utc>) -> DomainEvent {
    serde_json::from_value::<DomainEvent>(payload).unwrap_or(DomainEvent {
        event_type,
        payload: serde_json::json!({}),
        occurred_at: at.to_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

    use super::*;
    use crate::events::fanout::{insert_row, EventFanout};

    /// Guarda o que o barramento anunciou.
    #[derive(Default)]
    struct Recorder(Mutex<Vec<Notice>>);

    #[async_trait]
    impl EventFanout for Recorder {
        async fn persist(
            &self,
            _db: &DatabaseConnection,
            _origin: &str,
            _event: &DomainEvent,
        ) -> AppResult<()> {
            Ok(())
        }

        fn announce(&self, notice: Notice) {
            self.0.lock().unwrap().push(notice);
        }
    }

    async fn outbox() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE event_outbox (id INTEGER PRIMARY KEY AUTOINCREMENT, type TEXT NOT NULL, \
             origin TEXT NOT NULL, payload TEXT NOT NULL, snapshot_key TEXT NULL, \
             created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')))",
        )
        .await
        .unwrap();
        db
    }

    fn event(kind: &str, value: i64) -> DomainEvent {
        DomainEvent {
            event_type: kind.into(),
            payload: serde_json::json!({ "v": value }),
            occurred_at: Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn evento_de_outro_processo_chega_as_abas_uma_vez_so() {
        let db = outbox().await;
        let bus = EventBus::create();
        let mut tab = bus.subscribe();
        let mut seen = RecentIds::with_capacity(8);
        let notice = Notice::Event {
            origin: "outro".into(),
            id: Some(5),
            event: Some(event("device:status", 1)),
        };
        deliver(&db, &bus, notice.clone(), &mut seen).await.unwrap();
        deliver(&db, &bus, notice, &mut seen).await.unwrap();
        assert_eq!(tab.try_recv().unwrap().event_type, "device:status");
        assert!(tab.try_recv().is_err(), "o mesmo id entregue duas vezes");
    }

    #[tokio::test]
    async fn aviso_da_propria_origem_e_ignorado() {
        let db = outbox().await;
        let bus = EventBus::create();
        let mut tab = bus.subscribe();
        let mut seen = RecentIds::with_capacity(8);
        let notice = Notice::Snapshot {
            origin: bus.origin().to_string(),
            key: "k".into(),
            id: None,
            event: Some(event("vpn:peers", 1)),
        };
        deliver(&db, &bus, notice, &mut seen).await.unwrap();
        assert!(tab.try_recv().is_err());
        assert!(bus.current_snapshots().is_empty());
    }

    #[tokio::test]
    async fn snapshot_pela_linha_entra_no_cache_como_snapshot() {
        let db = outbox().await;
        let bus = EventBus::create();
        let big = event("wifi:ap", 7);
        let id = insert_row(&db, "outro", &big, Some("ap-7".into()))
            .await
            .unwrap();
        let mut seen = RecentIds::with_capacity(8);
        let notice = Notice::Snapshot {
            origin: "outro".into(),
            key: "ap-7".into(),
            id: Some(id),
            event: None,
        };
        deliver(&db, &bus, notice, &mut seen).await.unwrap();
        let cached = bus.current_snapshots();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].event_type, "wifi:ap");
        assert_eq!(cached[0].payload["v"], 7);
    }

    #[tokio::test]
    async fn esquecer_remoto_so_tira_o_que_veio_do_emissor() {
        let db = outbox().await;
        let bus = EventBus::create();
        let mut seen = RecentIds::with_capacity(8);
        deliver(
            &db,
            &bus,
            Notice::Snapshot {
                origin: "a".into(),
                key: "agent-1".into(),
                id: None,
                event: Some(event("docker:snapshot", 1)),
            },
            &mut seen,
        )
        .await
        .unwrap();
        // O agente reconectou neste processo: snapshot local da mesma chave,
        // de outro tipo.
        bus.publish_snapshot("docker:stats", "agent-1", serde_json::json!({}));
        deliver(
            &db,
            &bus,
            Notice::Forget {
                origin: "a".into(),
                key: "agent-1".into(),
            },
            &mut seen,
        )
        .await
        .unwrap();
        let left: Vec<String> = bus
            .current_snapshots()
            .into_iter()
            .map(|event| event.event_type)
            .collect();
        assert_eq!(left, vec!["docker:stats".to_string()]);
    }

    #[tokio::test]
    async fn resync_reanuncia_so_os_snapshots_deste_processo() {
        let db = outbox().await;
        let recorder = Arc::new(Recorder::default());
        let bus = EventBus::with_fanout(recorder.clone());
        let mut seen = RecentIds::with_capacity(8);
        bus.publish_snapshot("vpn:peers", "vpn-1", serde_json::json!({ "n": 1 }));
        bus.apply_remote_snapshot("b", "ap-1".into(), event("wifi:ap", 1));
        recorder.0.lock().unwrap().clear();

        deliver(&db, &bus, Notice::Resync { origin: "c".into() }, &mut seen)
            .await
            .unwrap();

        let announced = recorder.0.lock().unwrap().clone();
        assert_eq!(announced.len(), 1);
        assert!(matches!(
            &announced[0],
            Notice::Snapshot { key, origin, .. } if key == "vpn-1" && origin == bus.origin()
        ));
    }

    #[tokio::test]
    async fn releitura_entrega_o_que_faltou_sem_repetir_o_que_chegou() {
        let db = outbox().await;
        let bus = EventBus::create();
        let mut tab = bus.subscribe();
        let mut seen = RecentIds::with_capacity(8);
        let delivered = insert_row(&db, "outro", &event("a", 1), None)
            .await
            .unwrap();
        insert_row(&db, "outro", &event("b", 2), None)
            .await
            .unwrap();
        insert_row(&db, bus.origin(), &event("proprio", 3), None)
            .await
            .unwrap();
        insert_row(&db, "outro", &event("wifi:ap", 4), Some("ap".into()))
            .await
            .unwrap();
        assert!(
            seen.insert(delivered),
            "o primeiro já tinha chegado pelo aviso"
        );

        let count = replay_since(&db, &bus, Utc::now(), &mut seen)
            .await
            .unwrap();

        assert_eq!(count, 2, "o evento perdido e o snapshot grande");
        assert_eq!(tab.try_recv().unwrap().event_type, "b");
        assert_eq!(tab.try_recv().unwrap().event_type, "wifi:ap");
        assert!(tab.try_recv().is_err());
        assert_eq!(bus.current_snapshots().len(), 1);
    }
}
