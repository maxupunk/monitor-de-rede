//! Gráficos das séries do banco desenhados no chat.
//!
//! As fontes são as mesmas das telas — `calculate_monitor_timeseries` (gráfico
//! de latência do monitor) e a tabela `metrics` (tráfego de interface, CPU e
//! memória) — e o frontend usa o mesmo `BaseMetricChart`. A IA recebe só as
//! estatísticas; os pontos vão para a tela em [`ToolOutput::chart`].

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use super::{
    history::{metric_samples, monitor_selector_schema},
    lookup::{device_interfaces_of, find_device, find_monitor, pick_interface},
    series::{chart_points, round2, stats, Sample},
    AiToolHandler, ToolArgs, ToolGroup, ToolOutput,
};
use crate::{
    dtos::{
        ai::{AiChart, AiChartAxis, AiChartSeries, AiChartUnit},
        monitors::MonitorTimeSeriesQuery,
    },
    models::_entities::{monitor_results_hourly, monitors},
    services::{monitoring::timeseries::calculate_monitor_timeseries, shared::errors::AppResult},
};

/// Janela do gráfico de latência. As três curtas vêm da mesma agregação do
/// dashboard; a semanal lê os buckets horários.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LatencyWindow {
    Minutes15,
    Hour1,
    Hours24,
    Days7,
}

impl LatencyWindow {
    fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::to_lowercase).as_deref() {
            Some("15m") => Self::Minutes15,
            Some("24h" | "1d") => Self::Hours24,
            Some("7d" | "168h") => Self::Days7,
            _ => Self::Hour1,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Minutes15 => "15m",
            Self::Hour1 => "1h",
            Self::Hours24 => "24h",
            Self::Days7 => "7d",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Minutes15 => "últimos 15 minutos",
            Self::Hour1 => "última hora",
            Self::Hours24 => "últimas 24 horas",
            Self::Days7 => "últimos 7 dias",
        }
    }
}

fn from_millis(millis: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp_millis(millis)
}

/// Série de latência e perda média do monitor na janela.
async fn latency_samples(
    ctx: &AppContext,
    monitor: &monitors::Model,
    window: LatencyWindow,
) -> AppResult<(Vec<Sample>, Option<f64>)> {
    if window == LatencyWindow::Days7 {
        let since = Utc::now() - Duration::days(7);
        let buckets = monitor_results_hourly::Entity::find()
            .filter(monitor_results_hourly::Column::MonitorId.eq(monitor.id))
            .filter(monitor_results_hourly::Column::Bucket.gte(since))
            .order_by_asc(monitor_results_hourly::Column::Bucket)
            .all(&ctx.db)
            .await?;
        let (down, total) = buckets.iter().fold((0_i64, 0_i64), |(down, total), b| {
            (
                down + i64::from(b.down_checks),
                total + i64::from(b.total_checks),
            )
        });
        let loss = (total > 0).then(|| round2(down as f64 * 100.0 / total as f64));
        let samples = buckets
            .into_iter()
            .filter_map(|bucket| {
                bucket.avg_latency_ms.map(|value| Sample {
                    at: bucket.bucket.with_timezone(&Utc),
                    value,
                })
            })
            .collect();
        return Ok((samples, loss));
    }

    let response = calculate_monitor_timeseries(
        &ctx.db,
        MonitorTimeSeriesQuery {
            monitor_id: Some(monitor.id),
            monitor_type: None,
            timeframe: Some(window.label().to_string()),
        },
    )
    .await?;
    let samples = response
        .samples
        .iter()
        .filter_map(|point| {
            from_millis(point.timestamp).map(|at| Sample {
                at,
                value: point.latency,
            })
        })
        .collect();
    let loss = (response.total_checks > 0).then(|| f64::from(response.packet_loss_pct));
    Ok((samples, loss))
}

pub struct MonitorLatencyChart;

#[async_trait]
impl AiToolHandler for MonitorLatencyChart {
    fn name(&self) -> &'static str {
        "chart_monitor_latency"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Charts
    }

    fn description(&self) -> &'static str {
        "Exibe ao usuário o gráfico de latência de um monitor (ping, DNS, HTTP...) e devolve min/máx/média e perda. Use quando pedirem gráfico ou para mostrar a evolução de uma lentidão."
    }

    fn parameters(&self) -> Value {
        let mut properties = monitor_selector_schema();
        properties.insert(
            "timeframe".into(),
            json!({ "type": "string", "enum": ["15m", "1h", "24h", "7d"], "description": "Janela (padrão 1h)" }),
        );
        json!({ "type": "object", "properties": properties })
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
        let window = LatencyWindow::parse(args.text("timeframe").as_deref());
        let (samples, loss_pct) = latency_samples(ctx, &monitor, window).await?;

        let Some(summary) = stats(&samples) else {
            return Ok(ToolOutput::not_found(format!(
                "Sem checagens de '{}' na janela {}",
                monitor.name,
                window.label()
            )));
        };

        let chart = AiChart {
            title: format!("Latência — {}", monitor.name),
            subtitle: Some(window.description().to_string()),
            unit: AiChartUnit::Latency,
            x_axis: AiChartAxis::Time,
            series: vec![AiChartSeries {
                id: format!("monitor-{}", monitor.id),
                label: "Latência".into(),
                points: chart_points(&samples),
            }],
            avg_value: Some(summary.avg),
        };
        Ok(ToolOutput::with_chart(
            json!({
                "chart_shown": true,
                "monitor": monitor.name,
                "timeframe": window.label(),
                "latency_ms": summary,
                "loss_pct": loss_pct,
            }),
            chart,
        ))
    }
}

pub struct InterfaceTrafficChart;

#[async_trait]
impl AiToolHandler for InterfaceTrafficChart {
    fn name(&self) -> &'static str {
        "chart_interface_traffic"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Charts
    }

    fn description(&self) -> &'static str {
        "Exibe ao usuário o gráfico de tráfego (download/upload em bps) de uma interface e devolve pico, média e utilização máxima. Use para investigar saturação de link."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Nome, IP ou id do dispositivo" },
                "interface": { "type": "string", "description": "Nome, alias, ifIndex ou id da interface (de get_device_interfaces)" },
                "hours": { "type": "integer", "description": "Janela em horas (1 a 168, padrão 6)" }
            },
            "required": ["device", "interface"]
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let identifier = args.required_text("device", "Informe o dispositivo")?;
        let wanted = args.required_text("interface", "Informe a interface")?;
        let Some(device) = find_device(&ctx.db, &identifier).await? else {
            return Ok(ToolOutput::not_found(format!(
                "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
            )));
        };
        let interfaces = device_interfaces_of(&ctx.db, device.id).await?;
        let Some(iface) = pick_interface(&interfaces, &wanted) else {
            return Ok(ToolOutput::not_found(format!(
                "Interface '{wanted}' não encontrada ou ambígua em '{}'; use get_device_interfaces",
                device.name
            )));
        };

        let hours = args.integer_in("hours", 6, 1, 168);
        let since = Utc::now() - Duration::hours(hours);
        let inbound = metric_samples(&ctx.db, device.id, Some(iface.id), "inBps", since).await?;
        let outbound = metric_samples(&ctx.db, device.id, Some(iface.id), "outBps", since).await?;
        let (in_stats, out_stats) = (stats(&inbound), stats(&outbound));
        if in_stats.is_none() && out_stats.is_none() {
            return Ok(ToolOutput::not_found(format!(
                "Sem tráfego gravado para {} nas últimas {hours}h — a interface precisa estar monitorada",
                iface.name
            )));
        }

        let peak = in_stats
            .map_or(0.0, |s| s.max)
            .max(out_stats.map_or(0.0, |s| s.max));
        let peak_utilization = iface
            .speed
            .filter(|speed| *speed > 0)
            .map(|speed| round2(peak * 100.0 / speed as f64));

        let mut series = Vec::new();
        for (id, label, samples) in [
            ("inBps", "Download (IN)", &inbound),
            ("outBps", "Upload (OUT)", &outbound),
        ] {
            if !samples.is_empty() {
                series.push(AiChartSeries {
                    id: id.into(),
                    label: label.into(),
                    points: chart_points(samples),
                });
            }
        }
        let chart = AiChart {
            title: format!("Tráfego — {} / {}", device.name, iface.name),
            subtitle: Some(format!("últimas {hours}h")),
            unit: AiChartUnit::Bandwidth,
            x_axis: AiChartAxis::Time,
            series,
            avg_value: None,
        };
        Ok(ToolOutput::with_chart(
            json!({
                "chart_shown": true,
                "device": device.name,
                "interface": iface.name,
                "hours": hours,
                "speed_bps": iface.speed,
                "in_bps": in_stats,
                "out_bps": out_stats,
                "peak_utilization_pct": peak_utilization,
            }),
            chart,
        ))
    }
}

/// Métricas de sistema que o SNMP grava por dispositivo.
const DEVICE_METRICS: &[(&str, &str)] = &[("cpu_usage", "CPU"), ("memory_usage", "Memória")];

pub struct DeviceMetricChart;

#[async_trait]
impl AiToolHandler for DeviceMetricChart {
    fn name(&self) -> &'static str {
        "chart_device_metric"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Charts
    }

    fn description(&self) -> &'static str {
        "Exibe ao usuário o gráfico de uma métrica de sistema do dispositivo (cpu_usage, memory_usage ou outra série de get_device_metrics) e devolve min/máx/média."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Nome, IP ou id do dispositivo" },
                "metric": { "type": "string", "description": "cpu_usage (padrão), memory_usage ou outro nome de série" },
                "hours": { "type": "integer", "description": "Janela em horas (1 a 168, padrão 24)" }
            },
            "required": ["device"]
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let identifier = args.required_text("device", "Informe o dispositivo")?;
        let Some(device) = find_device(&ctx.db, &identifier).await? else {
            return Ok(ToolOutput::not_found(format!(
                "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
            )));
        };
        let metric = args.text("metric").unwrap_or_else(|| "cpu_usage".into());
        let hours = args.integer_in("hours", 24, 1, 168);
        let since = Utc::now() - Duration::hours(hours);
        let samples = metric_samples(&ctx.db, device.id, None, &metric, since).await?;
        let Some(summary) = stats(&samples) else {
            return Ok(ToolOutput::not_found(format!(
                "Sem leituras de '{metric}' para {} nas últimas {hours}h",
                device.name
            )));
        };

        let label = DEVICE_METRICS
            .iter()
            .find(|(name, _)| *name == metric)
            .map_or(metric.as_str(), |(_, label)| label);
        let unit = if metric.ends_with("_usage") {
            AiChartUnit::Percentage
        } else {
            AiChartUnit::Generic
        };
        let chart = AiChart {
            title: format!("{label} — {}", device.name),
            subtitle: Some(format!("últimas {hours}h")),
            unit,
            x_axis: AiChartAxis::Time,
            series: vec![AiChartSeries {
                id: metric.clone(),
                label: label.to_string(),
                points: chart_points(&samples),
            }],
            avg_value: Some(summary.avg),
        };
        Ok(ToolOutput::with_chart(
            json!({
                "chart_shown": true,
                "device": device.name,
                "metric": metric,
                "hours": hours,
                "stats": summary,
            }),
            chart,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn janela_de_latencia_tolera_variacoes_e_cai_em_uma_hora() {
        assert_eq!(LatencyWindow::parse(Some("15M")), LatencyWindow::Minutes15);
        assert_eq!(LatencyWindow::parse(Some("1d")), LatencyWindow::Hours24);
        assert_eq!(LatencyWindow::parse(Some("168h")), LatencyWindow::Days7);
        assert_eq!(LatencyWindow::parse(Some("qualquer")), LatencyWindow::Hour1);
        assert_eq!(LatencyWindow::parse(None), LatencyWindow::Hour1);
    }
}
