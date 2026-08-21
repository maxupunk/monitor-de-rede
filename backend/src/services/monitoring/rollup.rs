//! Rollup horário de `monitor_results` (Fase 3 do roadmap).
//!
//! Agrega o histórico bruto de checagens em buckets de uma hora. A pergunta que
//! esta tabela responde é "este link é estável?" em 24h / 7d / 30d, sem varrer
//! milhões de linhas a cada consulta.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::{
    models::{monitor_results, monitor_results_hourly},
    services::shared::errors::AppResult,
};

/// Quantas horas de bruto são mantidas após o rollup.
///
/// Desligado por padrão (`0`): a purga existente continua responsável por
/// apagar `monitor_results`, e o rollup não quebra o sparkline recente. Quando
/// configurado, apaga apenas buckets completos já copiados para a tabela hourly.
pub const DEFAULT_DELETE_BRUTO_AFTER_HOURS: i64 = 0;

/// Intervalo entre execuções do rollup no scheduler, em segundos.
pub const DEFAULT_ROLLUP_INTERVAL_SECONDS: i64 = 3600;

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
    let hour_start = ts - (ts % 3600);
    DateTime::from_timestamp(hour_start, 0).expect("timestamp de hora válido")
}

/// Executa o rollup de todos os buckets completos anteriores a `until`.
///
/// Um bucket só é considerado fechado quando a hora seguinte já começou, ou
/// seja, quando `until` é superior ao fim do bucket. Na prática `until` vem
/// truncado para a hora atual, então a hora em curso nunca é rollupada.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn rollup_monitor_results<C>(db: &C, until: DateTime<Utc>) -> AppResult<RollupStats>
where
    C: ConnectionTrait,
{
    let cutoff = truncate_to_hour(until);
    let rows = monitor_results::Entity::find()
        .filter(monitor_results::Column::StartedAt.lt(cutoff))
        .order_by_asc(monitor_results::Column::StartedAt)
        .all(db)
        .await?;

    if rows.is_empty() {
        return Ok(RollupStats::default());
    }

    // Agrupa por (monitor_id, probe_id, bucket).
    let mut buckets: BTreeMap<(i64, Option<i64>, DateTime<Utc>), Accumulator> = BTreeMap::new();
    for row in &rows {
        let bucket = truncate_to_hour(row.started_at.with_timezone(&Utc));
        let key = (row.monitor_id, row.probe_id, bucket);
        let acc = buckets.entry(key).or_default();
        acc.add(row);
    }

    let buckets_to_write: Vec<_> = buckets.into_iter().collect();
    let bucket_keys: Vec<(i64, DateTime<Utc>)> = buckets_to_write
        .iter()
        .map(|((monitor_id, _probe_id, bucket), _)| (*monitor_id, *bucket))
        .collect();

    // Remove entradas hourly que serão reescritas. Idempotência simples e
    // portável entre SQLite e Postgres.
    for (monitor_id, bucket) in &bucket_keys {
        monitor_results_hourly::Entity::delete_many()
            .filter(monitor_results_hourly::Column::MonitorId.eq(*monitor_id))
            .filter(monitor_results_hourly::Column::Bucket.eq(*bucket))
            .exec(db)
            .await?;
    }

    let mut stats = RollupStats {
        buckets_upserted: buckets_to_write.len() as u64,
        rows_aggregated: rows.len() as u64,
        ..Default::default()
    };

    let active_models: Vec<monitor_results_hourly::ActiveModel> = buckets_to_write
        .into_iter()
        .map(|((monitor_id, probe_id, bucket), acc)| {
            acc.into_active_model(monitor_id, probe_id, bucket)
        })
        .collect();

    for chunk in active_models.chunks(500) {
        monitor_results_hourly::Entity::insert_many(chunk.to_vec())
            .exec(db)
            .await?;
    }

    let delete_after = std::env::var("ROLLUP_DELETE_BRUTO_AFTER_HOURS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(DEFAULT_DELETE_BRUTO_AFTER_HOURS);

    if delete_after > 0 {
        let delete_cutoff = cutoff - Duration::hours(delete_after);
        let deleted = monitor_results::Entity::delete_many()
            .filter(monitor_results::Column::StartedAt.lt(delete_cutoff))
            .exec(db)
            .await?
            .rows_affected;
        stats.rows_deleted = deleted;
    }

    Ok(stats)
}

#[derive(Debug, Default)]
struct Accumulator {
    total: i32,
    up: i32,
    down: i32,
    unknown: i32,
    latencies: Vec<f64>,
    first_started_at: Option<DateTime<Utc>>,
    last_finished_at: Option<DateTime<Utc>>,
}

impl Accumulator {
    fn add(&mut self, row: &monitor_results::Model) {
        self.total += 1;
        match row.status.as_str() {
            "up" => self.up += 1,
            "down" => self.down += 1,
            _ => self.unknown += 1,
        }
        if let Some(latency) = row.latency_ms {
            if latency.is_finite() {
                self.latencies.push(latency);
            }
        }
        let started = row.started_at.with_timezone(&Utc);
        let finished = row.finished_at.with_timezone(&Utc);
        self.first_started_at = Some(
            self.first_started_at
                .map_or(started, |current| current.min(started)),
        );
        self.last_finished_at = Some(
            self.last_finished_at
                .map_or(finished, |current| current.max(finished)),
        );
    }

    fn into_active_model(
        self,
        monitor_id: i64,
        probe_id: Option<i64>,
        bucket: DateTime<Utc>,
    ) -> monitor_results_hourly::ActiveModel {
        let (avg, min, max) = if self.latencies.is_empty() {
            (None, None, None)
        } else {
            let sum: f64 = self.latencies.iter().sum();
            let avg = sum / self.latencies.len() as f64;
            let min = self.latencies.iter().copied().fold(f64::INFINITY, f64::min);
            let max = self
                .latencies
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (Some(avg), Some(min), Some(max))
        };

        monitor_results_hourly::ActiveModel {
            monitor_id: Set(monitor_id),
            probe_id: Set(probe_id),
            bucket: Set(bucket.into()),
            total_checks: Set(self.total),
            up_checks: Set(self.up),
            down_checks: Set(self.down),
            unknown_checks: Set(self.unknown),
            avg_latency_ms: Set(avg),
            min_latency_ms: Set(min),
            max_latency_ms: Set(max),
            first_started_at: Set(self.first_started_at.unwrap_or(bucket).into()),
            last_finished_at: Set(self.last_finished_at.unwrap_or(bucket).into()),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Timelike};

    #[test]
    fn trunca_para_inicio_da_hora() {
        let dt = Utc::now();
        let truncated = truncate_to_hour(dt);
        assert_eq!(truncated.minute(), 0);
        assert_eq!(truncated.second(), 0);
        assert_eq!(truncated.nanosecond(), 0);
    }

    #[test]
    fn acumulador_conta_status_e_latencias() {
        let t0 = Utc::now();
        let mut acc = Accumulator::default();
        acc.add(&monitor_results::Model {
            id: 1,
            monitor_id: 1,
            probe_id: None,
            status: "up".into(),
            started_at: t0.into(),
            finished_at: (t0 + Duration::seconds(1)).into(),
            duration_ms: 10,
            latency_ms: Some(12.5),
            message: None,
            data: None,
            created_at: t0.into(),
        });
        acc.add(&monitor_results::Model {
            id: 2,
            monitor_id: 1,
            probe_id: None,
            status: "down".into(),
            started_at: (t0 + Duration::seconds(5)).into(),
            finished_at: (t0 + Duration::seconds(6)).into(),
            duration_ms: 10,
            latency_ms: None,
            message: None,
            data: None,
            created_at: t0.into(),
        });

        assert_eq!(acc.total, 2);
        assert_eq!(acc.up, 1);
        assert_eq!(acc.down, 1);
        assert_eq!(acc.unknown, 0);
        assert_eq!(acc.latencies, vec![12.5]);
    }
}
