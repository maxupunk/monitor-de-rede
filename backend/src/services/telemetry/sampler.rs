//! Laço de amostragem: host + containers a cada 10 s, minuto fechado para um
//! destino.
//!
//! Roda no agente — o remoto e o local, ao lado da central (ADR 014). O
//! destino é uma abstração (inversão de dependência): o agente manda o minuto
//! pelo canal e, sem conexão, para o buffer offline; a central grava com a
//! chave do host ([`crate::services::agents::inbound`]).

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;

use crate::services::docker::source::DockerEngine;

use super::{
    host_metrics::HostMetricsReader,
    rollup::{ContainerSample, MetricsRollup, RollupAccumulator},
};

pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(10);

#[async_trait]
pub trait RollupSink: Send + Sync {
    async fn accept(&self, rollup: MetricsRollup);
}

pub struct Sampler {
    reader: HostMetricsReader,
    engine: Arc<dyn DockerEngine>,
    accumulator: RollupAccumulator,
}

impl Sampler {
    #[must_use]
    pub fn new(reader: HostMetricsReader, engine: Arc<dyn DockerEngine>) -> Self {
        Self {
            reader,
            engine,
            accumulator: RollupAccumulator::default(),
        }
    }

    /// Uma amostra; devolve o minuto anterior quando esta abre um novo.
    pub async fn tick(&mut self) -> Option<MetricsRollup> {
        let host = self.reader.sample();
        let metrics = self.engine.metrics().await;
        let containers = if metrics.docker_available {
            ContainerSample::from_metrics(&metrics)
        } else {
            Vec::new()
        };
        if host.is_none() && containers.is_empty() {
            return None;
        }
        self.accumulator.push(Utc::now(), host, containers)
    }
}

/// Roda o amostrador para sempre.
pub fn spawn(mut sampler: Sampler, sink: Arc<dyn RollupSink>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SAMPLE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Some(rollup) = sampler.tick().await {
                sink.accept(rollup).await;
            }
        }
    })
}

/// `TELEMETRY_ENABLED=false` no ambiente do agente desliga o histórico do
/// host dele.
#[must_use]
pub fn enabled() -> bool {
    std::env::var("TELEMETRY_ENABLED").map_or(true, |value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "0" | "no" | "off"
        )
    })
}
