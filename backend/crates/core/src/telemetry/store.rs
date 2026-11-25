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
    TransactionSession, TransactionTrait,
};

use agent_protocol::telemetry::{ContainerRollup, DiskUsage, MetricsRollup};

use super::views::{ContainerHistoryPoint, ContainerUsageSummary, HostHistoryPoint, MetricsRange};
use crate::{
    models::{container_metrics_1m, host_metrics_1m},
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

/// Teto de linhas por série numa leitura (30 dias de 1 minuto cabem com folga).
const MAX_ROWS: u64 = 50_000;
/// Cada linha de `container_metrics_1m` tem 13 parâmetros; lotes de 50 ficam
/// bem abaixo do limite de 999 variáveis do SQLite (50 * 13 = 650).
const CONTAINER_INSERT_BATCH: usize = 50;

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

/// Um container por nome (já cortado nos 255 da coluna), vencendo a última
/// ocorrência: o `ON CONFLICT` do PostgreSQL recusa afetar a mesma linha duas
/// vezes no mesmo comando, e dois nomes longos podem coincidir no corte.
fn unique_by_column_name(containers: &[ContainerRollup]) -> Vec<(String, &ContainerRollup)> {
    let mut index = std::collections::HashMap::with_capacity(containers.len());
    let mut unique: Vec<(String, &ContainerRollup)> = Vec::with_capacity(containers.len());
    for container in containers {
        let name: String = container.name.chars().take(255).collect();
        match index.get(&name) {
            Some(&position) => unique[position] = (name, container),
            None => {
                index.insert(name.clone(), unique.len());
                unique.push((name, container));
            }
        }
    }
    unique
}

/// # Errors
///
/// Propaga erro do banco.
pub async fn save<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    host_key: &str,
    rollup: &MetricsRollup,
) -> AppResult<()> {
    // Nada a gravar: não abre transação (no SQLite, `BEGIN IMMEDIATE` tomaria
    // o lock de escrita à toa).
    if rollup.host.is_none() && rollup.containers.is_empty() {
        return Ok(());
    }
    let txn = begin_write(db).await?;
    let bucket_at = rollup.bucket_at.into();
    let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
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
            created_at: Set(now),
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
        .exec(&txn)
        .await?;
    }

    let containers = unique_by_column_name(&rollup.containers);
    for batch in containers.chunks(CONTAINER_INSERT_BATCH) {
        let rows = batch
            .iter()
            .map(|(name, container)| container_metrics_1m::ActiveModel {
                host_key: Set(host_key.to_string()),
                container_name: Set(name.clone()),
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
                created_at: Set(now),
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
            .exec(&txn)
            .await?;
    }

    txn.commit().await?;
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
    Ok(downsample_host_rows(rows, range.step_minutes()))
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
    Ok(downsample_container_rows(rows, range.step_minutes()))
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

/// Passa o histórico de `from` para `to`. Idempotente: sem linhas de `from`,
/// não faz nada.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn rename_host<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    from: &str,
    to: &str,
) -> AppResult<u64> {
    let txn = begin_write(db).await?;
    let hosts = host_metrics_1m::Entity::update_many()
        .col_expr(host_metrics_1m::Column::HostKey, Expr::value(to))
        .filter(host_metrics_1m::Column::HostKey.eq(from))
        .exec(&txn)
        .await?
        .rows_affected;
    let containers = container_metrics_1m::Entity::update_many()
        .col_expr(container_metrics_1m::Column::HostKey, Expr::value(to))
        .filter(container_metrics_1m::Column::HostKey.eq(from))
        .exec(&txn)
        .await?
        .rows_affected;
    txn.commit().await?;
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

/// Agrupa linhas ordenadas por balde temporal alinhado a `step_minutes`.
///
/// Diferente de fatiar por contagem de linhas (`chunks`), lacunas offline
/// permanecem separadas em vez de fundirem horas ou dias distintos.
fn group_by_time_bucket<T, F>(rows: Vec<T>, step_minutes: usize, timestamp: F) -> Vec<Vec<T>>
where
    F: Fn(&T) -> i64,
{
    if step_minutes <= 1 {
        return rows.into_iter().map(|row| vec![row]).collect();
    }
    let step_secs = i64::try_from(step_minutes)
        .unwrap_or(1)
        .saturating_mul(60)
        .max(60);
    let mut output: Vec<Vec<T>> = Vec::new();
    let mut current_bucket: Option<i64> = None;
    for row in rows {
        let bucket = timestamp(&row).div_euclid(step_secs);
        if current_bucket == Some(bucket) {
            if let Some(group) = output.last_mut() {
                group.push(row);
            }
        } else {
            current_bucket = Some(bucket);
            output.push(vec![row]);
        }
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

/// Reduz as linhas de host por balde temporal e só decodifica `disks` JSON e
/// RFC3339 para os pontos de saída (~360 em 30d em vez de ~43.200).
fn downsample_host_rows(
    rows: Vec<host_metrics_1m::Model>,
    step_minutes: usize,
) -> Vec<HostHistoryPoint> {
    group_by_time_bucket(rows, step_minutes, |row| row.bucket_at.timestamp())
        .into_iter()
        .filter_map(|mut group| {
            let first_at = group.first()?.bucket_at.with_timezone(&Utc).to_rfc3339();
            let cpu_avg = mean(group.iter().map(|p| p.cpu_avg));
            let cpu_max = group.iter().map(|p| p.cpu_max).fold(0.0, f64::max);
            let memory_used = mean(group.iter().map(|p| p.memory_used_avg));
            let load1 = mean(group.iter().map(|p| p.load1));
            let net_rx_bps = mean(group.iter().map(|p| p.net_rx_bps));
            let net_tx_bps = mean(group.iter().map(|p| p.net_tx_bps));
            let last = group.pop()?;
            Some(HostHistoryPoint {
                at: first_at,
                cpu_avg,
                cpu_max,
                memory_used,
                memory_total: last.memory_total,
                load1,
                net_rx_bps,
                net_tx_bps,
                disks: serde_json::from_value::<Vec<DiskUsage>>(last.disks).unwrap_or_default(),
            })
        })
        .collect()
}

fn downsample_container_rows(
    rows: Vec<container_metrics_1m::Model>,
    step_minutes: usize,
) -> Vec<ContainerHistoryPoint> {
    group_by_time_bucket(rows, step_minutes, |row| row.bucket_at.timestamp())
        .into_iter()
        .filter_map(|group| {
            let first_at = group.first()?.bucket_at.with_timezone(&Utc).to_rfc3339();
            Some(ContainerHistoryPoint {
                at: first_at,
                cpu_avg: mean(group.iter().map(|p| p.cpu_avg)),
                cpu_max: group.iter().map(|p| p.cpu_max).fold(0.0, f64::max),
                memory_avg: mean(group.iter().map(|p| p.memory_avg)),
                memory_max: group.iter().map(|p| p.memory_max).max().unwrap_or(0),
                net_rx_bytes: group
                    .iter()
                    .map(|p| p.net_rx_bytes)
                    .fold(0_i64, i64::saturating_add),
                net_tx_bytes: group
                    .iter()
                    .map(|p| p.net_tx_bytes)
                    .fold(0_i64, i64::saturating_add),
                block_read_bytes: group
                    .iter()
                    .map(|p| p.block_read_bytes)
                    .fold(0_i64, i64::saturating_add),
                block_write_bytes: group
                    .iter()
                    .map(|p| p.block_write_bytes)
                    .fold(0_i64, i64::saturating_add),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn container(name: &str, cpu: f64) -> ContainerRollup {
        ContainerRollup {
            name: name.to_string(),
            project: None,
            cpu_avg: cpu,
            cpu_max: cpu,
            memory_avg: 0.0,
            memory_max: 0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            block_read_bytes: 0,
            block_write_bytes: 0,
        }
    }

    #[test]
    fn nome_repetido_no_lote_vira_uma_linha_com_a_ultima_ocorrencia() {
        let longo = "x".repeat(300);
        let lote = vec![
            container("web", 1.0),
            container(&longo, 2.0),
            container("web", 3.0),
            container(&format!("{longo}-outro"), 4.0),
        ];
        let unicos = unique_by_column_name(&lote);
        assert_eq!(unicos.len(), 2);
        assert_eq!(unicos[0].0, "web");
        assert!((unicos[0].1.cpu_avg - 3.0).abs() < f64::EPSILON);
        assert_eq!(unicos[1].0.chars().count(), 255);
        assert!((unicos[1].1.cpu_avg - 4.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn rollup_vazio_nao_abre_transacao() {
        // Conexão desconectada: qualquer `BEGIN` falharia.
        let db = sea_orm::DatabaseConnection::default();
        let vazio = MetricsRollup {
            bucket_at: Utc::now(),
            samples: 0,
            host: None,
            containers: Vec::new(),
        };
        assert!(save(&db, "agent-1", &vazio).await.is_ok());
    }

    fn container_row(minute: i64, cpu: f64, rx: i64) -> container_metrics_1m::Model {
        let base = Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
        let bucket = (base + chrono::Duration::minutes(minute)).fixed_offset();
        container_metrics_1m::Model {
            id: minute,
            host_key: "agent-1".into(),
            container_name: "api".into(),
            project: None,
            bucket_at: bucket,
            cpu_avg: cpu,
            cpu_max: cpu,
            memory_avg: 10.0,
            memory_max: 10,
            net_rx_bytes: rx,
            net_tx_bytes: 0,
            block_read_bytes: 0,
            block_write_bytes: 0,
            created_at: bucket,
        }
    }

    #[test]
    fn reducao_soma_contadores_e_media_gauges() {
        let rows = (0..5).map(|m| container_row(m, m as f64, 10)).collect();
        let reduced = downsample_container_rows(rows, 2);
        assert_eq!(reduced.len(), 3);
        assert!((reduced[0].cpu_avg - 0.5).abs() < f64::EPSILON);
        assert!((reduced[0].cpu_max - 1.0).abs() < f64::EPSILON);
        assert_eq!(reduced[0].net_rx_bytes, 20);
        assert_eq!(reduced[2].net_rx_bytes, 10, "sobra vira ponto próprio");
    }

    #[test]
    fn passo_um_preserva_os_pontos() {
        let rows: Vec<_> = (0..3).map(|m| container_row(m, 1.0, 1)).collect();
        assert_eq!(downsample_container_rows(rows, 1).len(), 3);
    }

    #[test]
    fn lacuna_offline_nao_mistura_baldes_distantes() {
        // Minuto 0 e minuto 180 (3h depois) não podem ser fundidos num balde de 30 min.
        let rows = vec![container_row(0, 10.0, 100), container_row(180, 90.0, 200)];
        let reduced = downsample_container_rows(rows, 30);
        assert_eq!(reduced.len(), 2);
        assert!((reduced[0].cpu_avg - 10.0).abs() < f64::EPSILON);
        assert!((reduced[1].cpu_avg - 90.0).abs() < f64::EPSILON);
    }
}
