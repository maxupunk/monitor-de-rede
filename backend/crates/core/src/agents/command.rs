//! Pontos de extensão dos comandos de área (ADR 017, `AgentCommandHandler`).
//!
//! Os dois lados de um comando de área são registrados pelo ponto de
//! montagem: na central, o [`AgentCommandDispatcher`] entrega ao agente
//! conectado o trabalho que a área enfileirou; no host do agente, o
//! [`AgentCommandHandler`] o executa.
//!
//! O **enum** dos comandos continua fechado em `agent-protocol` (AGENTS §7) e a
//! política local (`AGENT_ALLOW`) continua aplicada pelo executor do agente,
//! antes de qualquer efeito. O que muda é quem executa: o comando de uma área
//! (descoberta, acesso a equipamento, vazão — da Rede) é atendido pelo
//! handler que o crate dela contribui, registrado pelo ponto de montagem.
//! Assim o executor não conhece área nenhuma.

use std::sync::Arc;

use agent_protocol::{AgentEvent, Command, RemoteError};
use async_trait::async_trait;
use loco_rs::app::AppContext;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::session::AgentSession;
use crate::shared::errors::AppResult;

/// Um comando de área executado no host do agente.
#[async_trait]
pub trait AgentCommandHandler: Send + Sync {
    /// Atende este comando?
    fn handles(&self, command: &Command) -> bool;

    /// Executa o comando. Linhas intermediárias (progresso, amostras, saída
    /// do equipamento) vão em `chunks`; o valor final é a resposta. A
    /// política local já foi conferida por quem chama.
    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError>;
}

/// Os handlers registrados no agente.
#[derive(Clone, Default)]
pub struct AgentCommandHandlers(Arc<Vec<Arc<dyn AgentCommandHandler>>>);

impl AgentCommandHandlers {
    #[must_use]
    pub fn new(handlers: Vec<Arc<dyn AgentCommandHandler>>) -> Self {
        Self(Arc::new(handlers))
    }

    /// O handler do comando, se alguma área o atende.
    #[must_use]
    pub fn find(&self, command: &Command) -> Option<&dyn AgentCommandHandler> {
        self.0
            .iter()
            .find(|handler| handler.handles(command))
            .map(AsRef::as_ref)
    }
}

/// O lado central de um comando de área: entrega ao agente conectado o
/// trabalho que a área enfileirou (a varredura, da Rede). Chamado a cada
/// rodada do pump dos agentes.
#[async_trait]
pub trait AgentCommandDispatcher: Send + Sync {
    /// Despacha o trabalho pendente para `session`. A permissão anunciada
    /// pelo agente (`AGENT_ALLOW`) é conferida aqui.
    ///
    /// # Errors
    ///
    /// Erro do banco ao reivindicar o trabalho.
    async fn dispatch(&self, ctx: &AppContext, session: &Arc<AgentSession>) -> AppResult<()>;

    /// O mesmo trabalho para o probe que busca tarefas por HTTP
    /// (`GET /api/probes/tasks`), como comandos. Quem entrega marca o
    /// trabalho como reivindicado. O padrão é não haver nenhum.
    ///
    /// # Errors
    ///
    /// Erro do banco ao reivindicar o trabalho.
    async fn pending_for_probe(
        &self,
        _ctx: &AppContext,
        _probe_id: i64,
    ) -> AppResult<Vec<Command>> {
        Ok(Vec::new())
    }

    /// Um evento do agente (ou um resultado do probe por HTTP) que é desta
    /// área — o retorno de uma varredura. Devolve `true` quando o tratou.
    ///
    /// # Errors
    ///
    /// Erro do banco ao gravar o retorno.
    async fn receive(
        &self,
        _ctx: &AppContext,
        _probe_id: i64,
        _event: &AgentEvent,
    ) -> AppResult<bool> {
        Ok(false)
    }
}

/// Os despachantes registrados na central, no `shared_store`.
#[derive(Clone, Default)]
pub struct AgentCommandDispatchers(Arc<Vec<Arc<dyn AgentCommandDispatcher>>>);

impl AgentCommandDispatchers {
    #[must_use]
    pub fn new(dispatchers: Vec<Arc<dyn AgentCommandDispatcher>>) -> Self {
        Self(Arc::new(dispatchers))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn AgentCommandDispatcher>> {
        self.0.iter()
    }

    /// Os do processo; vazio quando o ponto de montagem não registrou nenhum.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        ctx.shared_store.get::<Self>().unwrap_or_default()
    }

    /// O trabalho pendente das áreas para o probe que busca por HTTP.
    ///
    /// # Errors
    ///
    /// Erro do banco ao reivindicar.
    pub async fn pending_for_probe(
        &self,
        ctx: &AppContext,
        probe_id: i64,
    ) -> AppResult<Vec<Command>> {
        let mut commands = Vec::new();
        for dispatcher in self.iter() {
            commands.extend(dispatcher.pending_for_probe(ctx, probe_id).await?);
        }
        Ok(commands)
    }

    /// Entrega o evento à área que o trata. `false` quando nenhuma tratou.
    ///
    /// # Errors
    ///
    /// Erro da área ao gravar o retorno.
    pub async fn receive(
        &self,
        ctx: &AppContext,
        probe_id: i64,
        event: &AgentEvent,
    ) -> AppResult<bool> {
        for dispatcher in self.iter() {
            if dispatcher.receive(ctx, probe_id, event).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
