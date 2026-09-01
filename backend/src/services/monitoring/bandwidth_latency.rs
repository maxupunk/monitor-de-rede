//! Serviço de correlação entre consumo de banda e latência de ping.
//!
//! Cruza as métricas de tráfego de interface (`metrics`) com as checagens de
//! latência de ping (`monitor_results`) alinhadas em baldes temporais uniformes,
//! permitindo detectar saturações de link em tempo real e em janelas históricas.

use chrono::{DateTime, Duration, Utc};
use futures::TryStreamExt;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, StreamTrait};

use crate::{
    dtos::devices::{BandwidthLatencyPoint, BandwidthLatencyQuery, BandwidthLatencyResponse},
    models::{_entities::metrics as metrics_entity, monitor_results, monitors},
    services::shared::errors::AppResult,
};

#[derive(Debug, Default)]
struct DualBucketAccumulator {
    bw_sum: f64,
    bw_count: i64,
    latency_sum: f64,
    latency_count: i64,
}

/// Calcula a série temporal correlacionada de banda e latência.
pub async fn calculate_bandwidth_latency_series<C>(
    db: &C,
    query: BandwidthLatencyQuery,
) -> AppResult<BandwidthLatencyResponse>
where
    C: ConnectionTrait + StreamTrait + Send,
{
    let timeframe_str = query.timeframe.unwrap_or_else(|| "15m".into());
    let (total_seconds, bucket_count, bucket_duration) = match timeframe_str.to_lowercase().as_str()
    {
        "5m" => (300, 20, 15),
        "1h" => (3600, 30, 120),
        "24h" => (86400, 48, 1800),
        _ => (900, 30, 30), // 15m
    };

    let now = Utc::now();
    let start_time = now - Duration::seconds(total_seconds);

    // 1. Identifica monitores de ping relevantes
    let mut monitors_query = monitors::Entity::find();
    if let Some(target) = &query.ping_target {
        if target != "all" && !target.trim().is_empty() {
            if let Ok(mon_id) = target.parse::<i64>() {
                monitors_query = monitors_query.filter(monitors::Column::Id.eq(mon_id));
            } else {
                // Alvo informado como IP ou hostname
                monitors_query = monitors_query.filter(monitors::Column::Type.eq("ping"));
            }
        } else {
            monitors_query = monitors_query.filter(monitors::Column::Type.eq("ping"));
        }
    } else {
        monitors_query = monitors_query.filter(monitors::Column::Type.eq("ping"));
    }

    let ping_monitors = monitors_query.all(db).await?;
    let monitor_ids: Vec<i64> = if let Some(target) = &query.ping_target {
        if target != "all" && target.parse::<i64>().is_err() {
            // Filtra em memória por target matching caso tenha sido informado IP/Host
            ping_monitors
                .into_iter()
                .filter(|m| m.target().eq_ignore_ascii_case(target))
                .map(|m| m.id)
                .collect()
        } else {
            ping_monitors.into_iter().map(|m| m.id).collect()
        }
    } else {
        ping_monitors.into_iter().map(|m| m.id).collect()
    };

    let mut buckets: Vec<DualBucketAccumulator> = (0..bucket_count)
        .map(|_| DualBucketAccumulator::default())
        .collect();

    // 2. Percorre latências de ping sem materializar a janela.
    if !monitor_ids.is_empty() {
        let mut latency_rows = monitor_results::Entity::find()
            .filter(monitor_results::Column::MonitorId.is_in(monitor_ids))
            .filter(monitor_results::Column::StartedAt.gte(start_time))
            .filter(monitor_results::Column::StartedAt.lte(now))
            .stream(db)
            .await?;
        while let Some(row) = latency_rows.try_next().await? {
            let row_time: DateTime<Utc> = row.started_at.with_timezone(&Utc);
            let offset = (row_time - start_time).num_seconds();
            if offset >= 0 && offset < total_seconds {
                let bucket_idx = ((offset / bucket_duration) as usize).min(bucket_count - 1);
                if let Some(latency) = row
                    .latency_ms
                    .filter(|value| value.is_finite() && *value >= 0.0)
                {
                    buckets[bucket_idx].latency_sum += latency;
                    buckets[bucket_idx].latency_count += 1;
                }
            }
        }
    }

    // 3. Consulta métricas de banda
    let mut metrics_query = metrics_entity::Entity::find()
        .filter(metrics_entity::Column::Name.is_in(vec![
            "inBps",
            "outBps",
            "traffic",
            "interface_traffic",
        ]))
        .filter(metrics_entity::Column::RecordedAt.gte(start_time))
        .filter(metrics_entity::Column::RecordedAt.lte(now));

    if let Some(dev_str) = &query.device_id {
        if dev_str != "all" && !dev_str.trim().is_empty() {
            if let Ok(dev_id) = dev_str.parse::<i64>() {
                metrics_query = metrics_query.filter(metrics_entity::Column::DeviceId.eq(dev_id));
            }
        }
    }

    let mut metric_rows = metrics_query.stream(db).await?;
    while let Some(m) = metric_rows.try_next().await? {
        let m_time: DateTime<Utc> = m.recorded_at.with_timezone(&Utc);
        let offset = (m_time - start_time).num_seconds();
        if offset >= 0 && offset < total_seconds {
            let bucket_idx = ((offset / bucket_duration) as usize).min(bucket_count - 1);
            if m.value.is_finite() && m.value >= 0.0 {
                buckets[bucket_idx].bw_sum += m.value;
                buckets[bucket_idx].bw_count += 1;
            }
        }
    }

    // 7. Constrói a lista alinhada de amostras
    let mut samples: Vec<BandwidthLatencyPoint> = Vec::with_capacity(bucket_count);
    let mut bucket_latency_sum = 0.0;
    let mut buckets_with_latency = 0_i64;

    for (i, bucket) in buckets.into_iter().enumerate() {
        let bucket_start = start_time + Duration::seconds((i as i64) * bucket_duration);
        let bucket_mid = bucket_start + Duration::seconds(bucket_duration / 2);
        let ts_millis = bucket_mid.timestamp_millis();

        let time_str = if total_seconds <= 900 {
            bucket_mid.format("%H:%M:%S").to_string()
        } else {
            bucket_mid.format("%H:%M").to_string()
        };

        let bw_val = if bucket.bw_count > 0 {
            bucket.bw_sum / bucket.bw_count as f64
        } else {
            0.0
        };

        let lat_val = if bucket.latency_count > 0 {
            (bucket.latency_sum / bucket.latency_count as f64 * 10.0).round() / 10.0
        } else {
            0.0
        };

        if lat_val > 0.0 {
            bucket_latency_sum += lat_val;
            buckets_with_latency += 1;
        }

        samples.push(BandwidthLatencyPoint {
            time: time_str,
            timestamp: ts_millis,
            bw_bps: bw_val.round(),
            latency: lat_val,
        });
    }

    // 8. Métricas de correlação e resumo
    let current_bw = samples.last().map(|s| s.bw_bps).unwrap_or(0.0);
    let peak_bw = samples
        .iter()
        .map(|s| s.bw_bps)
        .reduce(f64::max)
        .unwrap_or(0.0);

    let current_latency = samples.last().map(|s| s.latency).unwrap_or(0.0);
    let avg_latency = if buckets_with_latency > 0 {
        (bucket_latency_sum / buckets_with_latency as f64 * 10.0).round() / 10.0
    } else {
        0.0
    };

    // Detecção de correlação de saturação
    let mut correlated_points = 0;
    if peak_bw > 0.0 && avg_latency > 0.0 {
        for s in &samples {
            if s.bw_bps >= peak_bw * 0.75 && s.latency >= avg_latency * 1.4 {
                correlated_points += 1;
            }
        }
    }

    let has_saturation_correlation = correlated_points >= 2;
    let correlation_score = if has_saturation_correlation {
        82
    } else if correlated_points == 1 {
        55
    } else if peak_bw > 0.0 && avg_latency > 0.0 {
        24
    } else {
        0
    };

    Ok(BandwidthLatencyResponse {
        timeframe: timeframe_str,
        samples,
        current_bw,
        peak_bw,
        current_latency,
        avg_latency,
        correlation_score,
        has_saturation_correlation,
    })
}
