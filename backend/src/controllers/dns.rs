//! Endpoints de benchmark, lookup avulso e histórico de DNS.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use futures::TryStreamExt;
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Serialize;

use crate::{
    dtos::resources::{
        DnsBatchProvisionInput, DnsBenchmarkInput, DnsLookupInput, DnsPingProvisionInput,
    },
    models::{dns_servers, monitor_results, monitors},
    services::{
        monitoring::{
            execution_guard::{calculate_smart_timeout_seconds, effective_timeout_seconds},
            presenter::{present_monitors, RECENT_RESULTS_LIMIT},
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        network_tools::dns::{
            latency::{
                benchmark_dns_servers, measure_dns_lookup, parse_server_address, sort_by_latency,
                DnsBenchmarkOptions, DnsLookupOptions, DnsProtocol, DnsServerRanking,
                DnsServerTarget, DEFAULT_BENCHMARK_HOSTNAMES, DEFAULT_DNS_SERVERS,
            },
            registry::DnsServerRegistry,
            wire,
        },
        shared::errors::{AppError, AppResult},
    },
};

async fn benchmark(
    State(ctx): State<AppContext>,
    Json(input): Json<DnsBenchmarkInput>,
) -> AppResult<Response> {
    let hostnames = input.hostnames.unwrap_or_else(|| {
        DEFAULT_BENCHMARK_HOSTNAMES
            .iter()
            .map(ToString::to_string)
            .collect()
    });
    if hostnames.is_empty()
        || hostnames.len() > 10
        || hostnames
            .iter()
            .any(|hostname| hostname.trim().is_empty() || hostname.len() > 253)
    {
        return Err(AppError::validation(
            "Informe entre 1 e 10 hostnames DNS válidos",
        ));
    }
    let timeout_ms = input.timeout_ms.unwrap_or(3_000);
    if !(200..=15_000).contains(&timeout_ms) {
        return Err(AppError::validation(
            "timeoutMs deve estar entre 200 e 15000",
        ));
    }
    let rounds = input.rounds.unwrap_or(1);
    if !(1..=5).contains(&rounds) {
        return Err(AppError::validation("rounds deve estar entre 1 e 5"));
    }
    let record_type = wire::parse_record_type(input.record_type.as_deref())?;
    let servers = if let Some(servers) = input.servers {
        if servers.is_empty() || servers.len() > 12 {
            return Err(AppError::validation("Informe entre 1 e 12 servidores DNS"));
        }
        servers
            .into_iter()
            .map(|server| {
                Ok(DnsServerTarget {
                    server: server.server.trim().to_string(),
                    label: server.label,
                    protocol: DnsProtocol::parse(server.protocol.as_deref())?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?
    } else {
        let registered = DnsServerRegistry::benchmark_targets(&ctx.db).await?;
        if registered.is_empty() {
            DEFAULT_DNS_SERVERS
                .iter()
                .map(|(label, server)| DnsServerTarget {
                    server: (*server).into(),
                    label: Some((*label).into()),
                    protocol: DnsProtocol::Udp,
                })
                .collect()
        } else {
            registered
        }
    };
    let ranking = benchmark_dns_servers(DnsBenchmarkOptions {
        servers,
        hostnames: hostnames.clone(),
        record_type,
        timeout_ms,
        rounds,
    })
    .await;
    Ok(format::json(
        serde_json::json!({ "hostnames": hostnames, "recordType": wire::record_type_name(record_type), "measuredAt": Utc::now().to_rfc3339(), "ranking": ranking }),
    )?)
}

async fn lookup(Json(input): Json<DnsLookupInput>) -> AppResult<Response> {
    if input.hostname.trim().is_empty() || input.hostname.len() > 253 {
        return Err(AppError::validation("Informe um hostname DNS válido"));
    }
    let timeout_ms = input.timeout_ms.unwrap_or(3_000);
    if !(200..=15_000).contains(&timeout_ms) {
        return Err(AppError::validation(
            "timeoutMs deve estar entre 200 e 15000",
        ));
    }
    let protocol = DnsProtocol::parse(input.protocol.as_deref())?;
    let sample = measure_dns_lookup(DnsLookupOptions {
        hostname: input.hostname.trim().into(),
        server: input.server,
        protocol,
        doh_url: input.doh_url,
        record_type: wire::parse_record_type(input.record_type.as_deref())?,
        timeout_ms,
    })
    .await;
    Ok(format::json(sample)?)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsHistoryPoint {
    pub timestamp: String,
    pub latency_ms: Option<f64>,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsSeriesItem {
    pub server: String,
    pub label: String,
    pub protocol: String,
    pub monitor_ids: Vec<i64>,
    pub points: Vec<DnsHistoryPoint>,
}

const DNS_HISTORY_POINT_LIMIT: usize = 720;
const DNS_MEDIAN_SAMPLE_LIMIT: usize = 2_048;

#[derive(Default)]
struct PerformanceBucket {
    server: String,
    label: String,
    protocol: String,
    monitor_ids: Vec<i64>,
    latency_sample: Vec<f64>,
    latency_sample_stride: usize,
    latency_seen: usize,
    latency_sum: f64,
    min_latency: Option<f64>,
    max_latency: Option<f64>,
    total: usize,
    last_checked_at: Option<String>,
    points: Vec<DnsHistoryPoint>,
    point_stride: usize,
    points_seen: usize,
    last_point: Option<DnsHistoryPoint>,
}

impl PerformanceBucket {
    fn add_latency(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        self.latency_seen += 1;
        self.latency_sum += value;
        self.min_latency = Some(self.min_latency.map_or(value, |current| current.min(value)));
        self.max_latency = Some(self.max_latency.map_or(value, |current| current.max(value)));

        let stride = self.latency_sample_stride.max(1);
        self.latency_sample_stride = stride;
        if (self.latency_seen - 1).is_multiple_of(stride) {
            self.latency_sample.push(value);
        }
        if self.latency_sample.len() > DNS_MEDIAN_SAMPLE_LIMIT {
            retain_alternating(&mut self.latency_sample);
            self.latency_sample_stride = stride.saturating_mul(2);
        }
    }

    fn add_point(&mut self, point: DnsHistoryPoint) {
        self.points_seen += 1;
        self.last_point = Some(point.clone());
        let stride = self.point_stride.max(1);
        self.point_stride = stride;
        if (self.points_seen - 1).is_multiple_of(stride) {
            self.points.push(point);
        }
        if self.points.len() > DNS_HISTORY_POINT_LIMIT {
            retain_alternating(&mut self.points);
            self.point_stride = stride.saturating_mul(2);
        }
    }

    fn finish_points(&mut self) -> Vec<DnsHistoryPoint> {
        if let Some(last) = self.last_point.take() {
            let already_present = self
                .points
                .last()
                .is_some_and(|point| point.timestamp == last.timestamp);
            if !already_present {
                if self.points.len() >= DNS_HISTORY_POINT_LIMIT {
                    self.points.pop();
                }
                self.points.push(last);
            }
        }
        std::mem::take(&mut self.points)
    }
}

fn retain_alternating<T>(items: &mut Vec<T>) {
    let mut index = 0;
    items.retain(|_| {
        let retain = index % 2 == 0;
        index += 1;
        retain
    });
}

async fn performance(
    State(ctx): State<AppContext>,
    Query(query): Query<BTreeMap<String, String>>,
) -> AppResult<Response> {
    let hours = query
        .get("hours")
        .and_then(|raw| raw.parse::<i64>().ok())
        .unwrap_or(24)
        .clamp(1, 168);
    let dns_monitors = monitors::Entity::find()
        .filter(monitors::Column::Type.eq("dns"))
        .all(&ctx.db)
        .await?;
    if dns_monitors.is_empty() {
        return Ok(format::json(
            serde_json::json!({ "windowHours": hours, "monitorCount": 0, "ranking": [], "series": [] }),
        )?);
    }
    let cutoff = Utc::now() - Duration::hours(hours);
    let monitor_ids: Vec<i64> = dns_monitors.iter().map(|monitor| monitor.id).collect();
    let monitor_by_id: BTreeMap<i64, _> = dns_monitors
        .iter()
        .map(|monitor| (monitor.id, monitor))
        .collect();
    let mut results = monitor_results::Entity::find()
        .filter(monitor_results::Column::MonitorId.is_in(monitor_ids))
        .filter(monitor_results::Column::StartedAt.gte(cutoff))
        .filter(monitor_results::Column::StartedAt.lte(Utc::now()))
        .order_by_asc(monitor_results::Column::StartedAt)
        .stream(&ctx.db)
        .await?;
    let mut buckets: BTreeMap<String, PerformanceBucket> = BTreeMap::new();
    while let Some(result) = results.try_next().await? {
        let Some(monitor) = monitor_by_id.get(&result.monitor_id) else {
            continue;
        };
        let data = result.data.as_ref().and_then(serde_json::Value::as_object);
        let configuration = monitor.configuration.as_object();
        let protocol = data
            .and_then(|value| value.get("protocol"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                configuration
                    .and_then(|value| value.get("protocol"))
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or_else(|| {
                if configuration
                    .and_then(|value| value.get("dnsServer"))
                    .is_some()
                {
                    "udp"
                } else {
                    "system"
                }
            });
        let server = data
            .and_then(|value| value.get("server"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                configuration
                    .and_then(|value| value.get("dohUrl"))
                    .and_then(serde_json::Value::as_str)
            })
            .or_else(|| {
                configuration
                    .and_then(|value| value.get("dnsServer"))
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or("Resolvedor do sistema");
        let label = if !monitor.name.trim().is_empty() {
            monitor.name.clone()
        } else {
            server.to_string()
        };
        let key = format!("{server}|{protocol}");
        let bucket = buckets.entry(key).or_insert_with(|| PerformanceBucket {
            server: server.into(),
            label: label.clone(),
            protocol: protocol.into(),
            ..Default::default()
        });
        bucket.total += 1;
        if !bucket.monitor_ids.contains(&monitor.id) {
            bucket.monitor_ids.push(monitor.id);
        }
        let checked_at = result.finished_at.to_rfc3339();
        if bucket
            .last_checked_at
            .as_ref()
            .is_none_or(|previous| previous < &checked_at)
        {
            bucket.last_checked_at = Some(checked_at.clone());
        }
        let lookup = data
            .and_then(|value| value.get("avgLookupTimeMs"))
            .and_then(serde_json::Value::as_f64)
            .or(result.latency_ms);
        if result.status == "up" {
            if let Some(value) = lookup {
                bucket.add_latency(value);
            }
        }
        bucket.add_point(DnsHistoryPoint {
            timestamp: checked_at,
            latency_ms: if result.status == "up" { lookup } else { None },
            status: result.status.clone(),
        });
    }
    let mut ranking: Vec<DnsServerRanking> = Vec::new();
    let mut series: Vec<DnsSeriesItem> = Vec::new();

    for mut bucket in buckets.into_values() {
        let mut values = std::mem::take(&mut bucket.latency_sample);
        values.sort_by(f64::total_cmp);
        let count = bucket.latency_seen;
        let sampled_count = values.len();
        ranking.push(DnsServerRanking {
            server: bucket.server.clone(),
            label: if bucket.label.is_empty() {
                bucket.server.clone()
            } else {
                bucket.label.clone()
            },
            protocol: bucket.protocol.clone(),
            avg_lookup_time_ms: (count > 0).then(|| bucket.latency_sum / count as f64),
            min_lookup_time_ms: bucket.min_latency,
            max_lookup_time_ms: bucket.max_latency,
            median_lookup_time_ms: (sampled_count > 0).then(|| {
                if sampled_count % 2 == 0 {
                    (values[sampled_count / 2 - 1] + values[sampled_count / 2]) / 2.0
                } else {
                    values[sampled_count / 2]
                }
            }),
            success_rate: if bucket.total == 0 {
                0.0
            } else {
                (count as f64 / bucket.total as f64 * 1000.0).round() / 10.0
            },
            total_queries: bucket.total,
            failed_queries: bucket.total - count,
            error: None,
        });

        let points = bucket.finish_points();
        series.push(DnsSeriesItem {
            server: bucket.server,
            label: bucket.label,
            protocol: bucket.protocol,
            monitor_ids: bucket.monitor_ids,
            points,
        });
    }

    Ok(format::json(serde_json::json!({
        "windowHours": hours,
        "monitorCount": dns_monitors.len(),
        "ranking": sort_by_latency(ranking),
        "series": series,
    }))?)
}

async fn provision(
    State(ctx): State<AppContext>,
    Json(input): Json<DnsBatchProvisionInput>,
) -> AppResult<Response> {
    if input.servers.is_empty() || input.servers.len() > 30 {
        return Err(AppError::validation(
            "Informe entre 1 e 30 servidores DNS para provisionamento",
        ));
    }
    let domain = input
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("google.com");
    if domain.len() > 253 {
        return Err(AppError::validation("Domínio alvo inválido"));
    }
    let record_type = wire::parse_record_type(input.record_type.as_deref())?;
    let record_type_name = wire::record_type_name(record_type);
    let interval_seconds = input.interval_seconds.unwrap_or(60).max(1);
    let execute_now = input.execute_now.unwrap_or(true);

    let mut all_domains = vec![domain.to_string()];
    if let Some(extras) = input.domains {
        for extra in extras {
            let trimmed = extra.trim();
            if !trimmed.is_empty()
                && trimmed != domain
                && trimmed.len() <= 253
                && !all_domains.contains(&trimmed.to_string())
            {
                all_domains.push(trimmed.to_string());
            }
        }
    }
    all_domains.truncate(10);

    let existing_dns_monitors = monitors::Entity::find()
        .filter(monitors::Column::Type.eq("dns"))
        .all(&ctx.db)
        .await?;

    let mut created_rows = Vec::new();
    let mut already_monitored_count = 0;

    for item in input.servers {
        let server_addr = item.server.trim();
        if server_addr.is_empty() {
            continue;
        }
        let protocol = DnsProtocol::parse(item.protocol.as_deref())?;
        let protocol_str = protocol.as_str();

        let already_exists = existing_dns_monitors.iter().any(|m| {
            let config = m.configuration.as_object();
            let m_protocol = config
                .and_then(|c| c.get("protocol"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("udp");
            let m_server = config
                .and_then(|c| {
                    if m_protocol == "doh" {
                        c.get("dohUrl")
                    } else {
                        c.get("dnsServer")
                    }
                })
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            m_protocol == protocol_str
                && (m_server == server_addr
                    || (protocol_str == "doh"
                        && item.doh_url.as_deref().map(str::trim) == Some(m_server)))
        });

        if already_exists {
            already_monitored_count += 1;
            continue;
        }

        let name = item
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("DNS {server_addr}"));

        let mut config = serde_json::json!({
            "domain": domain,
            "recordType": record_type_name,
            "protocol": protocol_str,
        });
        if all_domains.len() > 1 {
            config["domains"] = serde_json::json!(all_domains);
        }
        if protocol_str == "doh" {
            let doh = item
                .doh_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(server_addr);
            config["dohUrl"] = serde_json::json!(doh);
        } else if protocol_str != "system" {
            config["dnsServer"] = serde_json::json!(server_addr);
        }

        let timeout_seconds = calculate_smart_timeout_seconds("dns", interval_seconds)
            .min((interval_seconds - 1).max(1))
            .max(1);

        let active_model = monitors::ActiveModel {
            device_id: Set(None),
            probe_id: Set(None),
            r#type: Set("dns".into()),
            name: Set(name.clone()),
            configuration: Set(config),
            interval_seconds: Set(interval_seconds),
            timeout_seconds: Set(timeout_seconds),
            retry_count: Set(3),
            enabled: Set(true),
            status: Set("unknown".into()),
            ..Default::default()
        };

        let inserted = active_model.insert(&ctx.db).await?;

        if execute_now {
            let timeout_ms =
                (effective_timeout_seconds(inserted.timeout_seconds, inserted.interval_seconds)
                    as u64)
                    * 1000;
            if let Ok(result) = run_monitor(
                &ctx,
                &inserted.r#type,
                &inserted.configuration,
                RunOptions {
                    timeout_ms: Some(timeout_ms),
                },
            )
            .await
            {
                let _ = process_result(&ctx, inserted.id, &result, inserted.probe_id).await;
            }
        }

        created_rows.push(inserted);

        if input.include_ping.unwrap_or(false) && protocol_str != "doh" {
            let ping_host = if let Ok((host, _)) = parse_server_address(server_addr) {
                host
            } else {
                server_addr.to_string()
            };

            let existing_ping = monitors::Entity::find()
                .filter(monitors::Column::Type.eq("ping"))
                .all(&ctx.db)
                .await?;

            let already_has_ping = existing_ping.iter().any(|m| {
                m.target().eq_ignore_ascii_case(&ping_host)
                    || m.configuration
                        .get("host")
                        .and_then(serde_json::Value::as_str)
                        .map(|h| h.eq_ignore_ascii_case(&ping_host))
                        .unwrap_or(false)
            });

            if !already_has_ping {
                let ping_name = format!("Ping {name}");
                let ping_model = monitors::ActiveModel {
                    device_id: Set(None),
                    probe_id: Set(None),
                    r#type: Set("ping".into()),
                    name: Set(ping_name),
                    configuration: Set(serde_json::json!({ "host": ping_host })),
                    interval_seconds: Set(interval_seconds),
                    timeout_seconds: Set(5),
                    retry_count: Set(3),
                    enabled: Set(true),
                    status: Set("unknown".into()),
                    ..Default::default()
                };
                if let Ok(inserted_ping) = ping_model.insert(&ctx.db).await {
                    if execute_now {
                        let timeout_ms = (effective_timeout_seconds(
                            inserted_ping.timeout_seconds,
                            inserted_ping.interval_seconds,
                        ) as u64)
                            * 1000;
                        if let Ok(result) = run_monitor(
                            &ctx,
                            &inserted_ping.r#type,
                            &inserted_ping.configuration,
                            RunOptions {
                                timeout_ms: Some(timeout_ms),
                            },
                        )
                        .await
                        {
                            let _ = process_result(
                                &ctx,
                                inserted_ping.id,
                                &result,
                                inserted_ping.probe_id,
                            )
                            .await;
                        }
                    }
                    created_rows.push(inserted_ping);
                }
            }
        }
    }

    let count = created_rows.len();
    let monitors_presentation =
        present_monitors(&ctx.db, created_rows, RECENT_RESULTS_LIMIT).await?;

    Ok(format::json(serde_json::json!({
        "createdCount": count,
        "alreadyMonitoredCount": already_monitored_count,
        "totalRequested": count + already_monitored_count,
        "monitors": monitors_presentation,
    }))?)
}

async fn provision_ping(
    State(ctx): State<AppContext>,
    Json(input): Json<DnsPingProvisionInput>,
) -> AppResult<Response> {
    let interval_seconds = input.interval_seconds.unwrap_or(60).clamp(10, 3600);
    let execute_now = input.execute_now.unwrap_or(true);

    let servers: Vec<(String, String)> = if let Some(srv_list) = input.servers {
        if srv_list.is_empty() {
            let db_servers = dns_servers::Entity::find().all(&ctx.db).await?;
            if db_servers.is_empty() {
                DEFAULT_DNS_SERVERS
                    .iter()
                    .map(|(label, addr)| (label.to_string(), addr.to_string()))
                    .collect()
            } else {
                db_servers
                    .into_iter()
                    .map(|s| (s.name, s.address))
                    .collect()
            }
        } else {
            srv_list
                .into_iter()
                .map(|s| {
                    let name = s.name.unwrap_or_else(|| s.server.clone());
                    (name, s.server)
                })
                .collect()
        }
    } else {
        let db_servers = dns_servers::Entity::find().all(&ctx.db).await?;
        if db_servers.is_empty() {
            DEFAULT_DNS_SERVERS
                .iter()
                .map(|(label, addr)| (label.to_string(), addr.to_string()))
                .collect()
        } else {
            db_servers
                .into_iter()
                .map(|s| (s.name, s.address))
                .collect()
        }
    };

    let existing_ping = monitors::Entity::find()
        .filter(monitors::Column::Type.eq("ping"))
        .all(&ctx.db)
        .await?;

    let mut created_rows = Vec::new();
    let mut already_monitored_count = 0;

    for (name, addr) in servers {
        let raw_addr = addr.trim();
        if raw_addr.is_empty() || raw_addr.starts_with("https://") {
            continue;
        }
        let ping_host = if let Ok((host, _)) = parse_server_address(raw_addr) {
            host
        } else {
            raw_addr.to_string()
        };

        let already_exists = existing_ping.iter().any(|m| {
            m.target().eq_ignore_ascii_case(&ping_host)
                || m.configuration
                    .get("host")
                    .and_then(serde_json::Value::as_str)
                    .map(|h| h.eq_ignore_ascii_case(&ping_host))
                    .unwrap_or(false)
        }) || created_rows
            .iter()
            .any(|m: &monitors::Model| m.target().eq_ignore_ascii_case(&ping_host));

        if already_exists {
            already_monitored_count += 1;
            continue;
        }

        let monitor_name = if name.to_lowercase().starts_with("ping") {
            name
        } else {
            format!("Ping DNS {name} ({ping_host})")
        };

        let active_model = monitors::ActiveModel {
            device_id: Set(None),
            probe_id: Set(None),
            r#type: Set("ping".into()),
            name: Set(monitor_name),
            configuration: Set(serde_json::json!({ "host": ping_host })),
            interval_seconds: Set(interval_seconds),
            timeout_seconds: Set(5),
            retry_count: Set(3),
            enabled: Set(true),
            status: Set("unknown".into()),
            ..Default::default()
        };

        let inserted = active_model.insert(&ctx.db).await?;

        if execute_now {
            let timeout_ms =
                (effective_timeout_seconds(inserted.timeout_seconds, inserted.interval_seconds)
                    as u64)
                    * 1000;
            if let Ok(result) = run_monitor(
                &ctx,
                &inserted.r#type,
                &inserted.configuration,
                RunOptions {
                    timeout_ms: Some(timeout_ms),
                },
            )
            .await
            {
                let _ = process_result(&ctx, inserted.id, &result, inserted.probe_id).await;
            }
        }

        created_rows.push(inserted);
    }

    let count = created_rows.len();
    let monitors_presentation =
        present_monitors(&ctx.db, created_rows, RECENT_RESULTS_LIMIT).await?;

    Ok(format::json(serde_json::json!({
        "createdCount": count,
        "alreadyMonitoredCount": already_monitored_count,
        "totalRequested": count + already_monitored_count,
        "monitors": monitors_presentation,
    }))?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/dns")
        .add("/benchmark", post(benchmark))
        .add("/lookup", post(lookup))
        .add("/performance", get(performance))
        .add("/provision", post(provision))
        .add("/provision-ping", post(provision_ping))
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn historico_dns_e_amostra_da_mediana_permanecem_limitados() {
        let mut bucket = PerformanceBucket::default();
        for index in 0..10_000 {
            bucket.add_latency(index as f64);
            bucket.add_point(DnsHistoryPoint {
                timestamp: index.to_string(),
                latency_ms: Some(index as f64),
                status: "up".into(),
            });
        }

        assert_eq!(bucket.latency_seen, 10_000);
        assert_eq!(bucket.latency_sum, 49_995_000.0);
        assert_eq!(bucket.min_latency, Some(0.0));
        assert_eq!(bucket.max_latency, Some(9_999.0));
        assert!(bucket.latency_sample.len() <= DNS_MEDIAN_SAMPLE_LIMIT);

        let points = bucket.finish_points();
        assert!(points.len() <= DNS_HISTORY_POINT_LIMIT);
        assert_eq!(points.first().unwrap().timestamp, "0");
        assert_eq!(points.last().unwrap().timestamp, "9999");
    }
}
