//! Persistência e leitura do histórico de 1 minuto.
//!
//! Gravar é upsert por `(host, [container,] minuto)`: o mesmo minuto pode
//! chegar duas vezes quando o agente reenvia o buffer offline, e a segunda
//! cópia só substitui a primeira. Ler reduz a resolução conforme a janela,
//! para que 30 dias não virem 43 mil pontos na tela.

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::{Alias, Expr, Func, OnConflict},
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::{
    models::{container_metrics_1m, host_metrics_1m},
    services::shared::errors::{AppError, AppResult},
    views::telemetry::{
        ContainerHistoryPoint, ContainerUsageSummary, HostHistoryPoint, MetricsRange,
    },
};

use super::rollup::{DiskUsage, MetricsRollup};

/// Teto de linhas por série numa leitura (30 dias de 1 minuto cabem com folga).
const MAX_ROWS: u64 = 50_000;

type UsageRow = (
    String,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

fn big_sum(column: container_metrics_1m::Column) -> sea_orm::sea_query::FunctionCall {
    Func::cast_as(Func::sum(Expr::col(column)), Alias::new("BIGINT"))
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// # Errors
///
/// Propaga erro do banco.
pub async fn save<C: ConnectionTrait>(
    db: &C,
    host_key: &str,
    rollup: &MetricsRollup,
) -> AppResult<()> {
    let bucket_at = rollup.bucket_at.into();
    if let Some(host) = &rollup.host {
        let disks = serde_json::to_value(&host.disks)
            .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
        host_metrics_1m::Entity::insert(host_metrics_1m::ActiveModel {
            host_key: Set(host_key.to_string()),
            bucket_at: Set(bucket_at),
            samples: Set(i32::try_from(rollup.samples).unwrap_or(i32::MAX)),
            cpu_avg: Set(host.cpu_avg),
            cpu_max: Set(host.cpu_max),
            memory_used_avg: Set(host.memory_used_avg),
            memory_total: Set(to_i64(host.memory_total)),
            load1: Set(host.load1),
            net_rx_bps: Set(host.net_rx_bps),
            net_tx_bps: Set(host.net_tx_bps),
            disks: Set(disks),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                host_metrics_1m::Column::HostKey,
                host_metrics_1m::Column::BucketAt,
            ])
            .update_columns([
                host_metrics_1m::Column::Samples,
                host_metrics_1m::Column::CpuAvg,
                host_metrics_1m::Column::CpuMax,
                host_metrics_1m::Column::MemoryUsedAvg,
                host_metrics_1m::Column::MemoryTotal,
                host_metrics_1m::Column::Load1,
                host_metrics_1m::Column::NetRxBps,
                host_metrics_1m::Column::NetTxBps,
                host_metrics_1m::Column::Disks,
            ])
            .to_owned(),
        )
        .exec(db)
        .await?;
    }

    if rollup.containers.is_empty() {
        return Ok(());
    }
    let rows = rollup
        .containers
        .iter()
        .map(|container| container_metrics_1m::ActiveModel {
            host_key: Set(host_key.to_string()),
            container_name: Set(container.name.chars().take(255).collect()),
            project: Set(container
                .project
                .as_ref()
                .map(|project| project.chars().take(255).collect())),
            bucket_at: Set(bucket_at),
            cpu_avg: Set(container.cpu_avg),
            cpu_max: Set(container.cpu_max),
            memory_avg: Set(container.memory_avg),
            memory_max: Set(to_i64(container.memory_max)),
            net_rx_bytes: Set(to_i64(container.net_rx_bytes)),
            net_tx_bytes: Set(to_i64(container.net_tx_bytes)),
            block_read_bytes: Set(to_i64(container.block_read_bytes)),
            block_write_bytes: Set(to_i64(container.block_write_bytes)),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        });
    container_metrics_1m::Entity::insert_many(rows)
        .on_conflict(
            OnConflict::columns([
                container_metrics_1m::Column::HostKey,
                container_metrics_1m::Column::ContainerName,
                container_metrics_1m::Column::BucketAt,
            ])
            .update_columns([
                container_metrics_1m::Column::Project,
                container_metrics_1m::Column::CpuAvg,
                container_metrics_1m::Column::CpuMax,
                container_metrics_1m::Column::MemoryAvg,
                container_metrics_1m::Column::MemoryMax,
                container_metrics_1m::Column::NetRxBytes,
                container_metrics_1m::Column::NetTxBytes,
                container_metrics_1m::Column::BlockReadBytes,
                container_metrics_1m::Column::BlockWriteBytes,
            ])
            .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

/// Série do host na janela, já reduzida ao passo da janela.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn host_history<C: ConnectionTrait>(
    db: &C,
    host_key: &str,
    range: MetricsRange,
    now: DateTime<Utc>,
) -> AppResult<Vec<HostHistoryPoint>> {
    let rows = host_metrics_1m::Entity::find()
        .filter(host_metrics_1m::Column::HostKey.eq(host_key))
        .filter(host_metrics_1m::Column::BucketAt.gte(now - range.window()))
        .order_by_asc(host_metrics_1m::Column::BucketAt)
        .limit(MAX_ROWS)
        .all(db)
        .await?;
    let points = rows
        .into_iter()
        .map(|row| HostHistoryPoint {
            at: row.bucket_at.with_timezone(&Utc).to_rfc3339(),
            cpu_avg: row.cpu_avg,
            cpu_max: row.cpu_max,
            memory_used: row.memory_used_avg,
            memory_total: row.memory_total,
            load1: row.load1,
            net_rx_bps: row.net_rx_bps,
            net_tx_bps: row.net_tx_bps,
            disks: serde_json::from_value::<Vec<DiskUsage>>(row.disks).unwrap_or_default(),
        })
        .collect();
    Ok(downsample_host(points, range.step_minutes()))
}

/// Uso médio/máximo de cada container na janela — a lista da tela.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn container_usage<C: ConnectionTrait>(
    db: &C,
    host_key: &str,
    range: MetricsRange,
    now: DateTime<Utc>,
) -> AppResult<Vec<ContainerUsageSummary>> {
    // AVG/MAX/SUM com GROUP BY: SQL igual nos dois dialetos.
    let rows: Vec<UsageRow> = container_metrics_1m::Entity::find()
        .select_only()
        .column(container_metrics_1m::Column::ContainerName)
        .column_as(container_metrics_1m::Column::Project.max(), "project")
        .column_as(container_metrics_1m::Column::CpuAvg.avg(), "cpu_avg")
        .column_as(container_metrics_1m::Column::CpuMax.max(), "cpu_max")
        .column_as(container_metrics_1m::Column::MemoryMax.max(), "memory_max")
        // No PostgreSQL `SUM(bigint)` é `numeric`, que o sqlx não lê como
        // i64. O CAST devolve o mesmo tipo nos dois dialetos.
        .expr_as(big_sum(container_metrics_1m::Column::NetRxBytes), "net_rx")
        .expr_as(big_sum(container_metrics_1m::Column::NetTxBytes), "net_tx")
        .filter(container_metrics_1m::Column::HostKey.eq(host_key))
        .filter(container_metrics_1m::Column::BucketAt.gte(now - range.window()))
        .group_by(container_metrics_1m::Column::ContainerName)
        .into_tuple()
        .all(db)
        .await?;
    let mut output: Vec<ContainerUsageSummary> = rows
        .into_iter()
        .map(
            |(name, project, cpu_avg, cpu_max, memory_max, net_rx, net_tx)| ContainerUsageSummary {
                name,
                project,
                cpu_avg: cpu_avg.unwrap_or_default(),
                cpu_max: cpu_max.unwrap_or_default(),
                memory_max: memory_max.unwrap_or_default(),
                net_rx_bytes: net_rx.unwrap_or_default(),
                net_tx_bytes: net_tx.unwrap_or_default(),
            },
        )
        .collect();
    output.sort_by(|a, b| b.cpu_avg.total_cmp(&a.cpu_avg).then(a.name.cmp(&b.name)));
    Ok(output)
}

/// Série de um container na janela.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn container_history<C: ConnectionTrait>(
    db: &C,
    host_key: &str,
    container_name: &str,
    range: MetricsRange,
    now: DateTime<Utc>,
) -> AppResult<Vec<ContainerHistoryPoint>> {
    let rows = container_metrics_1m::Entity::find()
        .filter(container_metrics_1m::Column::HostKey.eq(host_key))
        .filter(container_metrics_1m::Column::ContainerName.eq(container_name))
        .filter(container_metrics_1m::Column::BucketAt.gte(now - range.window()))
        .order_by_asc(container_metrics_1m::Column::BucketAt)
        .limit(MAX_ROWS)
        .all(db)
        .await?;
    let points = rows
        .into_iter()
        .map(|row| ContainerHistoryPoint {
            at: row.bucket_at.with_timezone(&Utc).to_rfc3339(),
            cpu_avg: row.cpu_avg,
            cpu_max: row.cpu_max,
            memory_avg: row.memory_avg,
            memory_max: row.memory_max,
            net_rx_bytes: row.net_rx_bytes,
            net_tx_bytes: row.net_tx_bytes,
            block_read_bytes: row.block_read_bytes,
            block_write_bytes: row.block_write_bytes,
        })
        .collect();
    Ok(downsample_containers(points, range.step_minutes()))
}

/// Apaga o histórico de um host (agente removido).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn forget_host<C: ConnectionTrait>(db: &C, host_key: &str) -> AppResult<u64> {
    let hosts = host_metrics_1m::Entity::delete_many()
        .filter(host_metrics_1m::Column::HostKey.eq(host_key))
        .exec(db)
        .await?
        .rows_affected;
    let containers = container_metrics_1m::Entity::delete_many()
        .filter(container_metrics_1m::Column::HostKey.eq(host_key))
        .exec(db)
        .await?
        .rows_affected;
    Ok(hosts + containers)
}

/// Retenção: apaga baldes anteriores a `cutoff`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn prune<C: ConnectionTrait>(db: &C, cutoff: DateTime<Utc>) -> AppResult<u64> {
    let hosts = host_metrics_1m::Entity::delete_many()
        .filter(host_metrics_1m::Column::BucketAt.lt(cutoff))
        .exec(db)
        .await?
        .rows_affected;
    let containers = container_metrics_1m::Entity::delete_many()
        .filter(container_metrics_1m::Column::BucketAt.lt(cutoff))
        .exec(db)
        .await?
        .rows_affected;
    Ok(hosts + containers)
}

/// Agrupa pontos consecutivos em janelas de `step` minutos: média das
/// médias, máximo dos máximos.
fn chunks<T>(points: Vec<T>, step: usize) -> Vec<Vec<T>> {
    if step <= 1 {
        return points.into_iter().map(|point| vec![point]).collect();
    }
    let mut output = Vec::new();
    let mut current = Vec::with_capacity(step);
    for point in points {
        current.push(point);
        if current.len() == step {
            output.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        output.push(current);
    }
    output
}

#[allow(clippy::cast_precision_loss)]
fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0_usize), |(s, c), v| (s + v, c + 1));
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

fn downsample_host(points: Vec<HostHistoryPoint>, step: usize) -> Vec<HostHistoryPoint> {
    chunks(points, step)
        .into_iter()
        .filter_map(|group| {
            let first = group.first()?.clone();
            let last = group.last()?.clone();
            Some(HostHistoryPoint {
                at: first.at,
                cpu_avg: mean(group.iter().map(|p| p.cpu_avg)),
                cpu_max: group.iter().map(|p| p.cpu_max).fold(0.0, f64::max),
                memory_used: mean(group.iter().map(|p| p.memory_used)),
                memory_total: last.memory_total,
                load1: mean(group.iter().map(|p| p.load1)),
                net_rx_bps: mean(group.iter().map(|p| p.net_rx_bps)),
                net_tx_bps: mean(group.iter().map(|p| p.net_tx_bps)),
                disks: last.disks,
            })
        })
        .collect()
}

fn downsample_containers(
    points: Vec<ContainerHistoryPoint>,
    step: usize,
) -> Vec<ContainerHistoryPoint> {
    chunks(points, step)
        .into_iter()
        .filter_map(|group| {
            let first = group.first()?.clone();
            Some(ContainerHistoryPoint {
                at: first.at,
                cpu_avg: mean(group.iter().map(|p| p.cpu_avg)),
                cpu_max: group.iter().map(|p| p.cpu_max).fold(0.0, f64::max),
                memory_avg: mean(group.iter().map(|p| p.memory_avg)),
                memory_max: group.iter().map(|p| p.memory_max).max().unwrap_or(0),
                net_rx_bytes: group.iter().map(|p| p.net_rx_bytes).sum(),
                net_tx_bytes: group.iter().map(|p| p.net_tx_bytes).sum(),
                block_read_bytes: group.iter().map(|p| p.block_read_bytes).sum(),
                block_write_bytes: group.iter().map(|p| p.block_write_bytes).sum(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(minute: usize, cpu: f64, rx: i64) -> ContainerHistoryPoint {
        ContainerHistoryPoint {
            at: format!("m{minute}"),
            cpu_avg: cpu,
            cpu_max: cpu,
            memory_avg: 10.0,
            memory_max: 10,
            net_rx_bytes: rx,
            net_tx_bytes: 0,
            block_read_bytes: 0,
            block_write_bytes: 0,
        }
    }

    #[test]
    fn reducao_soma_contadores_e_media_gauges() {
        let points = (0..5).map(|m| point(m, m as f64, 10)).collect();
        let reduced = downsample_containers(points, 2);
        assert_eq!(reduced.len(), 3);
        assert_eq!(reduced[0].at, "m0");
        assert!((reduced[0].cpu_avg - 0.5).abs() < f64::EPSILON);
        assert!((reduced[0].cpu_max - 1.0).abs() < f64::EPSILON);
        assert_eq!(reduced[0].net_rx_bytes, 20);
        assert_eq!(reduced[2].net_rx_bytes, 10, "sobra vira ponto próprio");
    }

    #[test]
    fn passo_um_preserva_os_pontos() {
        let points: Vec<_> = (0..3).map(|m| point(m, 1.0, 1)).collect();
        assert_eq!(downsample_containers(points, 1).len(), 3);
    }
}
