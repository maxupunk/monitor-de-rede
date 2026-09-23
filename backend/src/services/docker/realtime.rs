//! Coleta única de telemetria Docker e distribuição aos clientes SSE.
//!
//! A montagem dos snapshots ([`live_snapshot`], [`inventory_snapshot`])
//! recebe a fonte: o coletor da central a usa para o host local e o agente
//! remoto, para o dele — o mesmo mapeamento dos dois lados.

use std::time::Duration;

use loco_rs::app::AppContext;

use crate::{
    services::events::EventBus,
    views::docker::{DockerInventorySnapshot, DockerLiveSnapshot, DockerMetricsResponse},
};

use super::{
    engine,
    hosts::{HostKey, LOCAL_HOST_KEY},
    metrics,
    source::{DockerEngine, LocalEngine},
    DockerError,
};

const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(3);
const INVENTORY_EVERY_CYCLES: u8 = 5;

pub const LIVE_EVENT: &str = "docker:snapshot";
pub const INVENTORY_EVENT: &str = "docker:inventory";

/// Inicia o produtor somente no processo HTTP. Sem assinantes SSE, o ciclo não
/// consulta a Docker Engine.
pub fn spawn(ctx: AppContext) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SNAPSHOT_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut inventory_cycle = 0;

        loop {
            ticker.tick().await;
            let Ok(bus) = EventBus::from_context(&ctx) else {
                continue;
            };
            if !bus.has_subscribers() {
                inventory_cycle = 0;
                continue;
            }

            let available = match publish_live(&ctx, &bus).await {
                Ok(available) => available,
                Err(error) => {
                    tracing::debug!(%error, "falha ao coletar snapshot Docker para SSE");
                    false
                }
            };
            if available && inventory_cycle == 0 {
                if let Err(error) = publish_inventory(&bus).await {
                    tracing::debug!(%error, "falha ao coletar inventário Docker para SSE");
                }
            }
            inventory_cycle = (inventory_cycle + 1) % INVENTORY_EVERY_CYCLES;
        }
    });
}

/// Atualiza a tela depois de uma mutação. No host local a central coleta na
/// hora; um agente publica sozinho o snapshot seguinte à mutação.
pub async fn refresh(ctx: &AppContext, host: HostKey) {
    if host.is_local() {
        metrics::invalidate(ctx).await;
        publish_all(ctx).await;
    }
}

/// Atualiza imediatamente estado e inventário após uma mutação administrativa.
pub async fn publish_all(ctx: &AppContext) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    if !bus.has_subscribers() {
        return;
    }
    let available = match publish_live(ctx, &bus).await {
        Ok(available) => available,
        Err(error) => {
            tracing::debug!(%error, "falha ao publicar snapshot Docker após ação");
            false
        }
    };
    if available {
        if let Err(error) = publish_inventory(&bus).await {
            tracing::debug!(%error, "falha ao publicar inventário Docker após ação");
        }
    }
}

/// Estado, containers e métricas de um host. As métricas vêm de fora porque
/// a central as tira do cache do processo e o agente, da própria Engine.
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
        host_key: LOCAL_HOST_KEY.to_string(),
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
        host_key: LOCAL_HOST_KEY.to_string(),
        collected_at: chrono::Utc::now().to_rfc3339(),
        volumes,
        networks,
        images,
    })
}

async fn publish_live(ctx: &AppContext, bus: &EventBus) -> Result<bool, DockerError> {
    let snapshot = live_snapshot(&LocalEngine, metrics::overview(ctx)).await?;
    let available = snapshot.status.available;
    bus.publish_snapshot(LIVE_EVENT, LOCAL_HOST_KEY, serde_json::json!(snapshot));
    Ok(available)
}

async fn publish_inventory(bus: &EventBus) -> Result<(), DockerError> {
    let snapshot = inventory_snapshot(&LocalEngine).await?;
    bus.publish_snapshot(INVENTORY_EVENT, LOCAL_HOST_KEY, serde_json::json!(snapshot));
    Ok(())
}
