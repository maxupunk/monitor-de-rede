//! Análises que o NetMonitor já calcula para as telas, entregues à IA em
//! formato compacto: causa raiz pela topologia, comparação com o normal
//! (baseline), padrão por hora do dia e linha do tempo de um incidente.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use super::{
    history::monitor_selector_schema,
    log_digest::collapse_repeats,
    lookup::{device_names, find_device, find_monitor},
    series::round2,
    timeline::{merge, status_transitions, EntryKind, TimelineEntry},
    AiToolHandler, ToolArgs, ToolOutput,
};
use crate::{
    dtos::{
        ai::{AiChart, AiChartAxis, AiChartPoint, AiChartSeries, AiChartUnit},
        saas::HourlyHeatmapQuery,
    },
    models::{_entities::alert_events, devices, monitor_results, monitors},
    services::{
        alerts::{
            baseline,
            correlation::{self, DependencyNodeSummary, ImpactedDeviceSummary},
        },
        monitoring::heatmap::calculate_hourly_heatmap,
        shared::errors::AppResult,
        syslog::{
            db::LogsDb,
            repository::{self, LogFilters, LogQuery},
        },
    },
};

/// Dispositivos impactados listados por nome; o resto vira contagem.
const MAX_IMPACTED: usize = 12;
const MAX_CLUSTERS: usize = 5;
const TIMELINE_CAP: usize = 40;
/// Logs lidos para a linha do tempo: aviso ou pior.
const TIMELINE_LOG_SEVERITY: i16 = 4;
const TIMELINE_LOG_ROWS: u64 = 300;

fn impacted_names(devices: &[ImpactedDeviceSummary]) -> Vec<&str> {
    devices
        .iter()
        .take(MAX_IMPACTED)
        .map(|device| device.name.as_str())
        .collect()
}

fn chain_names(chain: &[DependencyNodeSummary]) -> Vec<String> {
    chain
        .iter()
        .map(|node| {
            let mut label = format!("{} ({})", node.name, node.status);
            if node.is_root_cause {
                label.push_str(" [causa]");
            }
            label
        })
        .collect()
}

pub struct RootCause;

#[async_trait]
impl AiToolHandler for RootCause {
    fn name(&self) -> &'static str {
        "analyze_root_cause"
    }

    fn description(&self) -> &'static str {
        "Correlaciona alertas pela topologia (quem depende de quem) para achar a causa raiz: um switch ou gateway caído que derruba os equipamentos abaixo. Com alert_id analisa um alerta; sem, agrupa todos os incidentes abertos."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "alert_id": { "type": "integer", "description": "Id do alerta (de get_alerts). Omitido: todos os abertos." }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        if let Some(alert_id) = args.integer("alert_id") {
            if alert_events::Entity::find_by_id(alert_id)
                .one(&ctx.db)
                .await?
                .is_none()
            {
                return Ok(ToolOutput::not_found(format!(
                    "Alerta {alert_id} não encontrado; use get_alerts"
                )));
            }
            let analysis = correlation::analyze(&ctx.db, alert_id, None).await?;
            return Ok(ToolOutput::data(json!({
                "alert_id": alert_id,
                "primary_cause": analysis.primary_cause.as_ref().map(|cause| json!({
                    "alert_id": cause.id,
                    "title": cause.title,
                    "device": cause.device.as_ref().map(|device| &device.name),
                })),
                "category": analysis.causal_category_label,
                "confidence_pct": analysis.confidence,
                "explanation": analysis.explanation,
                "impacted_devices_count": analysis.impacted_devices_count,
                "impacted_devices": impacted_names(&analysis.impacted_devices),
                "dependency_chain": chain_names(&analysis.dependency_chain),
                "correlated_alerts": analysis.correlation_count,
            })));
        }

        let summary = correlation::analyze_active_clusters(&ctx.db, None).await?;
        let clusters: Vec<Value> = summary
            .active_clusters
            .iter()
            .take(MAX_CLUSTERS)
            .map(|cluster| {
                json!({
                    "root_cause_device": cluster.root_cause_device_name,
                    "root_cause_alert_id": cluster.root_cause_event.as_ref().map(|event| event.id),
                    "category": cluster.causal_category_label,
                    "confidence_pct": cluster.confidence,
                    "explanation": cluster.explanation,
                    "impacted_devices_count": cluster.impacted_devices_count,
                    "alerts": cluster.total_alerts_count,
                    "max_severity": cluster.max_severity,
                    "started_at": cluster.started_at,
                })
            })
            .collect();
        Ok(ToolOutput::data(json!({
            "open_incidents": summary.total_active_incidents,
            "correlated_alerts": summary.total_correlated_alerts,
            "clusters": clusters,
        })))
    }
}

pub struct BaselineComparison;

#[async_trait]
impl AiToolHandler for BaselineComparison {
    fn name(&self) -> &'static str {
        "compare_with_baseline"
    }

    fn description(&self) -> &'static str {
        "Compara a última checagem de um monitor com o comportamento normal dele (média e desvio dos últimos dias): diz se a latência ou a perda atual é anormal e quanto."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": monitor_selector_schema() })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let device = args.text("device");
        let kind = args.text("monitor_type");
        let monitor = match find_monitor(
            &ctx.db,
            args.integer("monitor_id"),
            device.as_deref(),
            kind.as_deref(),
        )
        .await?
        {
            Ok(monitor) => monitor,
            Err(message) => return Ok(ToolOutput::not_found(message)),
        };
        let snapshot = baseline::snapshot(&ctx.db, monitor.id).await?;
        let enriched = snapshot.enriched;
        if enriched.is_empty() {
            return Ok(ToolOutput::data(json!({
                "monitor": monitor.name,
                "has_baseline": false,
                "note": "Histórico insuficiente para calcular o normal deste monitor (precisa de algumas horas de checagens).",
            })));
        }
        let round = |value: Option<f64>| value.map(round2);
        Ok(ToolOutput::data(json!({
            "monitor": monitor.name,
            "has_baseline": true,
            "window_days": enriched.window_days,
            "current": {
                "status": snapshot.latest.as_ref().map(|result| &result.status),
                "latency_ms": round(snapshot.current.latency_ms),
            },
            "latency": {
                "normal_ms": round(enriched.latency_baseline_ms),
                "stddev_ms": round(enriched.latency_stddev_ms),
                "upper_band_ms": round(enriched.latency_upper_band_ms),
                "deviation_pct": round(enriched.latency_deviation_percent),
                "z_score": round(enriched.latency_z_score),
                "anomaly": enriched.is_latency_anomaly,
            },
            "packet_loss": {
                "normal_pct": round(enriched.packet_loss_baseline_percent),
                "anomaly": enriched.is_packet_loss_anomaly,
            },
            "uptime_normal_pct": round(enriched.uptime_baseline_percent),
        })))
    }
}

pub struct HourlyPattern;

#[async_trait]
impl AiToolHandler for HourlyPattern {
    fn name(&self) -> &'static str {
        "get_hourly_pattern"
    }

    fn description(&self) -> &'static str {
        "Padrão por hora do dia (UTC) nos últimos N dias: latência média e disponibilidade por hora, hora de pico e melhor hora. Exibe o gráfico. Responde 'em que horário costuma piorar/cair?'. Sem monitor, considera todos."
    }

    fn parameters(&self) -> Value {
        let mut properties = monitor_selector_schema();
        properties.insert(
            "days".into(),
            json!({ "type": "integer", "description": "Dias analisados (1 a 30, padrão 7)" }),
        );
        json!({ "type": "object", "properties": properties })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let monitor_id = args.integer("monitor_id");
        let device = args.text("device");
        let monitor = if monitor_id.is_some() || device.is_some() {
            let kind = args.text("monitor_type");
            match find_monitor(&ctx.db, monitor_id, device.as_deref(), kind.as_deref()).await? {
                Ok(monitor) => Some(monitor),
                Err(message) => return Ok(ToolOutput::not_found(message)),
            }
        } else {
            None
        };
        let days = args.integer_in("days", 7, 1, 30);
        let heatmap = calculate_hourly_heatmap(
            &ctx.db,
            HourlyHeatmapQuery {
                monitor_id: monitor.as_ref().map(|monitor| monitor.id),
                is_saas: None,
                days: Some(days),
            },
        )
        .await?;
        let hours: Vec<_> = heatmap
            .by_hour_of_day
            .iter()
            .filter(|hour| hour.total_checks > 0)
            .collect();
        if hours.is_empty() {
            return Ok(ToolOutput::not_found(format!(
                "Sem checagens nos últimos {days} dias para montar o padrão por hora"
            )));
        }

        let mut worst_uptime: Vec<_> = hours.clone();
        worst_uptime.sort_by(|a, b| a.uptime_percentage.total_cmp(&b.uptime_percentage));
        let scope = monitor
            .as_ref()
            .map_or_else(|| "todos os monitores".to_string(), |m| m.name.clone());

        let points: Vec<AiChartPoint> = hours
            .iter()
            .filter_map(|hour| {
                hour.avg_latency_ms.map(|value| AiChartPoint {
                    time: format!("{:02}h", hour.hour),
                    value: round2(value),
                })
            })
            .collect();
        let data = json!({
            "scope": scope,
            "days": days,
            "timezone": "UTC",
            "peak_latency_hour": heatmap.peak_hour,
            "best_hour": heatmap.best_hour,
            "overall_uptime_pct": round2(heatmap.overall_uptime_percentage),
            "overall_avg_latency_ms": heatmap.overall_avg_latency_ms.map(round2),
            "lowest_uptime_hours": worst_uptime.iter().take(3).map(|hour| json!({
                "hour": hour.hour,
                "uptime_pct": round2(hour.uptime_percentage),
                "checks": hour.total_checks,
            })).collect::<Vec<_>>(),
        });
        if points.is_empty() {
            return Ok(ToolOutput::data(data));
        }
        let chart = AiChart {
            title: format!("Latência por hora do dia — {scope}"),
            subtitle: Some(format!("média dos últimos {days} dias (UTC)")),
            unit: AiChartUnit::Latency,
            x_axis: AiChartAxis::Label,
            series: vec![AiChartSeries {
                id: "hourly-latency".into(),
                label: "Latência média".into(),
                points,
            }],
            avg_value: heatmap.overall_avg_latency_ms.map(round2),
        };
        let mut data = data;
        data["chart_shown"] = json!(true);
        Ok(ToolOutput::with_chart(data, chart))
    }
}

/// Janela da linha do tempo: em volta de `around`, ou os últimos minutos.
fn timeline_window(
    around: Option<DateTime<Utc>>,
    minutes: i64,
    now: DateTime<Utc>,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let span = Duration::minutes(minutes);
    match around {
        Some(center) => (center - span, (center + span).min(now)),
        None => (now - span, now),
    }
}

async fn alert_entries(
    ctx: &AppContext,
    device: Option<&devices::Model>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> AppResult<Vec<TimelineEntry>> {
    let mut query = alert_events::Entity::find().filter(
        Condition::any()
            .add(
                Condition::all()
                    .add(alert_events::Column::StartedAt.gte(from))
                    .add(alert_events::Column::StartedAt.lte(to)),
            )
            .add(
                Condition::all()
                    .add(alert_events::Column::ResolvedAt.gte(from))
                    .add(alert_events::Column::ResolvedAt.lte(to)),
            ),
    );
    if let Some(device) = device {
        query = query.filter(alert_events::Column::DeviceId.eq(device.id));
    }
    let events = query.all(&ctx.db).await?;
    let names = device_names(&ctx.db, events.iter().filter_map(|event| event.device_id)).await?;

    let mut entries = Vec::new();
    for event in events {
        let source = event
            .device_id
            .and_then(|id| names.get(&id).cloned())
            .unwrap_or_else(|| "sistema".into());
        let message = event.message.clone().unwrap_or_default();
        let started = event.started_at.with_timezone(&Utc);
        if started >= from && started <= to {
            entries.push(TimelineEntry {
                at: started,
                kind: EntryKind::AlertOpened,
                source: source.clone(),
                detail: format!("[{}] #{} {message}", event.severity, event.id),
                rank: 0,
            });
        }
        if let Some(resolved) = event.resolved_at.map(|at| at.with_timezone(&Utc)) {
            if resolved >= from && resolved <= to {
                entries.push(TimelineEntry {
                    at: resolved,
                    kind: EntryKind::AlertResolved,
                    source,
                    detail: format!("#{} resolvido", event.id),
                    rank: 0,
                });
            }
        }
    }
    Ok(entries)
}

async fn monitor_entries(
    ctx: &AppContext,
    device: &devices::Model,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> AppResult<Vec<TimelineEntry>> {
    let device_monitors = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .all(&ctx.db)
        .await?;
    let mut entries = Vec::new();
    for monitor in device_monitors {
        let results = monitor_results::Entity::find()
            .filter(monitor_results::Column::MonitorId.eq(monitor.id))
            .filter(monitor_results::Column::StartedAt.gte(from))
            .filter(monitor_results::Column::StartedAt.lte(to))
            .order_by_asc(monitor_results::Column::StartedAt)
            .all(&ctx.db)
            .await?;
        entries.extend(status_transitions(&monitor.name, &results));
    }
    Ok(entries)
}

async fn log_entries(
    ctx: &AppContext,
    device: Option<&devices::Model>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> AppResult<Vec<TimelineEntry>> {
    let Ok(logs) = LogsDb::from_context(ctx) else {
        return Ok(Vec::new());
    };
    let query = LogQuery::normalize(
        LogFilters {
            device_id: device.map(|device| device.id),
            severity: Some(TIMELINE_LOG_SEVERITY),
            from: Some(from),
            to: Some(to),
            ..LogFilters::default()
        },
        Utc::now(),
    );
    let rows = repository::export(logs.connection(), &query, TIMELINE_LOG_ROWS).await?;
    let names = device_names(&ctx.db, rows.iter().filter_map(|row| row.device_id)).await?;
    let lines = collapse_repeats(&rows, |row| {
        row.device_id
            .and_then(|id| names.get(&id).cloned())
            .or_else(|| row.hostname.clone())
            .unwrap_or_else(|| row.source_ip.clone())
    });
    Ok(lines
        .into_iter()
        .map(|line| TimelineEntry {
            at: line.at,
            kind: EntryKind::Log,
            source: line.device,
            detail: if line.repeated > 1 {
                format!("[{}] {} (×{})", line.severity, line.message, line.repeated)
            } else {
                format!("[{}] {}", line.severity, line.message)
            },
            rank: line.severity_value.unwrap_or(i16::MAX),
        })
        .collect())
}

pub struct IncidentTimeline;

#[async_trait]
impl AiToolHandler for IncidentTimeline {
    fn name(&self) -> &'static str {
        "get_incident_timeline"
    }

    fn description(&self) -> &'static str {
        "Linha do tempo de um incidente numa janela: alertas abertos/resolvidos, mudanças de status dos monitores do dispositivo e logs de aviso ou pior, em ordem cronológica. Responde 'o que aconteceu às 14h?'."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Nome, IP ou id do dispositivo (omitido: todos; sem mudanças de monitor)" },
                "around": { "type": "string", "description": "Instante central em RFC 3339 (ex: 2026-09-21T14:00:00Z). Omitido: agora." },
                "minutes": { "type": "integer", "description": "Minutos antes e depois do instante (5 a 1440, padrão 60)" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let device = match args.text("device") {
            Some(identifier) => match find_device(&ctx.db, &identifier).await? {
                Some(device) => Some(device),
                None => {
                    return Ok(ToolOutput::not_found(format!(
                        "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
                    )))
                }
            },
            None => None,
        };
        let around = match args.text("around") {
            Some(raw) => match DateTime::parse_from_rfc3339(&raw) {
                Ok(at) => Some(at.with_timezone(&Utc)),
                Err(_) => {
                    return Ok(ToolOutput::not_found(format!(
                        "Instante '{raw}' inválido; use RFC 3339 (ex: 2026-09-21T14:00:00Z)"
                    )))
                }
            },
            None => None,
        };
        let minutes = args.integer_in("minutes", 60, 5, 1440);
        let (from, to) = timeline_window(around, minutes, Utc::now());

        let mut entries = alert_entries(ctx, device.as_ref(), from, to).await?;
        if let Some(device) = device.as_ref() {
            entries.extend(monitor_entries(ctx, device, from, to).await?);
        }
        entries.extend(log_entries(ctx, device.as_ref(), from, to).await?);
        let (timeline, omitted) = merge(entries, TIMELINE_CAP);

        Ok(ToolOutput::data(json!({
            "device": device.as_ref().map(|device| &device.name),
            "from": from.to_rfc3339(),
            "to": to.to_rfc3339(),
            "events": timeline,
            "omitted_low_priority_logs": omitted,
        })))
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn janela_em_volta_do_instante_nao_passa_de_agora() {
        let now = Utc.with_ymd_and_hms(2026, 9, 21, 15, 0, 0).unwrap();
        let center = Utc.with_ymd_and_hms(2026, 9, 21, 14, 30, 0).unwrap();
        let (from, to) = timeline_window(Some(center), 60, now);
        assert_eq!(from, center - Duration::minutes(60));
        assert_eq!(to, now);

        let (from, to) = timeline_window(None, 30, now);
        assert_eq!(from, now - Duration::minutes(30));
        assert_eq!(to, now);
    }
}
