//! O Docker do host do agente (ADR 011, ADR 014), do lado do agente.
//!
//! O agente roda no servidor remoto (e ao lado da central, como agente
//! local). O runtime dele é do núcleo — canal, sessão, caixa de saída,
//! pontes, amostrador —; o que fala com a Docker Engine daquele host é da área
//! Servidores e chega por aqui, montado pelo binário do agente: o `Hello`
//! anuncia o Docker e o compose, o amostrador lê as métricas dos containers,
//! a sessão publica o estado ao vivo e a ponte exporta um volume. Os comandos
//! Docker (`Command::Docker`) vão ao `AgentCommandHandler` da mesma área.

use agent_protocol::{
    docker::views::{DockerInventorySnapshot, DockerLiveSnapshot, DockerMetricsResponse},
    tunnel::TunnelStream,
    Capability, ErrorCode, RemoteError,
};
use async_trait::async_trait;

/// O Docker deste host, contribuído por Servidores.
#[async_trait]
pub trait HostDocker: Send + Sync {
    /// Docker e compose, como o `Hello` os anuncia.
    async fn capabilities(&self) -> (Capability, Capability);

    /// As métricas dos containers, para o histórico de telemetria.
    async fn metrics(&self) -> DockerMetricsResponse;

    /// O estado ao vivo, quando a central pede (`SetLive`).
    async fn live(&self) -> Option<DockerLiveSnapshot>;

    /// O inventário (volumes, redes, imagens), junto do estado ao vivo.
    async fn inventory(&self) -> Option<DockerInventorySnapshot>;

    /// O tar do volume `name` (`Command::VolumeExport`). A política local já
    /// foi conferida por quem chama.
    ///
    /// # Errors
    ///
    /// Volume inexistente ou Engine inacessível.
    async fn export_volume(&self, name: &str) -> Result<TunnelStream, RemoteError>;
}

/// Agente montado sem a área Servidores: não anuncia Docker nem compose, não
/// publica estado e recusa a exportação de volume.
pub struct NoHostDocker;

fn unavailable() -> Capability {
    Capability {
        available: false,
        version: None,
        reason: Some("Docker não está disponível neste agente".into()),
    }
}

#[async_trait]
impl HostDocker for NoHostDocker {
    async fn capabilities(&self) -> (Capability, Capability) {
        (unavailable(), unavailable())
    }

    async fn metrics(&self) -> DockerMetricsResponse {
        DockerMetricsResponse {
            docker_available: false,
            unavailable_reason: unavailable().reason,
            failed_container_count: 0,
            collected_at: chrono::Utc::now().to_rfc3339(),
            containers: Vec::new(),
        }
    }

    async fn live(&self) -> Option<DockerLiveSnapshot> {
        None
    }

    async fn inventory(&self) -> Option<DockerInventorySnapshot> {
        None
    }

    async fn export_volume(&self, _name: &str) -> Result<TunnelStream, RemoteError> {
        Err(RemoteError::new(
            ErrorCode::Unsupported,
            "Docker não está disponível neste agente",
        ))
    }
}
