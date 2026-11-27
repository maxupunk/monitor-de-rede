//! `LISTEN`/`NOTIFY` do PostgreSQL entre os processos da central (ADR 024).
//!
//! **Emissão.** O evento de domínio grava a linha do outbox e chama
//! `pg_notify` na mesma transação: o PostgreSQL só entrega o aviso no commit,
//! então ninguém recebe um `id` que ainda não pode ler, e nenhuma linha
//! confirmada fica sem aviso. Não há cursor — é ele que pulava o `id`
//! reservado antes e confirmado depois. Os avisos de código síncrono
//! (snapshot, esquecimento, efêmero, `Resync`) vão por uma fila limitada a uma
//! tarefa só, que preserva a ordem por processo.
//!
//! **Recepção.** [`spawn_listener`] mantém um `PgListener` (numa conexão
//! própria, fora do pool da aplicação) e entrega cada aviso pelo
//! [`super::receive::deliver`]. Se a conexão cair, entre a queda e o novo
//! `LISTEN` os avisos se perdem — o `NOTIFY` não guarda nada para quem não
//! escuta. A recuperação é em duas partes, as duas **depois** do novo
//! `LISTEN` (o que chegar dali em diante já está garantido):
//!
//! 1. relê do outbox as linhas de outros processos desde a última vez em que
//!    a escuta estava comprovadamente de pé (eventos de domínio e snapshots
//!    grandes), sem repetir os `id` já entregues;
//! 2. manda `Resync`: cada processo reanuncia os snapshots que ele produziu.
//!
//! O mesmo `Resync` sai quando o processo começa a escutar, e é assim que a
//! aba que conecta num processo recém-subido já recebe o estado atual da VPN e
//! do Wi-Fi produzido no líder. "Comprovadamente de pé" é a última
//! notificação recebida ou o último `SELECT 1` bem-sucedido pela própria
//! conexão de escuta, que roda a cada [`KEEPALIVE`] de silêncio — é ele também
//! que descobre uma conexão meio-aberta que o TCP ainda não derrubou.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sqlx::postgres::{PgListener, PgPoolOptions};
use tokio::sync::mpsc;

use super::{insert_row, receive, EventFanout};
use crate::{
    events::{
        notice::{Notice, RecentIds, CHANNEL},
        DomainEvent, EventBus,
    },
    shared::{errors::AppResult, transaction::begin_write},
};

/// Quantos avisos síncronos podem esperar a tarefa de envio. Cheia, o aviso é
/// descartado com registro: segurar quem publica (um laço de coleta, uma
/// requisição) por causa do banco seria pior que um snapshot atrasado — o
/// próximo o substitui.
const QUEUE: usize = 4_096;

/// Silêncio na escuta antes de conferir a conexão.
pub const KEEPALIVE: Duration = Duration::from_secs(30);

/// Espera entre tentativas de reabrir a escuta.
const RECONNECT_BACKOFF: Duration = Duration::from_secs(2);

/// A saída pelo PostgreSQL.
pub struct PgFanout {
    queue: mpsc::Sender<Notice>,
}

impl PgFanout {
    /// Sobe a tarefa de envio e devolve a saída. Precisa de um runtime tokio.
    #[must_use]
    pub fn start(db: DatabaseConnection) -> Arc<Self> {
        let (queue, mut pending) = mpsc::channel::<Notice>(QUEUE);
        tokio::spawn(async move {
            while let Some(notice) = pending.recv().await {
                if let Err(error) = send(&db, notice).await {
                    tracing::warn!(%error, "falha ao avisar os outros processos da central");
                }
            }
        });
        Arc::new(Self { queue })
    }
}

#[async_trait]
impl EventFanout for PgFanout {
    async fn persist(
        &self,
        db: &DatabaseConnection,
        origin: &str,
        event: &DomainEvent,
    ) -> AppResult<()> {
        let txn = begin_write(db).await?;
        let id = insert_row(&txn, origin, event, None).await?;
        let notice = Notice::Event {
            origin: origin.to_string(),
            id: Some(id),
            event: Some(event.clone()),
        };
        notify(&txn, &inline_or_row(notice, id)).await?;
        txn.commit().await?;
        Ok(())
    }

    fn announce(&self, notice: Notice) {
        if let Err(error) = self.queue.try_send(notice) {
            tracing::warn!(%error, "fila de avisos entre processos cheia; aviso descartado");
        }
    }
}

/// O texto do aviso: inteiro se couber, senão só a linha `id`.
fn inline_or_row(notice: Notice, id: i64) -> String {
    notice.encode().unwrap_or_else(|| {
        notice
            .by_row(id)
            .encode()
            .unwrap_or_else(|| bare_row_notice(id))
    })
}

/// Um aviso só com `origin` e `id` sempre cabe; se um dia não couber (origem
/// absurda), o `id` sozinho ainda leva à linha.
fn bare_row_notice(id: i64) -> String {
    format!("{{\"kind\":\"event\",\"origin\":\"\",\"id\":{id}}}")
}

/// Envia um aviso de código síncrono. O que não cabe no `NOTIFY` ganha linha
/// no outbox (com a chave, se for snapshot) e o aviso leva só o `id` — na
/// mesma transação, pelo mesmo motivo do `persist`.
async fn send(db: &DatabaseConnection, notice: Notice) -> AppResult<()> {
    if let Some(text) = notice.encode() {
        return notify(db, &text).await;
    }
    let (event, key) = match &notice {
        Notice::Event {
            event: Some(event), ..
        } => (event.clone(), None),
        Notice::Snapshot {
            event: Some(event),
            key,
            ..
        } => (event.clone(), Some(key.clone())),
        // `Forget`/`Resync` sempre cabem; aviso sem evento não tem o que gravar.
        _ => return Ok(()),
    };
    let txn = begin_write(db).await?;
    let id = insert_row(&txn, notice.origin(), &event, key).await?;
    notify(&txn, &inline_or_row(notice, id)).await?;
    txn.commit().await?;
    Ok(())
}

async fn notify<C: ConnectionTrait>(db: &C, text: &str) -> AppResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_notify($1, $2)",
        [CHANNEL.into(), text.into()],
    ))
    .await?;
    Ok(())
}

/// Sobe a escuta deste processo. Roda até o processo acabar.
pub fn spawn_listener(db: DatabaseConnection, bus: EventBus) -> tokio::task::JoinHandle<()> {
    tokio::spawn(listen_forever(db, bus))
}

async fn listen_forever(db: DatabaseConnection, bus: EventBus) {
    let mut seen = RecentIds::with_capacity(receive::RECENT_IDS);
    // `None` no primeiro `LISTEN`: o processo acabou de subir e não tem lacuna
    // a recuperar — reler o outbox aqui seria entregar histórico como se fosse
    // ao vivo.
    let mut healthy_at: Option<chrono::DateTime<Utc>> = None;
    loop {
        let mut listener = match open(&db).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::warn!(%error, "não foi possível escutar os avisos dos outros processos");
                tokio::time::sleep(RECONNECT_BACKOFF).await;
                continue;
            }
        };
        recover(&db, &bus, healthy_at, &mut seen).await;
        healthy_at = Some(Utc::now());
        loop {
            match tokio::time::timeout(KEEPALIVE, listener.try_recv()).await {
                Ok(Ok(Some(notification))) => {
                    healthy_at = Some(Utc::now());
                    handle(&db, &bus, notification.payload(), &mut seen).await;
                }
                // Conexão caiu e o `PgListener` já reabriu o `LISTEN` (é o
                // `eager_reconnect` padrão): recupera a lacuna e segue nela.
                Ok(Ok(None)) => {
                    tracing::warn!("escuta dos avisos reconectada; recuperando a lacuna");
                    recover(&db, &bus, healthy_at, &mut seen).await;
                    healthy_at = Some(Utc::now());
                }
                Ok(Err(error)) => {
                    tracing::warn!(%error, "escuta dos avisos falhou; reabrindo");
                    break;
                }
                // Silêncio: confere a conexão pela própria escuta (o
                // `try_recv` é cancel-safe).
                Err(_) => match sqlx::query("SELECT 1").execute(&mut listener).await {
                    Ok(_) => healthy_at = Some(Utc::now()),
                    Err(error) => {
                        tracing::warn!(%error, "escuta dos avisos sem resposta; reabrindo");
                        break;
                    }
                },
            }
        }
        tokio::time::sleep(RECONNECT_BACKOFF).await;
    }
}

/// A escuta segura a conexão dela enquanto viver; por isso ela sai de um pool
/// próprio de uma conexão, com as mesmas opções do da aplicação. Tirada do pool
/// da aplicação, ela ocuparia uma vaga dele para sempre — e, num pool de uma
/// conexão (o dos testes), travaria toda consulta até o `ConnectionAcquire`.
async fn open(db: &DatabaseConnection) -> Result<PgListener, sqlx::Error> {
    let options = db.get_postgres_connection_pool().connect_options();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy_with((*options).clone());
    let mut listener = PgListener::connect_with(&pool).await?;
    listener.listen(CHANNEL).await?;
    Ok(listener)
}

/// Depois de um `LISTEN` (novo ou refeito): relê a lacuna, se houve, e pede
/// aos outros processos os snapshots deles.
async fn recover(
    db: &DatabaseConnection,
    bus: &EventBus,
    healthy_at: Option<chrono::DateTime<Utc>>,
    seen: &mut RecentIds,
) {
    if let Some(since) = healthy_at {
        match receive::replay_since(db, bus, since, seen).await {
            Ok(0) => {}
            Ok(count) => tracing::info!(count, "eventos da lacuna da escuta reentregues"),
            Err(error) => tracing::warn!(%error, "falha ao reler o outbox depois da reconexão"),
        }
    }
    bus.announce(Notice::Resync {
        origin: bus.origin().to_string(),
    });
}

async fn handle(db: &DatabaseConnection, bus: &EventBus, payload: &str, seen: &mut RecentIds) {
    match Notice::decode(payload) {
        Ok(notice) => {
            if let Err(error) = receive::deliver(db, bus, notice, seen).await {
                tracing::warn!(%error, "falha ao entregar aviso de outro processo");
            }
        }
        Err(error) => tracing::warn!(%error, "aviso entre processos ilegível; ignorado"),
    }
}
