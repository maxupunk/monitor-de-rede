//! Snapshots Docker ao vivo de um host.
//!
//! Quem coleta é o agente, no host dele — o remoto ou o local, ao lado da
//! central (ADR 014) — e só enquanto a central pede (`SetLive`, ligado quando
//! há assinante SSE). A central carimba a chave do host e distribui
//! ([`crate::services::agents::inbound`]); depois de uma mutação, o próprio
//! agente publica o snapshot seguinte. A montagem fica aqui para o mapeamento
//! ser um só ([`super::engine`]).

use crate::views::docker::{DockerInventorySnapshot, DockerLiveSnapshot, DockerMetricsResponse};

use super::{engine, metrics, source::DockerEngine, DockerError};

pub const LIVE_EVENT: &str = "docker:snapshot";
pub const INVENTORY_EVENT: &str = "docker:inventory";

/// Estado, containers e métricas de um host.
///
/// # Errors
///
/// Falha ao listar containers com a Engine disponível.
pub async fn live_snapshot(
    engine: &dyn DockerEngine,
    metrics: impl std::future::Future<Output = DockerMetricsResponse>,
) -> Result<DockerLiveSnapshot, DockerError> {
    let status = engine::status(engine).await;
    let (containers, metrics) = if status.available {
        let (containers, metrics) = tokio::join!(engine::list_containers(engine), metrics);
        (containers?, metrics)
    } else {
        (
            Vec::new(),
            metrics::unavailable(status.reason.as_deref().unwrap_or_default()),
        )
    };
    Ok(DockerLiveSnapshot {
        host_key: String::new(),
        status,
        containers,
        metrics,
    })
}

/// Volumes, redes e imagens de um host.
///
/// # Errors
///
/// Falha de qualquer uma das três listagens.
pub async fn inventory_snapshot(
    engine: &dyn DockerEngine,
) -> Result<DockerInventorySnapshot, DockerError> {
    let (volumes, networks, images) = tokio::try_join!(
        engine::list_volumes(engine),
        engine::list_networks(engine),
        engine::list_images(engine)
    )?;
    Ok(DockerInventorySnapshot {
        host_key: String::new(),
        collected_at: chrono::Utc::now().to_rfc3339(),
        volumes,
        networks,
        images,
    })
}
