//! Cálculo de baseline móvel para alertas por desvio.
//!
//! A baseline é a média histórica de uma métrica ao longo de uma janela
//! deslizante (padrão 7 dias). Ela é calculada a partir dos buckets horários de
//! `monitor_results_hourly`, evitando varrer a tabela bruta a cada ciclo.
//!
//! O resultado é publicado no dataset de alertas como campos auxiliares; o
//! avaliador continua usando operadores simples (`gt`, `gte`), o que mantém a
//! compatibilidade com regras manuais e com o catálogo.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use chrono::Utc;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::{models::monitor_results_hourly, services::shared::errors::AppResult};

/// Janela padrão de baseline, em dias. Pode ser sobrescrita por
/// `BASELINE_WINDOW_DAYS`.
pub const DEFAULT_WINDOW_DAYS: i64 = 7;

/// Número mínimo de buckets horários para que uma baseline seja considerada
/// confiável. Menos que isso, os campos de baseline não são publicados.
pub const MIN_BUCKETS_FOR_BASELINE: usize = 6;

/// TTL do cache em memória, em segundos. A baseline muda lentamente (só quando
/// fecha um novo bucket), então 1h é suficiente e barato.
const CACHE_TTL_SECONDS: u64 = 3_600;

/// Métricas de baseline calculadas para um monitor.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MonitorBaseline {
    pub latency_baseline_ms: Option<f64>,
    pub latency_deviation_percent: Option<f64>,
    pub packet_loss_baseline_percent: Option<f64>,
    pub packet_loss_deviation_percent: Option<f64>,
    pub uptime_baseline_percent: Option<f64>,
    pub uptime_deviation_percent: Option<f64>,
}

impl MonitorBaseline {
    /// `true` quando não há baseline disponível.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.latency_baseline_ms.is_none()
            && self.packet_loss_baseline_percent.is_none()
            && self.uptime_baseline_percent.is_none()
    }
}

/// Entrada do cache de baseline.
#[derive(Debug, Clone)]
struct CacheEntry {
    baseline: MonitorBaseline,
    fetched_at: Instant,
}

fn cache() -> &'static Mutex<HashMap<(i64, i64), CacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<(i64, i64), CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Limpa o cache de baseline.
///
/// Expõe principalmente para testes: o cache é process-global, mas os bancos de
/// dados dos testes de integração são recriados a cada caso, então limpar entre
/// eles evita baseline vazia de um caso anterior contaminar o seguinte.
pub fn clear_cache() {
    if let Ok(mut cache) = cache().lock() {
        cache.clear();
    }
}

fn window_days() -> i64 {
    std::env::var("BASELINE_WINDOW_DAYS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_WINDOW_DAYS)
}

/// Lê a baseline de um monitor, usando cache quando possível.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn for_monitor<C>(db: &C, monitor_id: i64) -> AppResult<MonitorBaseline>
where
    C: ConnectionTrait,
{
    let days = window_days();
    let key = (monitor_id, days);

    {
        let cache = cache()
            .lock()
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;
        if let Some(entry) = cache.get(&key) {
            if entry.fetched_at.elapsed() < Duration::from_secs(CACHE_TTL_SECONDS) {
                return Ok(entry.baseline);
            }
        }
    }

    let baseline = compute_for_monitor(db, monitor_id, days).await?;

    {
        let mut cache = cache()
            .lock()
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error.to_string())))?;
        cache.insert(
            key,
            CacheEntry {
                baseline,
                fetched_at: Instant::now(),
            },
        );
    }

    Ok(baseline)
}

use crate::services::shared::errors::AppError;

/// Calcula a baseline a partir dos buckets horários da janela.
async fn compute_for_monitor<C>(db: &C, monitor_id: i64, days: i64) -> AppResult<MonitorBaseline>
where
    C: ConnectionTrait,
{
    let since = Utc::now() - chrono::Duration::days(days);
    let buckets = monitor_results_hourly::Entity::find()
        .filter(monitor_results_hourly::Column::MonitorId.eq(monitor_id))
        .filter(monitor_results_hourly::Column::Bucket.gte(since))
        .order_by_asc(monitor_results_hourly::Column::Bucket)
        .all(db)
        .await?;

    if buckets.len() < MIN_BUCKETS_FOR_BASELINE {
        return Ok(MonitorBaseline::default());
    }

    let mut latency_sum = 0.0;
    let mut latency_count = 0usize;
    let mut packet_loss_sum = 0.0;
    let mut packet_loss_count = 0usize;
    let mut uptime_sum = 0.0;
    let mut uptime_count = 0usize;

    for bucket in &buckets {
        if let Some(avg) = bucket
            .avg_latency_ms
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            latency_sum += avg;
            latency_count += 1;
        }

        if bucket.total_checks > 0 {
            let decisive = bucket.up_checks + bucket.down_checks;
            if decisive > 0 {
                let uptime = f64::from(bucket.up_checks) * 100.0 / f64::from(decisive);
                uptime_sum += uptime;
                uptime_count += 1;
            }

            let loss = f64::from(bucket.down_checks) * 100.0 / f64::from(bucket.total_checks);
            packet_loss_sum += loss;
            packet_loss_count += 1;
        }
    }

    if latency_count == 0 && uptime_count == 0 && packet_loss_count == 0 {
        return Ok(MonitorBaseline::default());
    }

    let latency_baseline_ms = (latency_count > 0).then(|| latency_sum / latency_count as f64);
    let packet_loss_baseline_percent =
        (packet_loss_count > 0).then(|| packet_loss_sum / packet_loss_count as f64);
    let uptime_baseline_percent = (uptime_count > 0).then(|| uptime_sum / uptime_count as f64);

    Ok(MonitorBaseline {
        latency_baseline_ms,
        latency_deviation_percent: None,
        packet_loss_baseline_percent,
        packet_loss_deviation_percent: None,
        uptime_baseline_percent,
        uptime_deviation_percent: None,
    })
}

/// Enriquece uma baseline já calculada com os desvios em relação ao valor
/// atual.
#[must_use]
pub fn with_current_value(
    baseline: &MonitorBaseline,
    latency_ms: Option<f64>,
    packet_loss_percent: Option<f64>,
    uptime_percent: Option<f64>,
) -> MonitorBaseline {
    MonitorBaseline {
        latency_baseline_ms: baseline.latency_baseline_ms,
        latency_deviation_percent: deviation_percent(latency_ms, baseline.latency_baseline_ms),
        packet_loss_baseline_percent: baseline.packet_loss_baseline_percent,
        packet_loss_deviation_percent: absolute_deviation_percent(
            packet_loss_percent,
            baseline.packet_loss_baseline_percent,
        ),
        uptime_baseline_percent: baseline.uptime_baseline_percent,
        uptime_deviation_percent: uptime_deviation_percent(
            uptime_percent,
            baseline.uptime_baseline_percent,
        ),
    }
}

fn deviation_percent(current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
    match (current, baseline) {
        (Some(current), Some(baseline)) if baseline > 0.0 && current.is_finite() => {
            Some(((current - baseline) / baseline) * 100.0)
        }
        _ => None,
    }
}

fn absolute_deviation_percent(current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
    match (current, baseline) {
        (Some(current), Some(baseline)) if current.is_finite() && baseline.is_finite() => {
            Some((current - baseline).abs())
        }
        _ => None,
    }
}

fn uptime_deviation_percent(current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
    match (current, baseline) {
        (Some(current), Some(baseline)) if current.is_finite() && baseline.is_finite() => {
            Some((baseline - current).max(0.0))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desvio_percentual_acima_da_baseline() {
        let baseline = MonitorBaseline {
            latency_baseline_ms: Some(100.0),
            ..Default::default()
        };
        let enriched = with_current_value(&baseline, Some(150.0), None, None);
        assert_eq!(enriched.latency_deviation_percent, Some(50.0));
    }

    #[test]
    fn desvio_de_perda_e_absoluto() {
        let baseline = MonitorBaseline {
            packet_loss_baseline_percent: Some(2.0),
            ..Default::default()
        };
        let enriched = with_current_value(&baseline, None, Some(5.0), None);
        assert_eq!(enriched.packet_loss_deviation_percent, Some(3.0));
    }

    #[test]
    fn desvio_de_uptime_e_queda_em_relacao_a_baseline() {
        let baseline = MonitorBaseline {
            uptime_baseline_percent: Some(99.9),
            ..Default::default()
        };
        let enriched = with_current_value(&baseline, None, None, Some(98.0));
        assert!(enriched
            .uptime_deviation_percent
            .is_some_and(|value| (value - 1.9).abs() < 0.001));

        let above_baseline = with_current_value(&baseline, None, None, Some(100.0));
        assert_eq!(above_baseline.uptime_deviation_percent, Some(0.0));
    }

    #[test]
    fn baseline_vazia_quando_sem_dados_suficientes() {
        let empty = MonitorBaseline::default();
        assert!(empty.is_empty());
        let enriched = with_current_value(&empty, Some(150.0), Some(5.0), Some(98.0));
        assert!(enriched.is_empty());
    }
}
