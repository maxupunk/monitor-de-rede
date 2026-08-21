//! Cálculo de uptime de um monitor a partir dos buckets horários e dos resultados
//! brutos da hora em curso.
//!
//! A ideia é responder "qual a disponibilidade nas últimas N horas?" sem varrer
//! toda a tabela `monitor_results`. Os buckets fechados vêm de
//! `monitor_results_hourly`; a hora atual (ainda aberta) é lida de
//! `monitor_results` e agregada no mesmo formato.

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::{
    models::{monitor_results, monitor_results_hourly},
    services::shared::errors::AppResult,
};

use super::rollup::truncate_to_hour;

/// Estatísticas consolidadas de uptime para um monitor em uma janela de tempo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UptimeStats {
    pub total_checks: i64,
    pub up_checks: i64,
    pub down_checks: i64,
    pub unknown_checks: i64,
    pub uptime_percentage: f64,
    pub avg_latency_ms: Option<f64>,
}

impl Default for UptimeStats {
    fn default() -> Self {
        Self {
            total_checks: 0,
            up_checks: 0,
            down_checks: 0,
            unknown_checks: 0,
            uptime_percentage: 100.0,
            avg_latency_ms: None,
        }
    }
}

impl UptimeStats {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total_checks == 0
    }

    /// Mescla outro conjunto de contagens, recalculando o uptime.
    ///
    /// A latência média é ponderada pelo número de checagens que a possuíram.
    pub fn merge(&mut self, other: &Self) {
        let total = self.total_checks + other.total_checks;
        if total == 0 {
            return;
        }
        self.avg_latency_ms = match (self.avg_latency_ms, other.avg_latency_ms) {
            (Some(a), Some(b)) => {
                let weighed =
                    (a * self.total_checks as f64 + b * other.total_checks as f64) / total as f64;
                Some(weighed)
            }
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        self.total_checks = total;
        self.up_checks += other.up_checks;
        self.down_checks += other.down_checks;
        self.unknown_checks += other.unknown_checks;
        self.recalculate_uptime();
    }

    fn recalculate_uptime(&mut self) {
        self.uptime_percentage = if self.total_checks == 0 {
            100.0
        } else {
            let decisive = self.up_checks + self.down_checks;
            if decisive == 0 {
                100.0
            } else {
                (self.up_checks as f64 * 1000.0 / decisive as f64).round() / 10.0
            }
        };
    }
}

/// Calcula o uptime de um monitor nas últimas `hours` horas.
///
/// A janela inclui a hora atual (parcial). Buckets horários fechados são
/// somados de `monitor_results_hourly`; resultados brutos da hora em curso são
/// lidos de `monitor_results` e agregados dinamicamente.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn uptime_for_monitor<C>(db: &C, monitor_id: i64, hours: i64) -> AppResult<UptimeStats>
where
    C: ConnectionTrait,
{
    if hours <= 0 {
        return Ok(UptimeStats::default());
    }

    let now = Utc::now();
    let window_start = truncate_to_hour(now - Duration::hours(hours - 1));

    let hourly = monitor_results_hourly::Entity::find()
        .filter(monitor_results_hourly::Column::MonitorId.eq(monitor_id))
        .filter(monitor_results_hourly::Column::Bucket.gte(window_start))
        .order_by_asc(monitor_results_hourly::Column::Bucket)
        .all(db)
        .await?;

    let mut stats = UptimeStats::default();
    for bucket in &hourly {
        stats.merge(&UptimeStats {
            total_checks: i64::from(bucket.total_checks),
            up_checks: i64::from(bucket.up_checks),
            down_checks: i64::from(bucket.down_checks),
            unknown_checks: i64::from(bucket.unknown_checks),
            uptime_percentage: 0.0,
            avg_latency_ms: bucket.avg_latency_ms,
        });
    }

    // Agrega a hora atual (bucket ainda aberto) a partir dos resultados brutos.
    let current_hour_start = truncate_to_hour(now);
    let partial = monitor_results::Entity::find()
        .filter(monitor_results::Column::MonitorId.eq(monitor_id))
        .filter(monitor_results::Column::StartedAt.gte(current_hour_start))
        .all(db)
        .await?;

    if !partial.is_empty() {
        let mut partial_stats = UptimeStats::default();
        let mut latency_sum = 0.0;
        let mut latency_count = 0;
        for row in &partial {
            partial_stats.total_checks += 1;
            match row.status.as_str() {
                "up" => partial_stats.up_checks += 1,
                "down" => partial_stats.down_checks += 1,
                _ => partial_stats.unknown_checks += 1,
            }
            if let Some(latency) = row.latency_ms.filter(|value| value.is_finite()) {
                latency_sum += latency;
                latency_count += 1;
            }
        }
        partial_stats.avg_latency_ms =
            (latency_count > 0).then(|| latency_sum / latency_count as f64);
        partial_stats.recalculate_uptime();
        stats.merge(&partial_stats);
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_vazio_e_cem_por_cento() {
        let stats = UptimeStats::default();
        assert!(stats.is_empty());
        assert_eq!(stats.uptime_percentage, 100.0);
    }

    #[test]
    fn merge_soma_contagens_e_recalcula_uptime() {
        let mut a = UptimeStats {
            total_checks: 10,
            up_checks: 9,
            down_checks: 1,
            unknown_checks: 0,
            uptime_percentage: 0.0,
            avg_latency_ms: Some(10.0),
        };
        a.recalculate_uptime();
        let mut b = UptimeStats {
            total_checks: 10,
            up_checks: 8,
            down_checks: 2,
            unknown_checks: 0,
            uptime_percentage: 0.0,
            avg_latency_ms: Some(20.0),
        };
        b.recalculate_uptime();

        a.merge(&b);

        assert_eq!(a.total_checks, 20);
        assert_eq!(a.up_checks, 17);
        assert_eq!(a.down_checks, 3);
        assert_eq!(a.avg_latency_ms, Some(15.0));
        assert_eq!(a.uptime_percentage, 85.0);
    }
}
