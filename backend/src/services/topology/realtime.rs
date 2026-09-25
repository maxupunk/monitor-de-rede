//! Coletor compartilhado de tráfego de topologia e interfaces para clientes SSE.
//!
//! Roda exclusivamente no processo HTTP da central. Condicionado à existência de
//! assinantes SSE conectados (`EventBus::has_subscribers`), evitando consultas
//! desnecessárias de rede quando ninguém está visualizando a interface (AGENTS §9).

use std::time::Duration;

use futures::{stream, StreamExt};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{
    models::devices,
    services::{events::EventBus, monitoring::execution_guard::try_acquire_snmp_device},
};

const MAX_CONCURRENT_POLLS: usize = 5;

fn realtime_enabled() -> bool {
    std::env::var("TOPOLOGY_REALTIME_ENABLED").map_or(true, |value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "0" | "no" | "off"
        )
    })
}

fn resolve_interval() -> Duration {
    let secs = std::env::var("TOPOLOGY_REALTIME_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
        .clamp(2, 60);
    Duration::from_secs(secs)
}

pub fn spawn(ctx: AppContext) {
    if !realtime_enabled() {
        tracing::info!("coletor em tempo real de topologia desativado");
        return;
    }

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(resolve_interval());
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            let Ok(bus) = EventBus::from_context(&ctx) else {
                continue;
            };
            if !bus.has_subscribers() {
                continue;
            }

            let Ok(all_devices) = devices::Entity::find()
                .filter(devices::Column::SnmpEnabled.eq(true))
                .all(&ctx.db)
                .await
            else {
                continue;
            };

            let snmp_devices: Vec<_> = all_devices
                .into_iter()
                .filter(|d| {
                    d.ip_address
                        .as_ref()
                        .is_some_and(|ip| !ip.trim().is_empty())
                })
                .collect();

            if snmp_devices.is_empty() {
                continue;
            }

            stream::iter(snmp_devices)
                .for_each_concurrent(MAX_CONCURRENT_POLLS, |device| {
                    let ctx = ctx.clone();
                    async move {
                        let Some(_guard) = try_acquire_snmp_device(device.id) else {
                            return;
                        };
                        let Ok(config) = crate::services::snmp::service::device_config(&device)
                        else {
                            return;
                        };
                        let _ = tokio::time::timeout(
                            Duration::from_millis(3500),
                            crate::services::snmp::service::poll_device(&ctx, &device, config),
                        )
                        .await;
                    }
                })
                .await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_realtime_enabled_default() {
        std::env::remove_var("TOPOLOGY_REALTIME_ENABLED");
        assert!(realtime_enabled());
    }

    #[test]
    #[serial]
    fn test_realtime_enabled_custom() {
        std::env::set_var("TOPOLOGY_REALTIME_ENABLED", "false");
        assert!(!realtime_enabled());
        std::env::set_var("TOPOLOGY_REALTIME_ENABLED", "0");
        assert!(!realtime_enabled());
        std::env::set_var("TOPOLOGY_REALTIME_ENABLED", "true");
        assert!(realtime_enabled());
        std::env::remove_var("TOPOLOGY_REALTIME_ENABLED");
    }

    #[test]
    #[serial]
    fn test_resolve_interval() {
        std::env::remove_var("TOPOLOGY_REALTIME_INTERVAL_SECS");
        assert_eq!(resolve_interval(), Duration::from_secs(5));

        std::env::set_var("TOPOLOGY_REALTIME_INTERVAL_SECS", "10");
        assert_eq!(resolve_interval(), Duration::from_secs(10));

        std::env::set_var("TOPOLOGY_REALTIME_INTERVAL_SECS", "1"); // clamped to 2
        assert_eq!(resolve_interval(), Duration::from_secs(2));

        std::env::set_var("TOPOLOGY_REALTIME_INTERVAL_SECS", "99"); // clamped to 60
        assert_eq!(resolve_interval(), Duration::from_secs(60));

        std::env::remove_var("TOPOLOGY_REALTIME_INTERVAL_SECS");
    }
}
