//! Fonte dos dados crus da Docker Engine.
//!
//! O trait separa **de onde** vêm os dados (a Engine local via `bollard`, ou a
//! de outro servidor via agente) de **como** eles viram DTO — o mapeamento, a
//! redação de segredos e as regras ficam em `engine.rs`, iguais para qualquer
//! fonte. Os métodos devolvem o JSON da Engine sem interpretação.

use std::time::Duration;

use async_trait::async_trait;
use bollard::{
    container::{ListContainersOptions, LogsOptions, RemoveContainerOptions},
    image::{ListImagesOptions, RemoveImageOptions},
    network::{
        ConnectNetworkOptions, CreateNetworkOptions, DisconnectNetworkOptions, ListNetworksOptions,
    },
    volume::{ListVolumesOptions, RemoveVolumeOptions},
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::views::docker::DockerMetricsResponse;

use super::{
    call, client,
    engine::{ContainerAction, LogFilters},
    DockerError,
};

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

/// Respostas de `/version` e `/info`, ainda sem normalização.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineStatusRaw {
    pub version: Value,
    pub info: Value,
}

/// Bloco de log como a Engine o entrega: um fluxo e um texto com várias linhas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogChunk {
    pub stream: String,
    pub text: String,
}

#[async_trait]
pub trait DockerEngine: Send + Sync {
    /// `Err(Disabled)` quando a integração está desligada; `Err(Unavailable)`
    /// quando a Engine não responde ao ping.
    async fn status_raw(&self) -> Result<EngineStatusRaw, DockerError>;
    async fn list_containers_raw(&self) -> Result<Vec<Value>, DockerError>;
    async fn inspect_container_raw(&self, id: &str) -> Result<Value, DockerError>;
    async fn container_action(&self, id: &str, action: ContainerAction) -> Result<(), DockerError>;
    /// Lê até `max_lines` linhas não vazias; o corte fino é do mapeamento.
    async fn container_logs_raw(
        &self,
        id: &str,
        filters: &LogFilters,
        max_lines: usize,
    ) -> Result<Vec<LogChunk>, DockerError>;
    async fn list_volumes_raw(&self) -> Result<Value, DockerError>;
    async fn inspect_volume_raw(&self, name: &str) -> Result<Value, DockerError>;
    async fn remove_volume(&self, name: &str, force: bool) -> Result<(), DockerError>;
    async fn list_networks_raw(&self) -> Result<Vec<Value>, DockerError>;
    async fn inspect_network_raw(&self, id: &str) -> Result<Value, DockerError>;
    async fn create_network(&self, name: &str, driver: &str) -> Result<(), DockerError>;
    async fn remove_network(&self, id: &str) -> Result<(), DockerError>;
    async fn connect_network(&self, network: &str, container: &str) -> Result<(), DockerError>;
    async fn disconnect_network(
        &self,
        network: &str,
        container: &str,
        force: bool,
    ) -> Result<(), DockerError>;
    async fn list_images_raw(&self) -> Result<Vec<Value>, DockerError>;
    async fn inspect_image_raw(&self, id: &str) -> Result<Value, DockerError>;
    async fn remove_image(&self, id: &str, force: bool) -> Result<(), DockerError>;
    async fn prune_images_raw(&self) -> Result<Value, DockerError>;
    /// Métricas dos containers em execução, já calculadas na origem: a
    /// amostra de CPU depende de duas leituras consecutivas do mesmo host.
    async fn metrics(&self) -> DockerMetricsResponse;
    /// Acompanha o log até `cancel` ou até o container terminar, entregando
    /// cada bloco em `chunks`.
    async fn follow_logs(
        &self,
        id: &str,
        tail: &str,
        chunks: UnboundedSender<LogChunk>,
        cancel: CancellationToken,
    ) -> Result<(), DockerError>;
}

/// Engine do próprio processo, pelo socket local. É a mesma implementação que
/// o agente remoto usa do lado dele — a central nunca fala `bollard` com outro host.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalEngine;

#[async_trait]
impl DockerEngine for LocalEngine {
    async fn status_raw(&self) -> Result<EngineStatusRaw, DockerError> {
        let client = client()?;
        if !matches!(
            tokio::time::timeout(STATUS_TIMEOUT, client.ping()).await,
            Ok(Ok(_))
        ) {
            return Err(DockerError::Unavailable);
        }
        let version = tokio::time::timeout(STATUS_TIMEOUT, client.version())
            .await
            .ok()
            .and_then(Result::ok)
            .and_then(|value| serde_json::to_value(value).ok())
            .unwrap_or(Value::Null);
        let info = tokio::time::timeout(STATUS_TIMEOUT, client.info())
            .await
            .ok()
            .and_then(Result::ok)
            .and_then(|value| serde_json::to_value(value).ok())
            .unwrap_or(Value::Null);
        Ok(EngineStatusRaw { version, info })
    }

    async fn list_containers_raw(&self) -> Result<Vec<Value>, DockerError> {
        call(
            client()?.list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            })),
        )
        .await?
        .into_iter()
        .map(to_value)
        .collect()
    }

    async fn inspect_container_raw(&self, id: &str) -> Result<Value, DockerError> {
        to_value(call(client()?.inspect_container(id, None)).await?)
    }

    async fn container_action(&self, id: &str, action: ContainerAction) -> Result<(), DockerError> {
        let client = client()?;
        match action {
            ContainerAction::Start => call(client.start_container::<String>(id, None)).await?,
            ContainerAction::Stop => call(client.stop_container(id, None)).await?,
            ContainerAction::Restart => call(client.restart_container(id, None)).await?,
            ContainerAction::Remove { force } => {
                call(client.remove_container(
                    id,
                    Some(RemoveContainerOptions {
                        force,
                        ..Default::default()
                    }),
                ))
                .await?;
            }
        }
        Ok(())
    }

    async fn container_logs_raw(
        &self,
        id: &str,
        filters: &LogFilters,
        max_lines: usize,
    ) -> Result<Vec<LogChunk>, DockerError> {
        let mut stream = client()?.logs(
            id,
            Some(LogsOptions {
                follow: false,
                stdout: true,
                stderr: true,
                since: filters.since,
                until: filters.until,
                timestamps: filters.timestamps,
                tail: filters.tail.clone(),
            }),
        );
        let mut chunks = Vec::new();
        let mut lines = 0_usize;
        while lines < max_lines {
            let next = tokio::time::timeout(STATUS_TIMEOUT, stream.next())
                .await
                .map_err(|_| DockerError::Unavailable)?;
            let Some(output) = next else { break };
            let output = output.map_err(|_| DockerError::Engine)?;
            let (stream_name, bytes) = match output {
                bollard::container::LogOutput::StdOut { message } => ("stdout", message),
                bollard::container::LogOutput::StdErr { message } => ("stderr", message),
                _ => continue,
            };
            let text = String::from_utf8_lossy(&bytes).into_owned();
            lines += text.lines().filter(|line| !line.trim().is_empty()).count();
            chunks.push(LogChunk {
                stream: stream_name.to_string(),
                text,
            });
        }
        Ok(chunks)
    }

    async fn list_volumes_raw(&self) -> Result<Value, DockerError> {
        to_value(call(client()?.list_volumes(Some(ListVolumesOptions::<String>::default()))).await?)
    }

    async fn inspect_volume_raw(&self, name: &str) -> Result<Value, DockerError> {
        to_value(call(client()?.inspect_volume(name)).await?)
    }

    async fn remove_volume(&self, name: &str, force: bool) -> Result<(), DockerError> {
        call(client()?.remove_volume(name, Some(RemoveVolumeOptions { force }))).await
    }

    async fn list_networks_raw(&self) -> Result<Vec<Value>, DockerError> {
        call(client()?.list_networks(Some(ListNetworksOptions::<String>::default())))
            .await?
            .into_iter()
            .map(to_value)
            .collect()
    }

    async fn inspect_network_raw(&self, id: &str) -> Result<Value, DockerError> {
        to_value(call(client()?.inspect_network::<String>(id, None)).await?)
    }

    async fn create_network(&self, name: &str, driver: &str) -> Result<(), DockerError> {
        call(client()?.create_network(CreateNetworkOptions {
            name: name.to_string(),
            driver: driver.to_string(),
            check_duplicate: true,
            ..Default::default()
        }))
        .await
        .map(|_| ())
    }

    async fn remove_network(&self, id: &str) -> Result<(), DockerError> {
        call(client()?.remove_network(id)).await
    }

    async fn connect_network(&self, network: &str, container: &str) -> Result<(), DockerError> {
        call(client()?.connect_network(
            network,
            ConnectNetworkOptions {
                container: container.to_string(),
                ..Default::default()
            },
        ))
        .await
    }

    async fn disconnect_network(
        &self,
        network: &str,
        container: &str,
        force: bool,
    ) -> Result<(), DockerError> {
        call(client()?.disconnect_network(
            network,
            DisconnectNetworkOptions {
                container: container.to_string(),
                force,
            },
        ))
        .await
    }

    async fn list_images_raw(&self) -> Result<Vec<Value>, DockerError> {
        call(client()?.list_images(Some(ListImagesOptions::<String>::default())))
            .await?
            .into_iter()
            .map(to_value)
            .collect()
    }

    async fn inspect_image_raw(&self, id: &str) -> Result<Value, DockerError> {
        to_value(call(client()?.inspect_image(id)).await?)
    }

    async fn remove_image(&self, id: &str, force: bool) -> Result<(), DockerError> {
        call(client()?.remove_image(
            id,
            Some(RemoveImageOptions {
                force,
                ..Default::default()
            }),
            None,
        ))
        .await
        .map(|_| ())
    }

    async fn prune_images_raw(&self) -> Result<Value, DockerError> {
        to_value(call(client()?.prune_images::<String>(None)).await?)
    }

    async fn metrics(&self) -> DockerMetricsResponse {
        super::metrics::collect().await
    }

    async fn follow_logs(
        &self,
        id: &str,
        tail: &str,
        chunks: UnboundedSender<LogChunk>,
        cancel: CancellationToken,
    ) -> Result<(), DockerError> {
        let mut stream = client()?.logs(
            id,
            Some(LogsOptions {
                follow: true,
                stdout: true,
                stderr: true,
                timestamps: true,
                tail: tail.to_string(),
                ..Default::default()
            }),
        );
        loop {
            let next = tokio::select! {
                () = cancel.cancelled() => return Ok(()),
                next = stream.next() => next,
            };
            let Some(output) = next else { return Ok(()) };
            let (stream_name, bytes) = match output.map_err(|_| DockerError::Engine)? {
                bollard::container::LogOutput::StdOut { message } => ("stdout", message),
                bollard::container::LogOutput::StdErr { message } => ("stderr", message),
                _ => continue,
            };
            let chunk = LogChunk {
                stream: stream_name.to_string(),
                text: String::from_utf8_lossy(&bytes).into_owned(),
            };
            if chunks.send(chunk).is_err() {
                return Ok(());
            }
        }
    }
}

fn to_value<T: Serialize>(value: T) -> Result<Value, DockerError> {
    serde_json::to_value(value).map_err(|_| DockerError::Engine)
}
