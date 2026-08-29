//! Serviço de agregação de séries temporais de latência e perda de pacotes.
//!
//! Agrupa checagens de `monitor_results` em baldes temporais proporcionais ao
//! `timeframe` selecionado (5m, 15m, 1h, 24h), gerando dados consistentes e
//! contínuos para os gráficos do Dashboard e detalhes de monitor.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::{
    dtos::monitors::{
        MonitorTimeSeriesDetailItem, MonitorTimeSeriesPoint, MonitorTimeSeriesQuery,
        MonitorTimeSeriesResponse,
    },
    models::{devices, monitor_results, monitors},
    services::shared::errors::AppResult,
};

#[derive(Debug, Default)]
struct BucketAccumulator {
    latencies: Vec<f64>,
    down_count: i64,
    total_checks: i64,
    monitor_items: HashMap<i64, MonitorTimeSeriesDetailItem>,
}

/// Calcula a série temporal de latência e perda de pacotes.
pub async fn calculate_monitor_timeseries<C>(
    db: &C,
    query: MonitorTimeSeriesQuery,
) -> AppResult<MonitorTimeSeriesResponse>
where
    C: ConnectionTrait,
{
    let timeframe_str = query.timeframe.unwrap_or_else(|| "15m".into());
    let (total_seconds, bucket_count, bucket_duration) = match timeframe_str.to_lowercase().as_str()
    {
        "5m" => (300, 20, 15),
        "1h" => (3600, 30, 120),
        "24h" => (86400, 48, 1800),
        _ => (900, 30, 30), // Padrão: 15m
    };

    let now = Utc::now();
    let start_time = now - Duration::seconds(total_seconds);

    // 1. Identifica monitores alvo
    let mut monitors_query = monitors::Entity::find();
    if let Some(id) = query.monitor_id {
        monitors_query = monitors_query.filter(monitors::Column::Id.eq(id));
    } else {
        let kind = query.monitor_type.unwrap_or_else(|| "ping".into());
        monitors_query = monitors_query
            .filter(monitors::Column::Type.eq(kind))
            .filter(monitors::Column::Enabled.eq(true));
    }

    let target_monitors = monitors_query.all(db).await?;
    if target_monitors.is_empty() {
        return Ok(MonitorTimeSeriesResponse {
            timeframe: timeframe_str,
            samples: Vec::new(),
            avg_latency: 0.0,
            max_latency: 0.0,
            min_latency: 0.0,
            packet_loss_pct: 0,
            total_checks: 0,
        });
    }

    let monitor_ids: Vec<i64> = target_monitors.iter().map(|m| m.id).collect();

    // 2. Mapa auxiliar de dispositivos
    let device_ids: Vec<i64> = target_monitors.iter().filter_map(|m| m.device_id).collect();
    let device_map: HashMap<i64, String> = if device_ids.is_empty() {
        HashMap::new()
    } else {
        devices::Entity::find()
            .filter(devices::Column::Id.is_in(device_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|d| (d.id, d.name))
            .collect()
    };

    let monitor_info_map: HashMap<i64, &monitors::Model> =
        target_monitors.iter().map(|m| (m.id, m)).collect();

    // 3. Consulta os resultados brutos no período
    let results = monitor_results::Entity::find()
        .filter(monitor_results::Column::MonitorId.is_in(monitor_ids))
        .filter(monitor_results::Column::StartedAt.gte(start_time))
        .order_by_asc(monitor_results::Column::StartedAt)
        .all(db)
        .await?;

    // 4. Inicializa baldes
    let mut buckets: Vec<BucketAccumulator> = (0..bucket_count)
        .map(|_| BucketAccumulator::default())
        .collect();

    let mut global_latencies: Vec<f64> = Vec::with_capacity(results.len());
    let mut global_down_count: i64 = 0;
    let total_checks_count = results.len() as i64;

    // 5. Distribui leituras nos baldes
    for res in &results {
        let res_time: DateTime<Utc> = res.started_at.with_timezone(&Utc);
        let offset = (res_time - start_time).num_seconds();

        let is_down = res.status == "down" || res.status == "offline";
        if is_down {
            global_down_count += 1;
        }

        if let Some(lat) = res.latency_ms {
            if lat.is_finite() {
                global_latencies.push(lat);
            }
        }

        if offset >= 0 && offset < total_seconds {
            let bucket_idx = ((offset / bucket_duration) as usize).min(bucket_count - 1);
            let bucket = &mut buckets[bucket_idx];
            bucket.total_checks += 1;

            if is_down {
                bucket.down_count += 1;
            }

            if let Some(lat) = res.latency_ms {
                if lat.is_finite() {
                    bucket.latencies.push(lat);
                }
            }

            if let Some(mon) = monitor_info_map.get(&res.monitor_id) {
                let dev_name = mon.device_id.and_then(|did| device_map.get(&did)).cloned();
                bucket
                    .monitor_items
                    .entry(mon.id)
                    .and_modify(|item| {
                        if let Some(lat) = res.latency_ms {
                            item.latency_ms = Some(lat);
                        }
                        item.status = res.status.clone();
                        item.loss_pct = if is_down { 100 } else { 0 };
                    })
                    .or_insert_with(|| MonitorTimeSeriesDetailItem {
                        id: mon.id,
                        name: mon.name.clone(),
                        target: mon.target(),
                        monitor_type: mon.r#type.clone(),
                        device_name: dev_name,
                        status: res.status.clone(),
                        latency_ms: res.latency_ms,
                        loss_pct: if is_down { 100 } else { 0 },
                    });
            }
        }
    }

    // 6. Constrói a lista de pontos serializáveis
    let mut samples: Vec<MonitorTimeSeriesPoint> = Vec::with_capacity(bucket_count);

    for (i, bucket) in buckets.into_iter().enumerate() {
        let bucket_start = start_time + Duration::seconds((i as i64) * bucket_duration);
        let bucket_mid = bucket_start + Duration::seconds(bucket_duration / 2);
        let ts_millis = bucket_mid.timestamp_millis();

        let time_str = if total_seconds <= 900 {
            bucket_mid.format("%H:%M:%S").to_string()
        } else {
            bucket_mid.format("%H:%M").to_string()
        };

        let avg_lat = if !bucket.latencies.is_empty() {
            let sum: f64 = bucket.latencies.iter().sum();
            (sum / bucket.latencies.len() as f64 * 10.0).round() / 10.0
        } else {
            0.0
        };

        let loss_pct = if bucket.total_checks > 0 {
            ((bucket.down_count as f64 / bucket.total_checks as f64) * 100.0).round() as i32
        } else {
            0
        };

        samples.push(MonitorTimeSeriesPoint {
            time: time_str,
            timestamp: ts_millis,
            latency: avg_lat,
            loss: loss_pct,
            monitors_detail: bucket.monitor_items.into_values().collect(),
        });
    }

    // 7. Estatísticas consolidadas
    let avg_latency = if !global_latencies.is_empty() {
        let sum: f64 = global_latencies.iter().sum();
        (sum / global_latencies.len() as f64 * 10.0).round() / 10.0
    } else {
        0.0
    };

    let max_latency = global_latencies
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(0.0);
    let min_latency = global_latencies
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0);

    let packet_loss_pct = if total_checks_count > 0 {
        ((global_down_count as f64 / total_checks_count as f64) * 100.0).round() as i32
    } else {
        0
    };

    Ok(MonitorTimeSeriesResponse {
        timeframe: timeframe_str,
        samples,
        avg_latency,
        max_latency,
        min_latency,
        packet_loss_pct,
        total_checks: total_checks_count,
    })
}
