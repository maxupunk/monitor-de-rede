//! Cálculo de baseline móvel e detecção de anomalias estatísticas.
//!
//! A baseline estatística compreende a média histórica ($\mu$), desvio padrão
//! ($\sigma$), Z-Scores ($z$) e bandas de confiança calculados ao longo de uma
//! janela deslizante (padrão 7 dias). Ela é calculada a partir dos buckets
//! horários de `monitor_results_hourly`, evitando varrer a tabela bruta a cada
//! ciclo.
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
use serde::{Deserialize, Serialize};

use crate::{
    models::monitor_results_hourly,
    services::shared::errors::{AppError, AppResult},
};

/// Janela padrão de baseline, em dias. Pode ser sobrescrita por
/// `BASELINE_WINDOW_DAYS`.
pub const DEFAULT_WINDOW_DAYS: i64 = 7;

/// Número mínimo de buckets horários para que uma baseline seja considerada
/// confiável. Menos que isso, os campos de baseline não são publicados.
pub const MIN_BUCKETS_FOR_BASELINE: usize = 6;

/// Piso de variância mínima para cálculo numérico estável do Z-Score em latência (ms).
pub const MIN_LATENCY_VARIANCE_FLOOR_MS: f64 = 0.5;

/// Piso de variância mínima para cálculo de Z-Score de percentual (perda / uptime).
pub const MIN_PERCENT_VARIANCE_FLOOR: f64 = 0.5;

/// Multiplicador de sigma ($\sigma$) para a banda superior/inferior de anomalia (regra 3-sigma / 99.7%).
pub const SIGMA_ANOMALY_MULTIPLIER: f64 = 3.0;

/// TTL do cache em memória, em segundos. A baseline muda lentamente (só quando
/// fecha um novo bucket), então 1h é suficiente e barato.
const CACHE_TTL_SECONDS: u64 = 3_600;

/// Métricas de baseline estatística calculadas para um monitor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorBaseline {
    // Latência
    pub latency_baseline_ms: Option<f64>,
    pub latency_stddev_ms: Option<f64>,
    pub latency_deviation_percent: Option<f64>,
    pub latency_z_score: Option<f64>,
    pub latency_upper_band_ms: Option<f64>,
    pub latency_lower_band_ms: Option<f64>,
    pub is_latency_anomaly: Option<bool>,

    // Perda de pacotes
    pub packet_loss_baseline_percent: Option<f64>,
    pub packet_loss_stddev_percent: Option<f64>,
    pub packet_loss_deviation_percent: Option<f64>,
    pub packet_loss_z_score: Option<f64>,
    pub packet_loss_upper_band_percent: Option<f64>,
    pub is_packet_loss_anomaly: Option<bool>,

    // Uptime
    pub uptime_baseline_percent: Option<f64>,
    pub uptime_stddev_percent: Option<f64>,
    pub uptime_deviation_percent: Option<f64>,
    pub uptime_z_score: Option<f64>,

    // Metadados
    pub sample_count: usize,
    pub window_days: i64,
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

/// Calcula a média aritmética de um vetor de valores.
fn calculate_mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        let sum: f64 = values.iter().sum();
        Some(sum / values.len() as f64)
    }
}

/// Calcula o desvio padrão amostral a partir da média fornecida.
fn calculate_sample_stddev(values: &[f64], mean: f64) -> f64 {
    if values.len() <= 1 {
        0.0
    } else {
        let variance_sum: f64 = values.iter().map(|&x| (x - mean).powi(2)).sum();
        (variance_sum / (values.len() - 1) as f64).sqrt()
    }
}

/// Calcula a baseline estatística a partir dos buckets horários da janela.
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

    let mut latencies = Vec::with_capacity(buckets.len());
    let mut packet_losses = Vec::with_capacity(buckets.len());
    let mut uptimes = Vec::with_capacity(buckets.len());

    for bucket in &buckets {
        if let Some(avg) = bucket
            .avg_latency_ms
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            latencies.push(avg);
        }

        if bucket.total_checks > 0 {
            let decisive = bucket.up_checks + bucket.down_checks;
            if decisive > 0 {
                let uptime = f64::from(bucket.up_checks) * 100.0 / f64::from(decisive);
                uptimes.push(uptime);
            }

            let loss = f64::from(bucket.down_checks) * 100.0 / f64::from(bucket.total_checks);
            packet_losses.push(loss);
        }
    }

    if latencies.is_empty() && uptimes.is_empty() && packet_losses.is_empty() {
        return Ok(MonitorBaseline::default());
    }

    let (latency_baseline_ms, latency_stddev_ms, latency_upper_band_ms, latency_lower_band_ms) =
        if let Some(mean) = calculate_mean(&latencies) {
            let stddev = calculate_sample_stddev(&latencies, mean);
            let upper = mean + SIGMA_ANOMALY_MULTIPLIER * stddev;
            let lower = (mean - SIGMA_ANOMALY_MULTIPLIER * stddev).max(0.0);
            (Some(mean), Some(stddev), Some(upper), Some(lower))
        } else {
            (None, None, None, None)
        };

    let (packet_loss_baseline_percent, packet_loss_stddev_percent, packet_loss_upper_band_percent) =
        if let Some(mean) = calculate_mean(&packet_losses) {
            let stddev = calculate_sample_stddev(&packet_losses, mean);
            let upper = (mean + SIGMA_ANOMALY_MULTIPLIER * stddev).min(100.0);
            (Some(mean), Some(stddev), Some(upper))
        } else {
            (None, None, None)
        };

    let (uptime_baseline_percent, uptime_stddev_percent) =
        if let Some(mean) = calculate_mean(&uptimes) {
            let stddev = calculate_sample_stddev(&uptimes, mean);
            (Some(mean), Some(stddev))
        } else {
            (None, None)
        };

    Ok(MonitorBaseline {
        latency_baseline_ms,
        latency_stddev_ms,
        latency_deviation_percent: None,
        latency_z_score: None,
        latency_upper_band_ms,
        latency_lower_band_ms,
        is_latency_anomaly: None,
        packet_loss_baseline_percent,
        packet_loss_stddev_percent,
        packet_loss_deviation_percent: None,
        packet_loss_z_score: None,
        packet_loss_upper_band_percent,
        is_packet_loss_anomaly: None,
        uptime_baseline_percent,
        uptime_stddev_percent,
        uptime_deviation_percent: None,
        uptime_z_score: None,
        sample_count: buckets.len(),
        window_days: days,
    })
}

/// Enriquece uma baseline já calculada com os desvios e Z-Scores em relação ao
/// valor atual.
#[must_use]
pub fn with_current_value(
    baseline: &MonitorBaseline,
    latency_ms: Option<f64>,
    packet_loss_percent: Option<f64>,
    uptime_percent: Option<f64>,
) -> MonitorBaseline {
    let latency_z = calculate_z_score(
        latency_ms,
        baseline.latency_baseline_ms,
        baseline.latency_stddev_ms,
        MIN_LATENCY_VARIANCE_FLOOR_MS,
    );
    let packet_loss_z = calculate_z_score(
        packet_loss_percent,
        baseline.packet_loss_baseline_percent,
        baseline.packet_loss_stddev_percent,
        MIN_PERCENT_VARIANCE_FLOOR,
    );
    let uptime_z = calculate_uptime_z_score(
        uptime_percent,
        baseline.uptime_baseline_percent,
        baseline.uptime_stddev_percent,
        MIN_PERCENT_VARIANCE_FLOOR,
    );

    let is_latency_anomaly = latency_z.map(|z| z >= SIGMA_ANOMALY_MULTIPLIER);
    let is_packet_loss_anomaly = packet_loss_z.map(|z| z >= SIGMA_ANOMALY_MULTIPLIER);

    MonitorBaseline {
        latency_baseline_ms: baseline.latency_baseline_ms,
        latency_stddev_ms: baseline.latency_stddev_ms,
        latency_deviation_percent: deviation_percent(latency_ms, baseline.latency_baseline_ms),
        latency_z_score: latency_z,
        latency_upper_band_ms: baseline.latency_upper_band_ms,
        latency_lower_band_ms: baseline.latency_lower_band_ms,
        is_latency_anomaly,
        packet_loss_baseline_percent: baseline.packet_loss_baseline_percent,
        packet_loss_stddev_percent: baseline.packet_loss_stddev_percent,
        packet_loss_deviation_percent: absolute_deviation_percent(
            packet_loss_percent,
            baseline.packet_loss_baseline_percent,
        ),
        packet_loss_z_score: packet_loss_z,
        packet_loss_upper_band_percent: baseline.packet_loss_upper_band_percent,
        is_packet_loss_anomaly,
        uptime_baseline_percent: baseline.uptime_baseline_percent,
        uptime_stddev_percent: baseline.uptime_stddev_percent,
        uptime_deviation_percent: uptime_deviation_percent(
            uptime_percent,
            baseline.uptime_baseline_percent,
        ),
        uptime_z_score: uptime_z,
        sample_count: baseline.sample_count,
        window_days: baseline.window_days,
    }
}

/// Calcula o Z-Score para métricas onde aumento indica degradação.
fn calculate_z_score(
    current: Option<f64>,
    baseline_mean: Option<f64>,
    baseline_stddev: Option<f64>,
    variance_floor: f64,
) -> Option<f64> {
    match (current, baseline_mean, baseline_stddev) {
        (Some(current), Some(mean), Some(stddev))
            if current.is_finite() && mean.is_finite() && stddev.is_finite() =>
        {
            let effective_sigma = stddev.max(variance_floor);
            Some((current - mean) / effective_sigma)
        }
        _ => None,
    }
}

/// Calcula o Z-Score de queda de uptime (onde valor abaixo da baseline indica degradação).
fn calculate_uptime_z_score(
    current: Option<f64>,
    baseline_mean: Option<f64>,
    baseline_stddev: Option<f64>,
    variance_floor: f64,
) -> Option<f64> {
    match (current, baseline_mean, baseline_stddev) {
        (Some(current), Some(mean), Some(stddev))
            if current.is_finite() && mean.is_finite() && stddev.is_finite() =>
        {
            let effective_sigma = stddev.max(variance_floor);
            Some(((mean - current).max(0.0)) / effective_sigma)
        }
        _ => None,
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
            latency_stddev_ms: Some(10.0),
            ..Default::default()
        };
        let enriched = with_current_value(&baseline, Some(150.0), None, None);
        assert_eq!(enriched.latency_deviation_percent, Some(50.0));
        assert_eq!(enriched.latency_z_score, Some(5.0));
        assert_eq!(enriched.is_latency_anomaly, Some(true));
    }

    #[test]
    fn z_score_com_piso_de_variancia() {
        // Quando stddev é 0.0 (todos os buckets idênticos a 20 ms), o piso de 0.5 ms evita divisão por zero.
        let baseline = MonitorBaseline {
            latency_baseline_ms: Some(20.0),
            latency_stddev_ms: Some(0.0),
            ..Default::default()
        };
        let enriched = with_current_value(&baseline, Some(22.0), None, None);
        // (22 - 20) / 0.5 = 4.0
        assert_eq!(enriched.latency_z_score, Some(4.0));
        assert_eq!(enriched.is_latency_anomaly, Some(true));

        let stable = with_current_value(&baseline, Some(20.5), None, None);
        // (20.5 - 20) / 0.5 = 1.0 -> não anômalo (< 3.0)
        assert_eq!(stable.latency_z_score, Some(1.0));
        assert_eq!(stable.is_latency_anomaly, Some(false));
    }

    #[test]
    fn desvio_de_perda_e_absoluto_e_z_score() {
        let baseline = MonitorBaseline {
            packet_loss_baseline_percent: Some(2.0),
            packet_loss_stddev_percent: Some(1.0),
            ..Default::default()
        };
        let enriched = with_current_value(&baseline, None, Some(6.0), None);
        assert_eq!(enriched.packet_loss_deviation_percent, Some(4.0));
        assert_eq!(enriched.packet_loss_z_score, Some(4.0));
        assert_eq!(enriched.is_packet_loss_anomaly, Some(true));
    }

    #[test]
    fn desvio_de_uptime_e_queda_em_relacao_a_baseline() {
        let baseline = MonitorBaseline {
            uptime_baseline_percent: Some(99.9),
            uptime_stddev_percent: Some(0.5),
            ..Default::default()
        };
        let enriched = with_current_value(&baseline, None, None, Some(98.0));
        assert!(enriched
            .uptime_deviation_percent
            .is_some_and(|value| (value - 1.9).abs() < 0.001));
        assert!(enriched
            .uptime_z_score
            .is_some_and(|value| (value - 3.8).abs() < 0.001));

        let above_baseline = with_current_value(&baseline, None, None, Some(100.0));
        assert_eq!(above_baseline.uptime_deviation_percent, Some(0.0));
        assert_eq!(above_baseline.uptime_z_score, Some(0.0));
    }

    #[test]
    fn baseline_vazia_quando_sem_dados_suficientes() {
        let empty = MonitorBaseline::default();
        assert!(empty.is_empty());
        let enriched = with_current_value(&empty, Some(150.0), Some(5.0), Some(98.0));
        assert!(enriched.is_empty());
        assert_eq!(enriched.is_latency_anomaly, None);
    }
}
