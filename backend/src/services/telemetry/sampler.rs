//! Laço de amostragem: host + containers a cada 10 s, minuto fechado para um
//! destino.
//!
//! O destino é a única diferença entre a central e o agente (inversão de
//! dependência): a central grava direto no banco com a chave `local`; o
//! agente manda o minuto pelo canal e, sem conexão, para o buffer offline.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::DatabaseConnection;

use crate::services::docker::source::DockerEngine;

use super::{
    host_metrics::HostMetricsReader,
    rollup::{ContainerSample, MetricsRollup, RollupAccumulator},
    store,
};

pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(10);

#[async_trait]
pub trait RollupSink: Send + Sync {
    async fn accept(&self, rollup: MetricsRollup);
}

/// Grava o minuto no histórico da própria central.
pub struct DatabaseSink {
    db: DatabaseConnection,
    host_key: String,
}

impl DatabaseSink {
    #[must_use]
    pub fn new(db: DatabaseConnection, host_key: impl Into<String>) -> Self {
        Self {
            db,
            host_key: host_key.into(),
        }
    }
}

#[async_trait]
impl RollupSink for DatabaseSink {
    async fn accept(&self, rollup: MetricsRollup) {
        if let Err(error) = store::save(&self.db, &self.host_key, &rollup).await {
            tracing::warn!(%error, host = %self.host_key, "falha ao gravar o minuto de métricas");
        }
    }
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

/// `TELEMETRY_ENABLED=false` desliga o histórico do host local.
#[must_use]
pub fn enabled() -> bool {
    std::env::var("TELEMETRY_ENABLED").map_or(true, |value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "0" | "no" | "off"
        )
    })
}
