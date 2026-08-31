//! Métricas de containers com coleta concorrente e cache curto.

use std::{sync::Arc, time::Duration};

use bollard::{
    container::{ListContainersOptions, StatsOptions},
    models::ContainerSummary,
    Docker,
};
use futures::{stream, StreamExt};
use loco_rs::app::AppContext;
use tokio::sync::Mutex;

use crate::views::docker::{
    DockerContainerMetrics, DockerCpuMetrics, DockerIoMetrics, DockerMemoryMetrics,
    DockerMetricsResponse, DockerNetworkMetrics,
};

use super::{client, DockerError, DISABLED_REASON, UNAVAILABLE_REASON};

const CACHE_TTL: Duration = Duration::from_secs(10);
const STATS_TIMEOUT: Duration = Duration::from_secs(4);
const MAX_CONCURRENT_STATS: usize = 8;

#[derive(Clone, Default)]
pub struct MetricsState {
    value: Arc<Mutex<Option<(tokio::time::Instant, DockerMetricsResponse)>>>,
}

pub fn install(ctx: &AppContext) {
    if !ctx.shared_store.contains::<MetricsState>() {
        ctx.shared_store.insert(MetricsState::default());
    }
}

pub async fn overview(ctx: &AppContext) -> DockerMetricsResponse {
    let state = ctx.shared_store.get::<MetricsState>();
    if let Some(cached) = cached(state.as_ref()).await {
        return cached;
    }

    let result = collect().await;
    if let Some(state) = state {
        *state.value.lock().await = Some((tokio::time::Instant::now(), result.clone()));
    }
    result
}

async fn cached(state: Option<&MetricsState>) -> Option<DockerMetricsResponse> {
    let guard = state?.value.lock().await;
    let (measured_at, value) = guard.as_ref()?;
    (measured_at.elapsed() < CACHE_TTL).then(|| value.clone())
}

async fn collect() -> DockerMetricsResponse {
    let client = match client() {
        Ok(client) => client,
        Err(DockerError::Disabled) => return unavailable(DISABLED_REASON),
        Err(_) => return unavailable(UNAVAILABLE_REASON),
    };
    let containers = match tokio::time::timeout(
        STATS_TIMEOUT,
        client.list_containers(Some(ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        })),
    )
    .await
    {
        Ok(Ok(items)) => items,
        _ => return unavailable("Não foi possível coletar métricas da Docker Engine"),
    };

    let measured = stream::iter(containers.into_iter().filter_map(|summary| {
        let id = summary.id.clone()?;
        let client = client.clone();
        Some(async move { container_metrics(&client, &id, &summary).await })
    }))
    .buffer_unordered(MAX_CONCURRENT_STATS)
    .collect::<Vec<Option<DockerContainerMetrics>>>()
    .await;
    let mut metrics = measured.into_iter().flatten().collect::<Vec<_>>();
    metrics.sort_by(|a, b| a.container_name.cmp(&b.container_name));

    DockerMetricsResponse {
        docker_available: true,
        unavailable_reason: None,
        collected_at: chrono::Utc::now().to_rfc3339(),
        containers: metrics,
    }
}

fn unavailable(reason: &str) -> DockerMetricsResponse {
    DockerMetricsResponse {
        docker_available: false,
        unavailable_reason: Some(reason.to_string()),
        collected_at: chrono::Utc::now().to_rfc3339(),
        containers: Vec::new(),
    }
}

async fn container_metrics(
    client: &Docker,
    id: &str,
    summary: &ContainerSummary,
) -> Option<DockerContainerMetrics> {
    let mut stream = client.stats(
        id,
        Some(StatsOptions {
            stream: false,
            one_shot: true,
        }),
    );
    let stats = tokio::time::timeout(STATS_TIMEOUT, stream.next())
        .await
        .ok()??
        .ok()?;

    let name = summary
        .names
        .as_ref()
        .and_then(|names| names.first())
        .map(|name| name.trim_start_matches('/').to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| id.chars().take(12).collect());
    let project_name = summary.labels.as_ref().and_then(|labels| {
        ["com.docker.compose.project", "io.podman.compose.project"]
            .iter()
            .find_map(|key| labels.get(*key))
            .cloned()
    });
    let usage = stats.memory_stats.usage.unwrap_or_default();
    let limit = stats.memory_stats.limit.unwrap_or_default();
    let mut received = 0;
    let mut transmitted = 0;
    if let Some(networks) = &stats.networks {
        for network in networks.values() {
            received += network.rx_bytes;
            transmitted += network.tx_bytes;
        }
    }
    let mut read = 0;
    let mut write = 0;
    if let Some(items) = &stats.blkio_stats.io_service_bytes_recursive {
        for item in items {
            match item.op.to_ascii_lowercase().as_str() {
                "read" => read += item.value,
                "write" => write += item.value,
                _ => {}
            }
        }
    }

    Some(DockerContainerMetrics {
        container_id: id.to_string(),
        container_name: name,
        project_name,
        image_name: summary.image.clone().unwrap_or_default(),
        status: summary
            .state
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        cpu: DockerCpuMetrics {
            usage_percent: cpu_usage(&stats),
        },
        memory: DockerMemoryMetrics {
            usage_bytes: usage,
            limit_bytes: limit,
            usage_percent: percentage(usage, limit),
        },
        network: DockerNetworkMetrics {
            received_bytes: received,
            transmitted_bytes: transmitted,
        },
        block_io: DockerIoMetrics {
            read_bytes: read,
            write_bytes: write,
        },
        pids: stats.pids_stats.current,
    })
}

fn cpu_usage(stats: &bollard::container::Stats) -> f64 {
    let cpu_delta = stats
        .cpu_stats
        .cpu_usage
        .total_usage
        .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
    let system_delta = stats
        .cpu_stats
        .system_cpu_usage
        .unwrap_or_default()
        .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or_default());
    if cpu_delta == 0 || system_delta == 0 {
        return 0.0;
    }
    let cpus = stats
        .cpu_stats
        .online_cpus
        .or_else(|| {
            stats
                .cpu_stats
                .cpu_usage
                .percpu_usage
                .as_ref()
                .map(|values| values.len() as u64)
        })
        .unwrap_or(1)
        .max(1);
    round((cpu_delta as f64 / system_delta as f64) * cpus as f64 * 100.0)
}

fn percentage(part: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    round(part as f64 * 100.0 / total as f64)
}

fn round(value: f64) -> f64 {
    (value.max(0.0) * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentual_trata_total_zero() {
        assert_eq!(percentage(10, 0), 0.0);
        assert_eq!(percentage(25, 100), 25.0);
    }
}
