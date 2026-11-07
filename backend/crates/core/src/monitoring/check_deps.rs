//! Dependências de processo que os checkers usam (o cliente ICMP e o
//! registro de `MonitorChecker`s).

use loco_rs::app::AppContext;

use super::MonitorCheckers;
use crate::{
    network_tools::ping_client::{PingClient, PingClientCell},
    shared::errors::{AppError, AppResult},
};

/// Dependências de processo que os checkers usam. O servidor as tira do
/// `AppContext`; o agente remoto as monta direto, sem subir o Loco nem banco.
#[derive(Clone, Default)]
pub struct CheckDeps {
    /// Opcional porque só o ping precisa dele: sem o socket ICMP, os demais
    /// tipos continuam rodando e só o ping falha — como antes do refactor.
    /// A célula tenta abrir de novo a cada uso (ver [`PingClientCell`]).
    pub ping: Option<PingClientCell>,
    /// Os checkers das áreas (`MonitorChecker`). Na central vêm do
    /// `shared_store` (montados em `process_deps`); no agente, do cliente,
    /// com a Docker Engine local.
    pub checkers: MonitorCheckers,
}

impl CheckDeps {
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        Self {
            ping: ctx.shared_store.get::<PingClientCell>(),
            checkers: ctx
                .shared_store
                .get::<MonitorCheckers>()
                .unwrap_or_default(),
        }
    }

    /// Cliente ICMP ou o mesmo erro que `PingClient::from_context` devolveria.
    pub fn ping_client(&self) -> AppResult<PingClient> {
        self.ping
            .as_ref()
            .ok_or_else(|| {
                AppError::ServiceUnavailable("Ping não habilitado neste processo".to_string())
            })?
            .client()
    }
}
