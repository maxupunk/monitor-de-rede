//! Ponto de extensão dos comandos de área no host do agente (ADR 017,
//! `AgentCommandHandler`).
//!
//! O **enum** dos comandos continua fechado em `agent-protocol` (AGENTS §7) e a
//! política local (`AGENT_ALLOW`) continua aplicada pelo executor do agente,
//! antes de qualquer efeito. O que muda é quem executa: o comando de uma área
//! (descoberta, acesso a equipamento, vazão — da Rede) é atendido pelo
//! handler que o crate dela contribui, registrado pelo ponto de montagem.
//! Assim o executor não conhece área nenhuma.

use std::sync::Arc;

use agent_protocol::{Command, RemoteError};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
