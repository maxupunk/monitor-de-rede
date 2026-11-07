//! Execução dos pedidos da central, no host do agente.
//!
//! É aqui que a política local é aplicada — a cada pedido, antes de qualquer
//! efeito (ADR 011). As checagens usam o **mesmo** código da central
//! (`run_monitor_with`). Os comandos das áreas — o Docker (compose incluído),
//! de Servidores; descoberta, equipamento e vazão, da Rede — vão aos
//! `AgentCommandHandler`s que o binário do agente monta.

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::services::{
    agents::{
        policy::Policy,
        protocol::{Command, ErrorCode, RemoteError},
    },
    monitoring::runner::{run_monitor_with, CheckDeps, RunOptions},
    probes::agent::failed_result,
};

use netmonitor_core::agents::command::AgentCommandHandlers;

/// Contrato do executor: a sessão do agente só conhece isto.
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    /// Executa o pedido. Linhas intermediárias (progresso, log) vão em
    /// `chunks`; o valor final é a resposta.
    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError>;
}

fn to_value<T: Serialize>(value: T) -> Result<Value, RemoteError> {
    serde_json::to_value(value)
        .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))
}

pub struct LocalExecutor {
    policy: Policy,
    deps: CheckDeps,
    /// Os comandos das áreas (Docker, descoberta, equipamento, vazão),
    /// montados pelo binário do agente (ADR 017, `AgentCommandHandler`).
    handlers: AgentCommandHandlers,
}

impl LocalExecutor {
    #[must_use]
    pub const fn new(policy: Policy, deps: CheckDeps, handlers: AgentCommandHandlers) -> Self {
        Self {
            policy,
            deps,
            handlers,
        }
    }
}

#[async_trait]
impl CommandExecutor for LocalExecutor {
    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let permission = command.permission();
        if !self.policy.allows(permission) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                format!("A política deste host não libera a permissão '{permission}'"),
            ));
        }
        match command {
            Command::Monitor { task } => {
                let result = run_monitor_with(
                    &self.deps,
                    &task.task_type,
                    &task.payload,
                    RunOptions {
                        timeout_ms: u64::try_from(task.timeout_ms).ok(),
                    },
                )
                .await
                .unwrap_or_else(|error| failed_result(&error.to_string()));
                to_value(result)
            }
            // O modo ao vivo é da sessão, não do executor.
            Command::SetLive { .. } => Ok(Value::Null),
            // A ponte precisa dos quadros binários do canal: é da sessão.
            Command::DatabaseTunnel { .. } | Command::VolumeExport { .. } => Err(RemoteError::new(
                ErrorCode::Unsupported,
                "pontes são atendidas pela sessão do agente",
            )),
            // Os comandos das áreas: Docker (Servidores); descoberta,
            // equipamento e vazão (Rede).
            other => match self.handlers.find(&other) {
                Some(handler) => handler.execute(other, chunks, cancel).await,
                None => Err(RemoteError::new(
                    ErrorCode::Unsupported,
                    "comando não atendido por este agente",
                )),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{
        agents::{policy::Permission, protocol::DockerCall},
        probes::dispatcher::ProbeTask,
    };

    /// Sem área nenhuma: o que se testa aqui é do executor (política local e
    /// monitor). Os comandos das áreas são testados em
    /// `tests/requests/agent_commands.rs`, com os handlers do ponto de montagem.
    fn executor(policy: Policy) -> LocalExecutor {
        LocalExecutor::new(
            policy,
            CheckDeps::default(),
            AgentCommandHandlers::default(),
        )
    }

    #[tokio::test]
    async fn politica_local_nega_antes_de_tocar_o_docker() {
        let executor = executor(Policy::from_permissions([Permission::Read]));
        let (chunks, _) = mpsc::unbounded_channel();
        let error = executor
            .execute(
                Command::Docker {
                    call: DockerCall::UpdateContainer { id: "web".into() },
                },
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect_err("negado");
        assert_eq!(error.code, ErrorCode::Forbidden);
    }

    #[tokio::test]
    async fn monitor_executa_o_mesmo_checker_da_central() {
        let executor = executor(Policy::default());
        let (chunks, _) = mpsc::unbounded_channel();
        let value = executor
            .execute(
                Command::Monitor {
                    task: ProbeTask {
                        id: "task-1".into(),
                        monitor_id: 1,
                        task_type: "tcp".into(),
                        timeout_ms: 1_000,
                        payload: serde_json::json!({ "host": "127.0.0.1", "port": 1 }),
                    },
                },
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect("resultado");
        assert!(value.get("status").is_some());
    }

    #[tokio::test]
    async fn falha_de_execucao_vira_observacao_down() {
        let executor = executor(Policy::default());
        let (chunks, _) = mpsc::unbounded_channel();
        let value = executor
            .execute(
                Command::Monitor {
                    task: ProbeTask {
                        id: "task-2".into(),
                        monitor_id: 2,
                        task_type: "inexistente".into(),
                        timeout_ms: 1_000,
                        payload: serde_json::json!({}),
                    },
                },
                chunks,
                CancellationToken::new(),
            )
            .await
            .expect("observação");
        assert_eq!(value["status"], "down");
    }
}
