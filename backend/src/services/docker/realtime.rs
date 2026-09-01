//! Coleta única de telemetria Docker e distribuição aos clientes SSE.

use std::time::Duration;

use loco_rs::app::AppContext;

use crate::{
    services::events::EventBus,
    views::docker::{DockerInventorySnapshot, DockerLiveSnapshot, DockerMetricsResponse},
};

use super::{engine, metrics, DockerError};

const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(3);
const INVENTORY_EVERY_CYCLES: u8 = 5;

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

async fn publish_live(ctx: &AppContext, bus: &EventBus) -> Result<bool, DockerError> {
    let status = engine::status().await;
    let available = status.available;
    let (containers, metrics) = if status.available {
        let (containers, metrics) = tokio::join!(engine::list_containers(), metrics::overview(ctx));
        (containers?, metrics)
    } else {
        (
            Vec::new(),
            DockerMetricsResponse {
                docker_available: false,
                unavailable_reason: status.reason.clone(),
                collected_at: chrono::Utc::now().to_rfc3339(),
                containers: Vec::new(),
            },
        )
    };
    bus.publish_ephemeral(
        "docker:snapshot",
        serde_json::json!(DockerLiveSnapshot {
            status,
            containers,
            metrics,
        }),
    );
    Ok(available)
}

async fn publish_inventory(bus: &EventBus) -> Result<(), DockerError> {
    let (volumes, networks, images) = tokio::try_join!(
        engine::list_volumes(),
        engine::list_networks(),
        engine::list_images()
    )?;
    bus.publish_ephemeral(
        "docker:inventory",
        serde_json::json!(DockerInventorySnapshot {
            collected_at: chrono::Utc::now().to_rfc3339(),
            volumes,
            networks,
            images,
        }),
    );
    Ok(())
}
