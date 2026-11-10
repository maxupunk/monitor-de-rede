//! O Docker do host do agente (ADR 011, ADR 014): o lado do agente.
//!
//! Registrado pelo binário do agente: os comandos Docker (`Command::Docker`)
//! como `AgentCommandHandler` e o resto do que o runtime do agente pede do
//! Docker deste host — o `Hello`, as métricas do amostrador, o estado ao vivo
//! e a exportação de volume — como [`HostDocker`]. A política local
//! (`AGENT_ALLOW`) é aplicada pelo executor antes de chegar aqui.

use std::sync::Arc;

use agent_protocol::{
    docker::views::{DockerInventorySnapshot, DockerLiveSnapshot, DockerMetricsResponse},
    tunnel::TunnelStream,
    Capability, Command, DockerCall, ErrorCode, RemoteError,
};
use async_trait::async_trait;
use netmonitor_core::agents::{command::AgentCommandHandler, host::HostDocker};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::docker::{
    builds::{BuildChunk, ImageHost, ImageImportRequest},
    compose,
    engine::ContainerAction,
    maintenance::{DockerMaintenance, Progress},
    protection::{is_current_container, is_protected_inspect},
    realtime::{inventory_snapshot, live_snapshot},
    source::{DockerEngine, LocalEngine, LogChunk},
    stacks::{StackChunk, StackHost},
    DockerError,
};

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

/// Os comandos Docker no host do agente, com a Engine local (compose
/// incluído).
pub struct DockerCommands {
    engine: Arc<LocalEngine>,
}

impl Default for DockerCommands {
    fn default() -> Self {
        Self {
            engine: Arc::new(LocalEngine),
        }
    }
}

impl DockerCommands {
    /// Nenhuma mutação atinge o container que executa o próprio agente nem
    /// um container protegido da instalação (a central, ao lado do agente
    /// local): derrubá-los cortaria a central do host sem volta (ADR 014).
    async fn guard(&self, id: &str) -> Result<(), RemoteError> {
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
        if is_protected_inspect(&inspected) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                "Container protegido da instalação NetMonitor: a ação derrubaria a central",
            ));
        }
        Ok(())
    }

    /// Operação longa: o progresso vira chunks de texto até o fim.
    async fn long<T, F>(
        chunks: mpsc::UnboundedSender<Value>,
        operation: impl FnOnce(Progress) -> F,
    ) -> Result<Value, RemoteError>
    where
        T: Serialize,
        F: std::future::Future<Output = Result<T, DockerError>>,
    {
        let (progress, forward) = progress_to_chunks(chunks);
        let result = operation(progress).await;
        let _ = forward.await;
        docker(result)
    }

    /// Operação com andamento tipado (`StackChunk`, `BuildChunk`): cada
    /// evento vai num chunk em JSON até o fim.
    async fn streamed<C, T, F>(
        chunks: mpsc::UnboundedSender<Value>,
        operation: impl FnOnce(mpsc::UnboundedSender<C>) -> F,
    ) -> Result<Value, RemoteError>
    where
        C: Serialize + Send + 'static,
        T: Serialize,
        F: std::future::Future<Output = Result<T, DockerError>>,
    {
        let (events, mut received) = mpsc::unbounded_channel::<C>();
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
        let result = operation(events).await;
        let _ = forward.await;
        docker(result)
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
                    self.guard(&id).await?;
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
                self.guard(&container).await?;
                docker(engine.disconnect_network(&network, &container, force).await)
            }
            DockerCall::ListImages => docker(engine.list_images_raw().await),
            DockerCall::InspectImage { id } => docker(engine.inspect_image_raw(&id).await),
            DockerCall::RemoveImage { id, force } => docker(engine.remove_image(&id, force).await),
            DockerCall::PruneImages => docker(engine.prune_images_raw().await),
            DockerCall::PullImage { image } => {
                Self::long(chunks, |progress| async move {
                    engine.pull_image(&image, progress).await
                })
                .await
            }
            DockerCall::UpdateContainer { id } => {
                self.guard(&id).await?;
                Self::long(chunks, |progress| async move {
                    engine.update_container(&id, progress).await
                })
                .await
            }
            DockerCall::Compose { request } => {
                Self::long(chunks, |progress| async move {
                    engine.compose(&request, progress).await
                })
                .await
            }
            DockerCall::StackDeploy { request } => {
                Self::streamed::<StackChunk, _, _>(chunks, |events| async move {
                    engine.deploy_stack(&request, events).await
                })
                .await
            }
            DockerCall::ImageBuild { request } => {
                Self::streamed::<BuildChunk, _, _>(chunks, |events| async move {
                    engine.build_images(&request, events, cancel).await
                })
                .await
            }
            DockerCall::BuilderReadiness => docker(engine.builder_readiness().await),
            DockerCall::StackReadiness { project } => {
                docker(engine.stack_readiness(&project).await)
            }
        }
    }
}

#[async_trait]
impl AgentCommandHandler for DockerCommands {
    fn handles(&self, command: &Command) -> bool {
        matches!(command, Command::Docker { .. })
    }

    async fn execute(
        &self,
        command: Command,
        chunks: mpsc::UnboundedSender<Value>,
        cancel: CancellationToken,
    ) -> Result<Value, RemoteError> {
        let Command::Docker { call } = command else {
            return Err(RemoteError::new(
                ErrorCode::Internal,
                "comando entregue ao handler errado",
            ));
        };
        self.docker(call, chunks, cancel).await
    }
}

/// O Docker deste host para o runtime do agente.
pub struct LocalHostDocker;

async fn docker_capability() -> Capability {
    match LocalEngine.status_raw().await {
        Ok(raw) => Capability {
            available: true,
            version: raw
                .version
                .get("Version")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            reason: None,
        },
        Err(DockerError::Disabled) => Capability {
            available: false,
            version: None,
            reason: Some("DOCKER_ENABLED=false neste host".into()),
        },
        Err(_) => Capability {
            available: false,
            version: None,
            reason: Some("Docker Engine inacessível pelo socket".into()),
        },
    }
}

async fn compose_capability() -> Capability {
    match compose::availability().await {
        Ok(version) => Capability {
            available: true,
            version: Some(version),
            reason: None,
        },
        Err(reason) => Capability {
            available: false,
            version: None,
            reason: Some(reason),
        },
    }
}

#[async_trait]
impl HostDocker for LocalHostDocker {
    async fn capabilities(&self) -> (Capability, Capability) {
        tokio::join!(docker_capability(), compose_capability())
    }

    async fn metrics(&self) -> DockerMetricsResponse {
        LocalEngine.metrics().await
    }

    async fn live(&self) -> Option<DockerLiveSnapshot> {
        live_snapshot(&LocalEngine, LocalEngine.metrics())
            .await
            .ok()
    }

    async fn inventory(&self) -> Option<DockerInventorySnapshot> {
        inventory_snapshot(&LocalEngine).await.ok()
    }

    async fn open_artifact(&self, artifact: uuid::Uuid) -> Result<TunnelStream, RemoteError> {
        LocalEngine
            .open_artifact(artifact)
            .await
            .map_err(RemoteError::from)
    }

    async fn open_import(&self, request: &ImageImportRequest) -> Result<TunnelStream, RemoteError> {
        LocalEngine
            .open_import(request)
            .await
            .map_err(RemoteError::from)
    }

    async fn export_volume(&self, name: &str) -> Result<TunnelStream, RemoteError> {
        let archive = LocalEngine
            .export_volume(name)
            .await
            .map_err(RemoteError::from)?;
        Ok(Box::new(tokio::io::join(archive.reader, tokio::io::sink())))
    }
}
