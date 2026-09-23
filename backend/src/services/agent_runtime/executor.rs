//! Execução dos pedidos da central, no host do agente.
//!
//! É aqui que a política local é aplicada — a cada pedido, antes de qualquer
//! efeito (ADR 011). As operações usam o **mesmo** código da central:
//! `LocalEngine` para o Docker, `run_monitor_with`/`scan_network_with` para
//! as checagens, `compose` para os projetos.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::services::{
    agents::{
        policy::Policy,
        protocol::{Command, DockerCall, ErrorCode, RemoteError},
    },
    discovery::service::scan_network_with,
    docker::{
        compose,
        engine::{self, ContainerAction},
        log_clear::is_current_container,
        maintenance::{ComposeRequest, DockerMaintenance, Progress},
        source::{DockerEngine, LocalEngine, LogChunk},
        DockerError,
    },
    monitoring::runner::{run_monitor_with, CheckDeps, RunOptions},
    probes::agent::failed_result,
};

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

fn docker<T: Serialize>(result: Result<T, DockerError>) -> Result<Value, RemoteError> {
    result.map_err(RemoteError::from).and_then(to_value)
}

/// Linhas de progresso de texto viram chunks de string.
fn progress_to_chunks(
    chunks: mpsc::UnboundedSender<Value>,
) -> (Progress, tokio::task::JoinHandle<()>) {
    let (progress, mut lines) = mpsc::unbounded_channel::<String>();
    let forward = tokio::spawn(async move {
        while let Some(line) = lines.recv().await {
            if chunks.send(Value::String(line)).is_err() {
                break;
            }
        }
    });
    (progress, forward)
}

pub struct LocalExecutor {
    policy: Policy,
    engine: Arc<LocalEngine>,
    deps: CheckDeps,
}

impl LocalExecutor {
    #[must_use]
    pub fn new(policy: Policy, deps: CheckDeps) -> Self {
        Self {
            policy,
            engine: Arc::new(LocalEngine),
            deps,
        }
    }

    /// Nenhuma mutação atinge o container que executa o próprio agente:
    /// derrubá-lo cortaria a central do host sem volta.
    async fn guard_self(&self, id: &str) -> Result<(), RemoteError> {
        let inspected = self
            .engine
            .inspect_container_raw(id)
            .await
            .map_err(RemoteError::from)?;
        let full_id = inspected.get("Id").and_then(Value::as_str).unwrap_or(id);
        if is_current_container(full_id) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                "O agente não executa ações sobre o próprio container",
            ));
        }
        Ok(())
    }

    async fn compose(
        &self,
        request: &ComposeRequest,
        progress: &Progress,
    ) -> Result<Value, RemoteError> {
        compose::availability().await.map_err(|reason| {
            RemoteError::new(
                ErrorCode::Unsupported,
                format!("Compose indisponível: {reason}"),
            )
        })?;
        let containers = engine::list_containers(self.engine.as_ref())
            .await
            .map_err(RemoteError::from)?;
        let args = compose::build_args(&compose::projects(&containers), request)
            .map_err(RemoteError::from)?;
        docker(compose::run(args, request, progress).await)
    }

    async fn follow(
        &self,
        id: &str,
        tail: &str,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let (logs, mut received) = mpsc::unbounded_channel::<LogChunk>();
        let forward = tokio::spawn(async move {
            while let Some(chunk) = received.recv().await {
                let Ok(value) = serde_json::to_value(chunk) else {
                    continue;
                };
                if chunks.send(value).is_err() {
                    break;
                }
            }
        });
        let result = self.engine.follow_logs(id, tail, logs, cancel).await;
        let _ = forward.await;
        docker(result.map(|()| Value::Null))
    }

    #[allow(clippy::too_many_lines)]
    async fn docker(
        &self,
        call: DockerCall,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let engine = self.engine.as_ref();
        match call {
            DockerCall::Status => docker(engine.status_raw().await),
            DockerCall::ListContainers => docker(engine.list_containers_raw().await),
            DockerCall::InspectContainer { id } => docker(engine.inspect_container_raw(&id).await),
            DockerCall::ContainerAction { id, action } => {
                if action != ContainerAction::Start {
                    self.guard_self(&id).await?;
                }
                docker(engine.container_action(&id, action).await)
            }
            DockerCall::Logs {
                id,
                filters,
                max_lines,
            } => docker(
                engine
                    .container_logs_raw(&id, &filters, max_lines.min(10_000))
                    .await,
            ),
            DockerCall::FollowLogs { id, tail } => self.follow(&id, &tail, chunks, cancel).await,
            DockerCall::ClearLogs { id } => docker(engine.clear_logs(&id).await),
            DockerCall::Metrics => to_value(engine.metrics().await),
            DockerCall::ListVolumes => docker(engine.list_volumes_raw().await),
            DockerCall::InspectVolume { name } => docker(engine.inspect_volume_raw(&name).await),
            DockerCall::RemoveVolume { name, force } => {
                docker(engine.remove_volume(&name, force).await)
            }
            DockerCall::ListNetworks => docker(engine.list_networks_raw().await),
            DockerCall::InspectNetwork { id } => docker(engine.inspect_network_raw(&id).await),
            DockerCall::CreateNetwork { name, driver } => {
                docker(engine.create_network(&name, &driver).await)
            }
            DockerCall::RemoveNetwork { id } => docker(engine.remove_network(&id).await),
            DockerCall::ConnectNetwork { network, container } => {
                docker(engine.connect_network(&network, &container).await)
            }
            DockerCall::DisconnectNetwork {
                network,
                container,
                force,
            } => {
                self.guard_self(&container).await?;
                docker(engine.disconnect_network(&network, &container, force).await)
            }
            DockerCall::ListImages => docker(engine.list_images_raw().await),
            DockerCall::InspectImage { id } => docker(engine.inspect_image_raw(&id).await),
            DockerCall::RemoveImage { id, force } => docker(engine.remove_image(&id, force).await),
            DockerCall::PruneImages => docker(engine.prune_images_raw().await),
            DockerCall::PullImage { image } => {
                let (progress, forward) = progress_to_chunks(chunks);
                let result = engine.pull_image(&image, progress).await;
                let _ = forward.await;
                docker(result)
            }
            DockerCall::UpdateContainer { id } => {
                self.guard_self(&id).await?;
                let (progress, forward) = progress_to_chunks(chunks);
                let result = engine.update_container(&id, progress).await;
                let _ = forward.await;
                docker(result)
            }
            DockerCall::Compose { request } => {
                let (progress, forward) = progress_to_chunks(chunks);
                let result = self.compose(&request, &progress).await;
                drop(progress);
                let _ = forward.await;
                result
            }
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
            Command::Docker { call } => self.docker(call, chunks, cancel).await,
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
            Command::Discovery { task } => {
                let hosts = scan_network_with(&self.deps, &task.cidr, cancel)
                    .await
                    .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?;
                to_value(hosts)
            }
            // O modo ao vivo é da sessão, não do executor.
            Command::SetLive { .. } => Ok(Value::Null),
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

    #[tokio::test]
    async fn politica_local_nega_antes_de_tocar_o_docker() {
        let executor = LocalExecutor::new(
            Policy::from_permissions([Permission::Read]),
            CheckDeps::default(),
        );
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
        let executor = LocalExecutor::new(Policy::default(), CheckDeps::default());
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
        let executor = LocalExecutor::new(Policy::default(), CheckDeps::default());
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
