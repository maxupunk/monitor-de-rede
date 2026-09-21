//! Histórico gravado no banco: disponibilidade e falhas de monitores e as
//! métricas de sistema (CPU, memória, sensores) de um dispositivo.

use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use super::{
    lookup::{find_device, find_monitor},
    series::{round2, stats, Sample},
    AiToolHandler, ToolArgs, ToolOutput,
};
use crate::{
    models::_entities::{metrics, monitor_results},
    services::{monitoring::uptime::uptime_for_monitor, shared::errors::AppResult},
};

/// Falhas recentes devolvidas junto do uptime.
const RECENT_FAILURES: u64 = 10;

/// Leituras de uma série de `metrics` a partir de `since`, em ordem cronológica.
///
/// `interface_id = None` lê as métricas do dispositivo inteiro (CPU,
/// memória); com `Some`, as da porta (tráfego).
///
/// # Errors
///
/// Propaga erro do banco.
pub(super) async fn metric_samples<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    interface_id: Option<i64>,
    name: &str,
    since: DateTime<Utc>,
) -> AppResult<Vec<Sample>> {
    let mut query = metrics::Entity::find()
        .filter(metrics::Column::DeviceId.eq(device_id))
        .filter(metrics::Column::Name.eq(name))
        .filter(metrics::Column::RecordedAt.gte(since))
        .order_by_asc(metrics::Column::RecordedAt);
    query = match interface_id {
        Some(id) => query.filter(metrics::Column::InterfaceId.eq(id)),
        None => query.filter(metrics::Column::InterfaceId.is_null()),
    };
    Ok(query
        .all(db)
        .await?
        .into_iter()
        .map(|row| Sample {
            at: row.recorded_at.with_timezone(&Utc),
            value: row.value,
        })
        .collect())
}

/// Argumentos comuns para localizar um monitor.
pub(super) fn monitor_selector_schema() -> serde_json::Map<String, Value> {
    let schema = json!({
        "monitor_id": { "type": "integer", "description": "Id do monitor (de list_monitors ou get_device_detail)" },
        "device": { "type": "string", "description": "Alternativa ao monitor_id: nome, IP ou id do dispositivo" },
        "monitor_type": { "type": "string", "description": "Tipo do monitor do dispositivo (padrão ping)" }
    });
    schema.as_object().cloned().unwrap_or_default()
}

pub struct MonitorHistory;

#[async_trait]
impl AiToolHandler for MonitorHistory {
    fn name(&self) -> &'static str {
        "get_monitor_history"
    }

    fn description(&self) -> &'static str {
        "Histórico de um monitor nas últimas N horas: uptime (%), checagens, latência média e as falhas mais recentes com mensagem. Base para saber se um problema é pontual ou recorrente."
    }

    fn parameters(&self) -> Value {
        let mut properties = monitor_selector_schema();
        properties.insert(
            "hours".into(),
            json!({ "type": "integer", "description": "Janela em horas (1 a 720, padrão 24)" }),
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
        let hours = args.integer_in("hours", 24, 1, 720);
        let uptime = uptime_for_monitor(&ctx.db, monitor.id, hours).await?;

        let since = Utc::now() - Duration::hours(hours);
        let failures: Vec<Value> = monitor_results::Entity::find()
            .filter(monitor_results::Column::MonitorId.eq(monitor.id))
            .filter(monitor_results::Column::Status.ne("up"))
            .filter(monitor_results::Column::StartedAt.gte(since))
            .order_by_desc(monitor_results::Column::StartedAt)
            .limit(RECENT_FAILURES)
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|row| {
                json!({
                    "at": row.started_at,
                    "status": row.status,
                    "message": row.message,
                })
            })
            .collect();

        Ok(ToolOutput::data(json!({
            "monitor": {
                "id": monitor.id,
                "name": monitor.name,
                "type": monitor.r#type,
                "current_status": monitor.status,
            },
            "hours": hours,
            "uptime_pct": uptime.uptime_percentage,
            "checks": {
                "total": uptime.total_checks,
                "up": uptime.up_checks,
                "down": uptime.down_checks,
                "unknown": uptime.unknown_checks,
            },
            "avg_latency_ms": uptime.avg_latency_ms.map(round2),
            "recent_failures": failures,
        })))
    }
}

/// Séries que não fazem sentido resumir por min/máx/média.
const NON_STATISTICAL: &[&str] = &["snmp_uptime"];

pub struct DeviceMetrics;

#[async_trait]
impl AiToolHandler for DeviceMetrics {
    fn name(&self) -> &'static str {
        "get_device_metrics"
    }

    fn description(&self) -> &'static str {
        "Métricas de sistema de um dispositivo SNMP nas últimas N horas (CPU, memória, sensores): mínimo, máximo, média e valor atual de cada série."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Nome, IP ou id do dispositivo" },
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
        let hours = args.integer_in("hours", 24, 1, 168);
        let since = Utc::now() - Duration::hours(hours);

        let rows = metrics::Entity::find()
            .filter(metrics::Column::DeviceId.eq(device.id))
            .filter(metrics::Column::InterfaceId.is_null())
            .filter(metrics::Column::RecordedAt.gte(since))
            .order_by_asc(metrics::Column::RecordedAt)
            .all(&ctx.db)
            .await?;

        let mut grouped: BTreeMap<String, (String, Vec<Sample>)> = BTreeMap::new();
        for row in rows {
            let (_, samples) = grouped
                .entry(row.name.clone())
                .or_insert_with(|| (row.unit.clone(), Vec::new()));
            samples.push(Sample {
                at: row.recorded_at.with_timezone(&Utc),
                value: row.value,
            });
        }

        let series: Vec<Value> = grouped
            .into_iter()
            .filter(|(name, _)| !NON_STATISTICAL.contains(&name.as_str()))
            .filter_map(|(name, (unit, samples))| {
                stats(&samples)
                    .map(|summary| json!({ "metric": name, "unit": unit, "stats": summary }))
            })
            .collect();

        Ok(ToolOutput::data(json!({
            "device": device.name,
            "hours": hours,
            "series": series,
        })))
    }
}
