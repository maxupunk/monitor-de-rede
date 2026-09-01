//! Rollup horário de `monitor_results`.
//!
//! A agregação acontece no banco e devolve somente um registro por
//! `(monitor, hora)`. O processo nunca materializa as checagens brutas, mesmo
//! durante a recuperação de um backlog grande.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, Set, Statement, TransactionTrait, Value,
};

use crate::{
    models::{monitor_results, monitor_results_hourly},
    services::shared::errors::{AppError, AppResult},
};

/// Quantas horas de bruto são mantidas após o rollup.
///
/// Desligado por padrão (`0`): a purga existente continua responsável por
/// apagar `monitor_results`, e o rollup não quebra o sparkline recente.
pub const DEFAULT_DELETE_BRUTO_AFTER_HOURS: i64 = 0;

/// Intervalo entre execuções do rollup no scheduler, em segundos.
pub const DEFAULT_ROLLUP_INTERVAL_SECONDS: i64 = 3_600;

/// Máximo de histórico agregado em uma execução. Uma instalação com backlog
/// avança uma semana por ciclo sem alocar todo o passado no processo.
pub const MAX_ROLLUP_HOURS_PER_RUN: i64 = 24 * 7;

/// Horas fechadas recalculadas para absorver resultados atrasados.
const ROLLUP_LOOKBACK_HOURS: i64 = 2;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RollupStats {
    pub buckets_upserted: u64,
    pub rows_aggregated: u64,
    pub rows_deleted: u64,
}

impl RollupStats {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.buckets_upserted + self.rows_aggregated + self.rows_deleted
    }
}

/// Trunca um instante para o início da hora UTC.
#[must_use]
pub fn truncate_to_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    let ts = dt.timestamp();
    let hour_start = ts - (ts % 3_600);
    DateTime::from_timestamp(hour_start, 0).expect("timestamp de hora válido")
}

/// Agrega buckets completos anteriores a `until`.
///
/// O intervalo é limitado, a agregação é feita por SQL e a substituição dos
/// buckets ocorre na mesma transação. O bucket mais recente é recalculado para
/// incorporar resultados que chegaram com atraso.
pub async fn rollup_monitor_results(
    db: &DatabaseConnection,
    until: DateTime<Utc>,
) -> AppResult<RollupStats> {
    let cutoff = truncate_to_hour(until);
    let Some(earliest) = monitor_results::Entity::find()
        .filter(monitor_results::Column::StartedAt.lt(cutoff))
        .order_by_asc(monitor_results::Column::StartedAt)
        .one(db)
        .await?
    else {
        return Ok(RollupStats::default());
    };

    let earliest_bucket = truncate_to_hour(earliest.started_at.with_timezone(&Utc));
    let latest_bucket = monitor_results_hourly::Entity::find()
        .filter(monitor_results_hourly::Column::Bucket.lt(cutoff))
        .order_by_desc(monitor_results_hourly::Column::Bucket)
        .one(db)
        .await?
        .map(|row| truncate_to_hour(row.bucket.with_timezone(&Utc)));
    let start = latest_bucket.map_or(earliest_bucket, |latest| {
        earliest_bucket.max(latest - Duration::hours(ROLLUP_LOOKBACK_HOURS))
    });
    let end = cutoff.min(start + Duration::hours(MAX_ROLLUP_HOURS_PER_RUN));
    if start >= end {
        return Ok(RollupStats::default());
    }

    let txn = db.begin().await?;
    let aggregates = aggregate_range(&txn, start, end).await?;
    let rows_aggregated = aggregates.iter().try_fold(0_u64, |total, row| {
        u64::try_from(row.total_checks)
            .ok()
            .and_then(|count| total.checked_add(count))
    });
    let rows_aggregated = rows_aggregated
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("contagem do rollup excede u64")))?;

    monitor_results_hourly::Entity::delete_many()
        .filter(monitor_results_hourly::Column::Bucket.gte(start))
        .filter(monitor_results_hourly::Column::Bucket.lt(end))
        .exec(&txn)
        .await?;

    let mut stats = RollupStats {
        buckets_upserted: aggregates.len() as u64,
        rows_aggregated,
        ..Default::default()
    };
    let mut insert_batch = Vec::with_capacity(500);
    for aggregate in aggregates {
        insert_batch.push(aggregate.into_active_model()?);
        if insert_batch.len() == 500 {
            monitor_results_hourly::Entity::insert_many(std::mem::take(&mut insert_batch))
                .exec(&txn)
                .await?;
        }
    }
    if !insert_batch.is_empty() {
        monitor_results_hourly::Entity::insert_many(insert_batch)
            .exec(&txn)
            .await?;
    }

    let delete_after = std::env::var("ROLLUP_DELETE_BRUTO_AFTER_HOURS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(DEFAULT_DELETE_BRUTO_AFTER_HOURS);
    if delete_after > 0 {
        let delete_cutoff = cutoff - Duration::hours(delete_after);
        stats.rows_deleted = monitor_results::Entity::delete_many()
            .filter(monitor_results::Column::StartedAt.lt(delete_cutoff))
            // Durante a recuperação de backlog, horas posteriores a `end`
            // ainda não foram agregadas e não podem ser descartadas.
            .filter(monitor_results::Column::StartedAt.lt(end))
            .exec(&txn)
            .await?
            .rows_affected;
    }

    txn.commit().await?;
    Ok(stats)
}

#[derive(Debug, FromQueryResult)]
struct HourlyAggregate {
    monitor_id: i64,
    probe_id: Option<i64>,
    bucket: sea_orm::prelude::DateTimeWithTimeZone,
    total_checks: i64,
    up_checks: i64,
    down_checks: i64,
    unknown_checks: i64,
    avg_latency_ms: Option<f64>,
    min_latency_ms: Option<f64>,
    max_latency_ms: Option<f64>,
    first_started_at: sea_orm::prelude::DateTimeWithTimeZone,
    last_finished_at: sea_orm::prelude::DateTimeWithTimeZone,
}

impl HourlyAggregate {
    fn into_active_model(self) -> AppResult<monitor_results_hourly::ActiveModel> {
        let count = |name: &str, value: i64| {
            i32::try_from(value)
                .map_err(|_| AppError::Internal(anyhow::anyhow!("{name} do rollup excede i32")))
        };
        Ok(monitor_results_hourly::ActiveModel {
            monitor_id: Set(self.monitor_id),
            probe_id: Set(self.probe_id),
            bucket: Set(self.bucket),
            total_checks: Set(count("total_checks", self.total_checks)?),
            up_checks: Set(count("up_checks", self.up_checks)?),
            down_checks: Set(count("down_checks", self.down_checks)?),
            unknown_checks: Set(count("unknown_checks", self.unknown_checks)?),
            avg_latency_ms: Set(self.avg_latency_ms),
            min_latency_ms: Set(self.min_latency_ms),
            max_latency_ms: Set(self.max_latency_ms),
            first_started_at: Set(self.first_started_at),
            last_finished_at: Set(self.last_finished_at),
            ..Default::default()
        })
    }
}

async fn aggregate_range<C>(
    db: &C,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> AppResult<Vec<HourlyAggregate>>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (start_marker, end_marker, bucket_expression) = match backend {
        DatabaseBackend::Postgres => ("$1", "$2", "date_trunc('hour', started_at)"),
        _ => ("?", "?", "strftime('%Y-%m-%dT%H:00:00+00:00', started_at)"),
    };
    let sql = format!(
        "SELECT monitor_id, \
                CASE WHEN COUNT(probe_id) = COUNT(*) AND MIN(probe_id) = MAX(probe_id) \
                     THEN MIN(probe_id) ELSE NULL END AS probe_id, \
                {bucket_expression} AS bucket, \
                COUNT(*) AS total_checks, \
                SUM(CASE WHEN status = 'up' THEN 1 ELSE 0 END) AS up_checks, \
                SUM(CASE WHEN status = 'down' THEN 1 ELSE 0 END) AS down_checks, \
                SUM(CASE WHEN status NOT IN ('up', 'down') THEN 1 ELSE 0 END) AS unknown_checks, \
                AVG(latency_ms) AS avg_latency_ms, \
                MIN(latency_ms) AS min_latency_ms, \
                MAX(latency_ms) AS max_latency_ms, \
                MIN(started_at) AS first_started_at, \
                MAX(finished_at) AS last_finished_at \
           FROM monitor_results \
          WHERE started_at >= {start_marker} AND started_at < {end_marker} \
          GROUP BY monitor_id, {bucket_expression} \
          ORDER BY bucket ASC, monitor_id ASC"
    );

    Ok(
        HourlyAggregate::find_by_statement(Statement::from_sql_and_values(
            backend,
            sql,
            Vec::<Value>::from([start.into(), end.into()]),
        ))
        .all(db)
        .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn trunca_para_inicio_da_hora() {
        let truncated = truncate_to_hour(Utc::now());
        assert_eq!(truncated.minute(), 0);
        assert_eq!(truncated.second(), 0);
        assert_eq!(truncated.nanosecond(), 0);
    }

    #[test]
    fn janela_de_backlog_tem_teto_explicito() {
        assert_eq!(MAX_ROLLUP_HOURS_PER_RUN, 168);
        assert_eq!(ROLLUP_LOOKBACK_HOURS, 2);
    }
}
