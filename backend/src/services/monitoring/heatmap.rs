//! Cálculo e agregação da matriz de Heatmap Horário de Latência (§2.2.2).
//!
//! Agrupa os buckets horários de `monitor_results_hourly` e os resultados parciais
//! da hora aberta de `monitor_results` em uma grade de calor (Dias x Horas do dia).
//! Permite identificar visualmente variações diárias e horários de pico de lentidão.

use std::collections::HashMap;

use chrono::{Datelike, Duration, Timelike, Utc};
use futures::TryStreamExt;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, StreamTrait};

use crate::{
    dtos::saas::{
        HourOfDaySummary, HourlyHeatmapCell, HourlyHeatmapMonitorSummary, HourlyHeatmapQuery,
        HourlyHeatmapResponse,
    },
    models::{monitor_results, monitor_results_hourly, monitors},
    services::{monitoring::rollup::truncate_to_hour, shared::errors::AppResult},
};

#[derive(Debug, Default)]
struct HourlyAccumulator {
    total_checks: i64,
    up_checks: i64,
    down_checks: i64,
    latency_sum: f64,
    latency_count: i64,
    min_latency: Option<f64>,
    max_latency: Option<f64>,
}

impl HourlyAccumulator {
    fn add_hourly_bucket(&mut self, bucket: &monitor_results_hourly::Model) {
        self.total_checks += i64::from(bucket.total_checks);
        self.up_checks += i64::from(bucket.up_checks);
        self.down_checks += i64::from(bucket.down_checks);

        if let Some(avg) = bucket.avg_latency_ms {
            let checks_with_latency = i64::from(bucket.up_checks).max(1);
            self.latency_sum += avg * checks_with_latency as f64;
            self.latency_count += checks_with_latency;
        }
        if let Some(min) = bucket.min_latency_ms {
            self.min_latency = Some(self.min_latency.map_or(min, |cur| cur.min(min)));
        }
        if let Some(max) = bucket.max_latency_ms {
            self.max_latency = Some(self.max_latency.map_or(max, |cur| cur.max(max)));
        }
    }

    fn add_raw_result(&mut self, result: &monitor_results::Model) {
        self.total_checks += 1;
        match result.status.as_str() {
            "up" => self.up_checks += 1,
            "down" => self.down_checks += 1,
            _ => {}
        }
        if let Some(lat) = result.latency_ms {
            if lat.is_finite() {
                self.latency_sum += lat;
                self.latency_count += 1;
                self.min_latency = Some(self.min_latency.map_or(lat, |cur| cur.min(lat)));
                self.max_latency = Some(self.max_latency.map_or(lat, |cur| cur.max(lat)));
            }
        }
    }

    fn to_cell(&self, date: String, day_of_week: u32, hour: u32) -> HourlyHeatmapCell {
        let avg_latency = if self.latency_count > 0 {
            Some((self.latency_sum / self.latency_count as f64 * 10.0).round() / 10.0)
        } else {
            None
        };
        let uptime = if self.total_checks == 0 {
            100.0
        } else {
            let decisive = self.up_checks + self.down_checks;
            if decisive == 0 {
                100.0
            } else {
                (self.up_checks as f64 * 1000.0 / decisive as f64).round() / 10.0
            }
        };

        HourlyHeatmapCell {
            date,
            day_of_week,
            hour,
            avg_latency_ms: avg_latency,
            min_latency_ms: self.min_latency,
            max_latency_ms: self.max_latency,
            uptime_percentage: uptime,
            total_checks: self.total_checks,
            up_checks: self.up_checks,
            down_checks: self.down_checks,
        }
    }
}

/// Gera a matriz de calor horária de latência com agregação de picos e métricas de QoE.
pub async fn calculate_hourly_heatmap<C>(
    db: &C,
    query: HourlyHeatmapQuery,
) -> AppResult<HourlyHeatmapResponse>
where
    C: ConnectionTrait + StreamTrait + Send,
{
    let days_count = query.days.unwrap_or(7).clamp(1, 30);
    let now = Utc::now();
    let window_start = truncate_to_hour(now - Duration::days(days_count - 1));
    let current_hour_start = truncate_to_hour(now);

    // Seleciona os monitores elegíveis
    let mut monitors_query = monitors::Entity::find();

    if let Some(monitor_id) = query.monitor_id {
        monitors_query = monitors_query.filter(monitors::Column::Id.eq(monitor_id));
    }

    let all_monitors = monitors_query.all(db).await?;

    let eligible_monitors: Vec<_> = all_monitors
        .into_iter()
        .filter(|m| {
            if query.is_saas == Some(true) {
                m.configuration
                    .get("isSaas")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || m.configuration.get("saasService").is_some()
            } else {
                true
            }
        })
        .collect();

    let monitor_ids: Vec<i64> = eligible_monitors.iter().map(|m| m.id).collect();

    let monitor_summaries: Vec<HourlyHeatmapMonitorSummary> = eligible_monitors
        .iter()
        .map(|m| HourlyHeatmapMonitorSummary {
            id: m.id,
            name: m.name.clone(),
            target: m.target(),
            check_type: m.r#type.clone(),
            is_saas: m
                .configuration
                .get("isSaas")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            saas_service: m
                .configuration
                .get("saasService")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            current_status: m.status.clone(),
        })
        .collect();

    let start_date_str = window_start.format("%Y-%m-%d").to_string();
    let end_date_str = now.format("%Y-%m-%d").to_string();

    if monitor_ids.is_empty() {
        return Ok(HourlyHeatmapResponse {
            matrix: Vec::new(),
            by_hour_of_day: (0..24)
                .map(|h| HourOfDaySummary {
                    hour: h,
                    avg_latency_ms: None,
                    min_latency_ms: None,
                    max_latency_ms: None,
                    uptime_percentage: 100.0,
                    total_checks: 0,
                })
                .collect(),
            monitors: Vec::new(),
            overall_avg_latency_ms: None,
            peak_hour: None,
            best_hour: None,
            overall_uptime_percentage: 100.0,
            total_checks: 0,
            start_date: start_date_str,
            end_date: end_date_str,
        });
    }

    // Consulta buckets horários fechados
    let mut hourly_records = monitor_results_hourly::Entity::find()
        .filter(monitor_results_hourly::Column::MonitorId.is_in(monitor_ids.clone()))
        .filter(monitor_results_hourly::Column::Bucket.gte(window_start))
        .filter(monitor_results_hourly::Column::Bucket.lt(current_hour_start))
        .order_by_asc(monitor_results_hourly::Column::Bucket)
        .stream(db)
        .await?;

    // Mapa: (date_string, hour_u32) -> HourlyAccumulator
    let mut map: HashMap<(String, u32), HourlyAccumulator> = HashMap::new();

    while let Some(record) = hourly_records.try_next().await? {
        let dt = record.bucket.with_timezone(&Utc);
        let date_key = dt.format("%Y-%m-%d").to_string();
        let hour = dt.hour();
        let acc = map.entry((date_key, hour)).or_default();
        acc.add_hourly_bucket(&record);
    }
    drop(hourly_records);

    // Adiciona hora em andamento a partir de `monitor_results`
    let mut raw_recent = monitor_results::Entity::find()
        .filter(monitor_results::Column::MonitorId.is_in(monitor_ids))
        .filter(monitor_results::Column::StartedAt.gte(current_hour_start))
        .filter(monitor_results::Column::StartedAt.lte(now))
        .stream(db)
        .await?;

    while let Some(raw) = raw_recent.try_next().await? {
        let dt = raw.started_at.with_timezone(&Utc);
        let date_key = dt.format("%Y-%m-%d").to_string();
        let hour = dt.hour();
        let acc = map.entry((date_key, hour)).or_default();
        acc.add_raw_result(&raw);
    }

    // Constrói a grade completa ordenada por data e hora
    let mut matrix = Vec::new();
    let mut current_day = window_start.date_naive();
    let end_day = now.date_naive();

    while current_day <= end_day {
        let date_str = current_day.format("%Y-%m-%d").to_string();
        let day_of_week = current_day.weekday().num_days_from_sunday();

        for hour in 0..24 {
            let cell = if let Some(acc) = map.get(&(date_str.clone(), hour)) {
                acc.to_cell(date_str.clone(), day_of_week, hour)
            } else {
                HourlyHeatmapCell {
                    date: date_str.clone(),
                    day_of_week,
                    hour,
                    avg_latency_ms: None,
                    min_latency_ms: None,
                    max_latency_ms: None,
                    uptime_percentage: 100.0,
                    total_checks: 0,
                    up_checks: 0,
                    down_checks: 0,
                }
            };
            matrix.push(cell);
        }

        current_day += Duration::days(1);
    }

    // Calcula resumo por hora do dia (0h..23h)
    let mut by_hour_of_day = Vec::with_capacity(24);
    let mut total_latency_sum = 0.0;
    let mut total_latency_count = 0i64;
    let mut global_total_checks = 0i64;
    let mut global_up_checks = 0i64;
    let mut global_down_checks = 0i64;

    for h in 0..24 {
        let mut hour_sum = 0.0;
        let mut hour_count = 0i64;
        let mut hour_checks = 0i64;
        let mut hour_up = 0i64;
        let mut hour_down = 0i64;
        let mut min_lat: Option<f64> = None;
        let mut max_lat: Option<f64> = None;

        for cell in &matrix {
            if cell.hour == h && cell.total_checks > 0 {
                hour_checks += cell.total_checks;
                hour_up += cell.up_checks;
                hour_down += cell.down_checks;

                if let Some(avg) = cell.avg_latency_ms {
                    let w = cell.up_checks.max(1);
                    hour_sum += avg * w as f64;
                    hour_count += w;
                }
                if let Some(min) = cell.min_latency_ms {
                    min_lat = Some(min_lat.map_or(min, |cur| cur.min(min)));
                }
                if let Some(max) = cell.max_latency_ms {
                    max_lat = Some(max_lat.map_or(max, |cur| cur.max(max)));
                }
            }
        }

        global_total_checks += hour_checks;
        global_up_checks += hour_up;
        global_down_checks += hour_down;
        total_latency_sum += hour_sum;
        total_latency_count += hour_count;

        let avg = if hour_count > 0 {
            Some((hour_sum / hour_count as f64 * 10.0).round() / 10.0)
        } else {
            None
        };
        let uptime = if hour_checks == 0 {
            100.0
        } else {
            let decisive = hour_up + hour_down;
            if decisive == 0 {
                100.0
            } else {
                (hour_up as f64 * 1000.0 / decisive as f64).round() / 10.0
            }
        };

        by_hour_of_day.push(HourOfDaySummary {
            hour: h,
            avg_latency_ms: avg,
            min_latency_ms: min_lat,
            max_latency_ms: max_lat,
            uptime_percentage: uptime,
            total_checks: hour_checks,
        });
    }

    // Identifica peak_hour e best_hour
    let mut peak_hour = None;
    let mut highest_avg = f64::NEG_INFINITY;
    let mut best_hour = None;
    let mut lowest_avg = f64::INFINITY;

    for summary in &by_hour_of_day {
        if let Some(avg) = summary.avg_latency_ms {
            if avg > highest_avg {
                highest_avg = avg;
                peak_hour = Some(summary.hour);
            }
            if avg < lowest_avg {
                lowest_avg = avg;
                best_hour = Some(summary.hour);
            }
        }
    }

    let overall_avg = if total_latency_count > 0 {
        Some((total_latency_sum / total_latency_count as f64 * 10.0).round() / 10.0)
    } else {
        None
    };

    let overall_uptime = if global_total_checks == 0 {
        100.0
    } else {
        let decisive = global_up_checks + global_down_checks;
        if decisive == 0 {
            100.0
        } else {
            (global_up_checks as f64 * 1000.0 / decisive as f64).round() / 10.0
        }
    };

    Ok(HourlyHeatmapResponse {
        matrix,
        by_hour_of_day,
        monitors: monitor_summaries,
        overall_avg_latency_ms: overall_avg,
        peak_hour,
        best_hour,
        overall_uptime_percentage: overall_uptime,
        total_checks: global_total_checks,
        start_date: start_date_str,
        end_date: end_date_str,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acumulador_calcula_media_e_uptime_corretamente() {
        let acc = HourlyAccumulator {
            total_checks: 10,
            up_checks: 9,
            down_checks: 1,
            latency_sum: 180.0,
            latency_count: 9,
            min_latency: Some(15.0),
            max_latency: Some(25.0),
        };

        let cell = acc.to_cell("2026-08-22".into(), 6, 14);
        assert_eq!(cell.hour, 14);
        assert_eq!(cell.avg_latency_ms, Some(20.0));
        assert_eq!(cell.uptime_percentage, 90.0);
        assert_eq!(cell.min_latency_ms, Some(15.0));
        assert_eq!(cell.max_latency_ms, Some(25.0));
    }
}
