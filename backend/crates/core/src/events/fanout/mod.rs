//! Como um evento sai deste processo para os outros da central (ADR 024).
//!
//! Duas implementações, escolhidas **uma vez** no boot pelo dialeto do banco
//! ([`crate::cluster::Coordination`]) — nenhum `if sqlite` fora dali:
//!
//! * [`Standalone`] — SQLite (testes, desenvolvimento): um processo só. O
//!   evento de domínio vai para o `event_outbox`, que continua sendo o
//!   registro que os testes e a auditoria leem, e não há a quem avisar.
//! * [`postgres::PgFanout`] — PostgreSQL: a linha do outbox e o `NOTIFY` saem
//!   no mesmo commit; os snapshots, os esquecimentos e os efêmeros vão pelo
//!   aviso (ou pela linha, quando não cabem). Quem recebe é o
//!   [`postgres::spawn_listener`], que entrega pelo [`receive`].

pub mod postgres;
pub mod receive;

use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, Set};

use super::{notice::Notice, DomainEvent};
use crate::{
    models::event_outbox,
    shared::errors::{AppError, AppResult},
};

/// A saída de eventos para os outros processos.
#[async_trait]
pub trait EventFanout: Send + Sync {
    /// Grava o evento de domínio no `event_outbox` e, havendo outros
    /// processos, os avisa **no commit** dessa gravação.
    ///
    /// # Errors
    ///
    /// Erro do banco ao gravar.
    async fn persist(
        &self,
        db: &DatabaseConnection,
        origin: &str,
        event: &DomainEvent,
    ) -> AppResult<()>;

    /// Avisa os outros processos sem esperar o banco (snapshot, esquecimento,
    /// efêmero, pedido de reenvio). Chamado de código síncrono: quem implementa
    /// não bloqueia.
    fn announce(&self, notice: Notice);
}

/// Um processo só: o outbox recebe a linha e ninguém mais precisa de aviso.
pub struct Standalone;

#[async_trait]
impl EventFanout for Standalone {
    async fn persist(
        &self,
        db: &DatabaseConnection,
        origin: &str,
        event: &DomainEvent,
    ) -> AppResult<()> {
        outbox_row(origin, event, None)?.insert(db).await?;
        Ok(())
    }

    fn announce(&self, _notice: Notice) {}
}

/// A linha do `event_outbox` de um evento (ou de um snapshot, com a chave).
///
/// # Errors
///
/// Evento que não serializa (não acontece com `serde_json::Value`).
pub fn outbox_row(
    origin: &str,
    event: &DomainEvent,
    snapshot_key: Option<String>,
) -> AppResult<event_outbox::ActiveModel> {
    Ok(event_outbox::ActiveModel {
        r#type: Set(event.event_type.clone()),
        origin: Set(origin.to_string()),
        payload: Set(serde_json::to_value(event)
            .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?),
        snapshot_key: Set(snapshot_key),
        ..Default::default()
    })
}

/// Grava a linha e devolve o `id` dela.
pub(crate) async fn insert_row<C: ConnectionTrait>(
    db: &C,
    origin: &str,
    event: &DomainEvent,
    snapshot_key: Option<String>,
) -> AppResult<i64> {
    Ok(outbox_row(origin, event, snapshot_key)?
        .insert(db)
        .await?
        .id)
}
