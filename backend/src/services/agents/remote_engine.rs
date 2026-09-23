//! Docker de um servidor remoto, visto pela central através do agente.
//!
//! Implementa os mesmos traits do `LocalEngine`: para o controller e para o
//! mapeamento de `docker/engine.rs`, um host remoto é só outra fonte. Do lado
//! de lá, o agente executa o **mesmo** `LocalEngine` (ADR 011).

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    services::docker::{
        engine::{ContainerAction, LogFilters},
        maintenance::{ComposeRequest, DockerMaintenance, Progress},
        metrics,
        source::{DockerEngine, EngineStatusRaw, LogChunk},
        DockerError,
    },
    views::docker::{DockerActionResponse, DockerMetricsResponse},
};

use super::{
    protocol::{Command, DockerCall},
    session::{AgentSession, Call},
};

/// Consultas e ciclo de vida: generoso para a volta pelo túnel, curto o
/// bastante para a tela não ficar pendurada.
const QUERY_TIMEOUT: Duration = Duration::from_secs(25);
/// Pull, recreate e compose podem baixar imagens grandes.
const OPERATION_TIMEOUT: Duration = Duration::from_secs(35 * 60);
/// Teto de um follow de logs; a central encerra antes (ver `log_stream`).
const FOLLOW_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct AgentEngine {
    session: Arc<AgentSession>,
}

impl AgentEngine {
    #[must_use]
    pub const fn new(session: Arc<AgentSession>) -> Self {
        Self { session }
    }

    async fn call(&self, call: DockerCall, timeout: Duration) -> Result<Value, DockerError> {
        self.session
            .request(Command::Docker { call }, timeout)
            .await
            .map_err(DockerError::from)
    }

    async fn query<T: DeserializeOwned>(&self, call: DockerCall) -> Result<T, DockerError> {
        decode(self.call(call, QUERY_TIMEOUT).await?)
    }

    async fn unit(&self, call: DockerCall) -> Result<(), DockerError> {
        self.call(call, QUERY_TIMEOUT).await.map(|_| ())
    }

    /// Operação longa: cada chunk de texto do agente vira uma linha de progresso.
    async fn operation(
        &self,
        call: DockerCall,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        let (chunks, mut received) = mpsc::unbounded_channel::<Value>();
        let forward = tokio::spawn(async move {
            while let Some(chunk) = received.recv().await {
                if let Some(line) = chunk.as_str() {
                    let _ = progress.send(line.to_string());
                }
            }
        });
        let result = self
            .session
            .call(Call::new(Command::Docker { call }, OPERATION_TIMEOUT).with_chunks(chunks))
            .await
            .map_err(DockerError::from);
        let _ = forward.await;
        decode(result?)
    }
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, DockerError> {
    serde_json::from_value(value).map_err(|_| DockerError::Engine)
}

#[async_trait]
impl DockerEngine for AgentEngine {
    async fn status_raw(&self) -> Result<EngineStatusRaw, DockerError> {
        self.query(DockerCall::Status).await
    }

    async fn list_containers_raw(&self) -> Result<Vec<Value>, DockerError> {
        self.query(DockerCall::ListContainers).await
    }

    async fn inspect_container_raw(&self, id: &str) -> Result<Value, DockerError> {
        self.query(DockerCall::InspectContainer { id: id.into() })
            .await
    }

    async fn container_action(&self, id: &str, action: ContainerAction) -> Result<(), DockerError> {
        self.unit(DockerCall::ContainerAction {
            id: id.into(),
            action,
        })
        .await
    }

    async fn container_logs_raw(
        &self,
        id: &str,
        filters: &LogFilters,
        max_lines: usize,
    ) -> Result<Vec<LogChunk>, DockerError> {
        self.query(DockerCall::Logs {
            id: id.into(),
            filters: filters.clone(),
            max_lines,
        })
        .await
    }

    async fn list_volumes_raw(&self) -> Result<Value, DockerError> {
        self.query(DockerCall::ListVolumes).await
    }

    async fn inspect_volume_raw(&self, name: &str) -> Result<Value, DockerError> {
        self.query(DockerCall::InspectVolume { name: name.into() })
            .await
    }

    async fn remove_volume(&self, name: &str, force: bool) -> Result<(), DockerError> {
        self.unit(DockerCall::RemoveVolume {
            name: name.into(),
            force,
        })
        .await
    }

    async fn list_networks_raw(&self) -> Result<Vec<Value>, DockerError> {
        self.query(DockerCall::ListNetworks).await
    }

    async fn inspect_network_raw(&self, id: &str) -> Result<Value, DockerError> {
        self.query(DockerCall::InspectNetwork { id: id.into() })
            .await
    }

    async fn create_network(&self, name: &str, driver: &str) -> Result<(), DockerError> {
        self.unit(DockerCall::CreateNetwork {
            name: name.into(),
            driver: driver.into(),
        })
        .await
    }

    async fn remove_network(&self, id: &str) -> Result<(), DockerError> {
        self.unit(DockerCall::RemoveNetwork { id: id.into() }).await
    }

    async fn connect_network(&self, network: &str, container: &str) -> Result<(), DockerError> {
        self.unit(DockerCall::ConnectNetwork {
            network: network.into(),
            container: container.into(),
        })
        .await
    }

    async fn disconnect_network(
        &self,
        network: &str,
        container: &str,
        force: bool,
    ) -> Result<(), DockerError> {
        self.unit(DockerCall::DisconnectNetwork {
            network: network.into(),
            container: container.into(),
            force,
        })
        .await
    }

    async fn list_images_raw(&self) -> Result<Vec<Value>, DockerError> {
        self.query(DockerCall::ListImages).await
    }

    async fn inspect_image_raw(&self, id: &str) -> Result<Value, DockerError> {
        self.query(DockerCall::InspectImage { id: id.into() }).await
    }

    async fn remove_image(&self, id: &str, force: bool) -> Result<(), DockerError> {
        self.unit(DockerCall::RemoveImage {
            id: id.into(),
            force,
        })
        .await
    }

    async fn prune_images_raw(&self) -> Result<Value, DockerError> {
        self.query(DockerCall::PruneImages).await
    }

    async fn metrics(&self) -> DockerMetricsResponse {
        match self.query(DockerCall::Metrics).await {
            Ok(metrics) => metrics,
            Err(error) => metrics::unavailable(&error.to_string()),
        }
    }

    /// Stream até o cancelamento local, que vira `Cancel` para o agente.
    async fn follow_logs(
        &self,
        id: &str,
        tail: &str,
        chunks: mpsc::UnboundedSender<LogChunk>,
        cancel: CancellationToken,
    ) -> Result<(), DockerError> {
        let (raw, mut received) = mpsc::unbounded_channel::<Value>();
        let forward = tokio::spawn(async move {
            while let Some(value) = received.recv().await {
                if let Ok(chunk) = serde_json::from_value::<LogChunk>(value) {
                    if chunks.send(chunk).is_err() {
                        break;
                    }
                }
            }
        });
        let call = Call::new(
            Command::Docker {
                call: DockerCall::FollowLogs {
                    id: id.into(),
                    tail: tail.into(),
                },
            },
            FOLLOW_TIMEOUT,
        )
        .with_chunks(raw);
        let request_id = call.id;
        let result = tokio::select! {
            result = self.session.call(call) => result.map(|_| ()).map_err(DockerError::from),
            () = cancel.cancelled() => {
                self.session.cancel(request_id).await;
                Ok(())
            }
        };
        let _ = forward.await;
        result
    }
}

#[async_trait]
impl DockerMaintenance for AgentEngine {
    async fn clear_logs(&self, id: &str) -> Result<DockerActionResponse, DockerError> {
        decode(
            self.call(DockerCall::ClearLogs { id: id.into() }, OPERATION_TIMEOUT)
                .await?,
        )
    }

    async fn pull_image(
        &self,
        image: &str,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        self.operation(
            DockerCall::PullImage {
                image: image.into(),
            },
            progress,
        )
        .await
    }

    async fn update_container(
        &self,
        id: &str,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        self.operation(DockerCall::UpdateContainer { id: id.into() }, progress)
            .await
    }

    async fn compose(
        &self,
        request: &ComposeRequest,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        self.operation(
            DockerCall::Compose {
                request: request.clone(),
            },
            progress,
        )
        .await
    }
}
