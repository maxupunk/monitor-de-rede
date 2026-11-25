//! Vários processos da central sobre o mesmo banco (ADR 024).
//!
//! Dois problemas, uma escolha de dialeto feita **uma vez**, no boot
//! ([`Coordination::for_database`]):
//!
//! * **Eventos entre processos** — no PostgreSQL, `LISTEN`/`NOTIFY`
//!   ([`crate::events::fanout::postgres`]); no SQLite, nada a entregar.
//! * **Quem roda as rotinas** — no PostgreSQL, o processo que segura a trava
//!   consultiva da função ([`leadership`]); no SQLite, sempre o próprio
//!   processo.
//!
//! Nenhum outro ponto do código pergunta pelo dialeto para decidir isso: quem
//! precisa de liderança pede a [`leadership::Leadership`], quem publica
//! evento usa o [`crate::events::EventBus`].

pub mod advisory;
pub mod leadership;

use std::sync::Arc;

use sea_orm::{DatabaseBackend, DatabaseConnection};

use crate::events::{
    fanout::postgres::{spawn_listener, PgFanout},
    EventBus,
};
use leadership::{LeaderLock, LeaderRole};

/// Como os processos desta central se coordenam.
#[derive(Clone)]
pub enum Coordination {
    /// SQLite: um processo só, sempre líder, sem ninguém a avisar.
    Standalone,
    /// PostgreSQL: avisos por `NOTIFY` e liderança por trava consultiva.
    Postgres(DatabaseConnection),
}

impl Coordination {
    /// A coordenação que o banco do processo permite.
    #[must_use]
    pub fn for_database(db: &DatabaseConnection) -> Self {
        match db.get_database_backend() {
            DatabaseBackend::Postgres => Self::Postgres(db.clone()),
            _ => Self::Standalone,
        }
    }

    /// O barramento do processo. No PostgreSQL já sobe a tarefa de envio e a
    /// escuta dos avisos — em **todo** processo, inclusive o de `task`: é a
    /// escuta que responde ao `Resync` com os snapshots que ele produziu.
    /// Precisa de um runtime tokio.
    #[must_use]
    pub fn start_event_bus(&self) -> EventBus {
        match self {
            Self::Standalone => EventBus::create(),
            Self::Postgres(db) => {
                let bus = EventBus::with_fanout(PgFanout::start(db.clone()));
                spawn_listener(db.clone(), bus.clone());
                bus
            }
        }
    }

    /// A trava de liderança de uma função; `None` quando o processo é sempre
    /// líder.
    #[must_use]
    pub fn leader_lock(&self, role: LeaderRole) -> Option<Arc<dyn LeaderLock>> {
        match self {
            Self::Standalone => None,
            Self::Postgres(db) => Some(Arc::new(advisory::PgAdvisoryLock::new(
                db.clone(),
                role.lock_key(),
            ))),
        }
    }
}
