//! Laço de amostragem: host + containers a cada 10 s, minuto fechado para um
//! destino.
//!
//! Roda no agente — o remoto e o local, ao lado da central (ADR 014). O
//! destino é uma abstração (inversão de dependência): o agente manda o minuto
//! pelo canal e, sem conexão, para o buffer offline; a central grava com a
//! chave do host ([`crate::services::agents::inbound`]).

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use tokio_util::sync::CancellationToken;

use netmonitor_core::agents::host::HostDocker;

use crate::services::telemetry::{
    host_metrics::HostMetricsReader,
    rollup::{ContainerSample, MetricsRollup, RollupAccumulator},
};

pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(10);

#[async_trait]
pub trait RollupSink: Send + Sync {
    async fn accept(&self, rollup: MetricsRollup);
}

pub struct Sampler {
    reader: Arc<Mutex<HostMetricsReader>>,
    docker: Arc<dyn HostDocker>,
    accumulator: RollupAccumulator,
}

impl Sampler {
    #[must_use]
    pub fn new(reader: HostMetricsReader, docker: Arc<dyn HostDocker>) -> Self {
        Self {
            reader: Arc::new(Mutex::new(reader)),
            docker,
            accumulator: RollupAccumulator::default(),
        }
    }

    /// Uma amostra; devolve o minuto anterior quando esta abre um novo.
    pub async fn tick(&mut self) -> Option<MetricsRollup> {
        // `/proc` e `statvfs` fazem I/O síncrono (um ponto de montagem NFS/FUSE
        // travado não pode segurar a thread do Tokio).
        let reader = self.reader.clone();
        let host = tokio::task::spawn_blocking(move || {
            reader
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .sample()
        })
        .await
        .ok()
        .flatten();
        let metrics = self.docker.metrics().await;
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
pub fn spawn(sampler: Sampler, sink: Arc<dyn RollupSink>) -> tokio::task::JoinHandle<()> {
    spawn_with_shutdown(sampler, sink, CancellationToken::new())
}

/// Roda o amostrador até `shutdown` ser cancelado.
pub fn spawn_with_shutdown(
    mut sampler: Sampler,
    sink: Arc<dyn RollupSink>,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SAMPLE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                () = shutdown.cancelled() => break,
                _ = ticker.tick() => {
                    if let Some(rollup) = sampler.tick().await {
                        sink.accept(rollup).await;
                    }
                }
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
