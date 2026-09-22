//! Estado atual do que o NetMonitor conhece: dispositivos, interfaces,
//! monitores, alertas e a documentação interna.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use super::{
    lookup::{device_interfaces_of, device_matches, device_names, find_device},
    series::round2,
    AiToolHandler, ToolArgs, ToolOutput,
};
use crate::{
    models::_entities::{alert_events, device_interfaces, devices, monitors},
    services::{
        ai::knowledge, alerts::contracts::AlertStatus,
        monitoring::metrics_repository::latest_for_interfaces, shared::errors::AppResult,
    },
};

/// Teto de linhas por listagem: acima disso a IA deve filtrar.
const MAX_LIST_ROWS: usize = 50;

const DEVICE_ARG: &str = "Nome, IP ou id do dispositivo";

fn device_not_found(identifier: &str) -> ToolOutput {
    ToolOutput::not_found(format!(
        "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
    ))
}

pub struct SystemSummary;

#[async_trait]
impl AiToolHandler for SystemSummary {
    fn name(&self) -> &'static str {
        "get_system_summary"
    }

    fn description(&self) -> &'static str {
        "Resumo da infraestrutura: dispositivos por status, monitores por status e alertas abertos por severidade. Ponto de partida de um diagnóstico geral."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, ctx: &AppContext, _args: &ToolArgs) -> AppResult<ToolOutput> {
        let count_by = |statuses: Vec<String>| {
            let mut counts: HashMap<String, usize> = HashMap::new();
            for status in statuses {
                *counts.entry(status).or_default() += 1;
            }
            counts
        };

        let device_statuses: Vec<String> = devices::Entity::find()
            .select_only()
            .column(devices::Column::Status)
            .into_tuple()
            .all(&ctx.db)
            .await?;
        let monitor_statuses: Vec<String> = monitors::Entity::find()
            .select_only()
            .column(monitors::Column::Status)
            .filter(monitors::Column::Enabled.eq(true))
            .into_tuple()
            .all(&ctx.db)
            .await?;
        let open_severities: Vec<String> = alert_events::Entity::find()
            .select_only()
            .column(alert_events::Column::Severity)
            .filter(alert_events::Column::Status.is_in(AlertStatus::OPEN))
            .into_tuple()
            .all(&ctx.db)
            .await?;

        Ok(ToolOutput::data(json!({
            "devices": { "total": device_statuses.len(), "by_status": count_by(device_statuses) },
            "enabled_monitors": { "total": monitor_statuses.len(), "by_status": count_by(monitor_statuses) },
            "open_alerts": { "total": open_severities.len(), "by_severity": count_by(open_severities) },
        })))
    }
}

pub struct ListDevices;

#[async_trait]
impl AiToolHandler for ListDevices {
    fn name(&self) -> &'static str {
        "list_devices"
    }

    fn description(&self) -> &'static str {
        "Lista dispositivos cadastrados (id, nome, IP, tipo, fabricante, status, último contato)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "search": { "type": "string", "description": "Filtra por parte do nome, IP ou fabricante" },
                "status": { "type": "string", "enum": ["up", "down", "warning", "unknown"] }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let mut query = devices::Entity::find().order_by_asc(devices::Column::Name);
        if let Some(status) = args.text("status") {
            query = query.filter(devices::Column::Status.eq(status));
        }
        let search = args.text("search").unwrap_or_default();
        let matching: Vec<devices::Model> = query
            .all(&ctx.db)
            .await?
            .into_iter()
            .filter(|device| device_matches(device, &search))
            .collect();

        let rows: Vec<Value> = matching
            .iter()
            .take(MAX_LIST_ROWS)
            .map(|device| {
                json!({
                    "id": device.id,
                    "name": device.name,
                    "ip": device.ip_address,
                    "type": device.r#type,
                    "vendor": device.vendor,
                    "status": device.status,
                    "last_seen": device.last_seen_at,
                })
            })
            .collect();
        Ok(ToolOutput::data(json!({
            "total": matching.len(),
            "shown": rows.len(),
            "devices": rows,
        })))
    }
}

pub struct DeviceDetail;

#[async_trait]
impl AiToolHandler for DeviceDetail {
    fn name(&self) -> &'static str {
        "get_device_detail"
    }

    fn description(&self) -> &'static str {
        "Detalhes de um dispositivo: dados cadastrais, monitores (com id e status), resumo das interfaces e alertas abertos."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "identifier": { "type": "string", "description": DEVICE_ARG }
            },
            "required": ["identifier"]
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let identifier = args.required_text("identifier", "Informe o dispositivo")?;
        let Some(device) = find_device(&ctx.db, &identifier).await? else {
            return Ok(device_not_found(&identifier));
        };

        let device_monitors: Vec<Value> = monitors::Entity::find()
            .filter(monitors::Column::DeviceId.eq(device.id))
            .order_by_asc(monitors::Column::Name)
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|monitor| {
                json!({
                    "id": monitor.id,
                    "name": monitor.name,
                    "type": monitor.r#type,
                    "status": monitor.status,
                    "enabled": monitor.enabled,
                    "interval_s": monitor.interval_seconds,
                    "last_run": monitor.last_run_at,
                })
            })
            .collect();

        let interfaces = device_interfaces_of(&ctx.db, device.id).await?;
        let oper_up = interfaces
            .iter()
            .filter(|iface| iface.oper_status.as_deref() == Some("up"))
            .count();
        let down_but_enabled = interfaces
            .iter()
            .filter(|iface| is_down_but_enabled(iface))
            .count();

        let open_alerts = alert_events::Entity::find()
            .filter(alert_events::Column::DeviceId.eq(device.id))
            .filter(alert_events::Column::Status.is_in(AlertStatus::OPEN))
            .count(&ctx.db)
            .await?;

        Ok(ToolOutput::data(json!({
            "id": device.id,
            "name": device.name,
            "ip": device.ip_address,
            "type": device.r#type,
            "vendor": device.vendor,
            "model": device.model,
            "operating_system": device.operating_system,
            "status": device.status,
            "snmp_enabled": device.snmp_enabled,
            "last_seen": device.last_seen_at,
            "uplink_interface": device.link_interface_name,
            "interfaces": {
                "total": interfaces.len(),
                "oper_up": oper_up,
                "down_but_admin_up": down_but_enabled,
            },
            "open_alerts": open_alerts,
            "monitors": device_monitors,
        })))
    }
}

/// Porta habilitada que não subiu: cabo, SFP ou o outro lado.
fn is_down_but_enabled(iface: &device_interfaces::Model) -> bool {
    iface.admin_status.as_deref() == Some("up") && iface.oper_status.as_deref() != Some("up")
}

/// Maior sentido do tráfego sobre a velocidade negociada, em %.
fn utilization_pct(in_bps: Option<f64>, out_bps: Option<f64>, speed: Option<i64>) -> Option<f64> {
    let speed = speed.filter(|speed| *speed > 0)? as f64;
    let peak = in_bps.unwrap_or(0.0).max(out_bps.unwrap_or(0.0));
    Some(round2(peak * 100.0 / speed))
}

/// Utilização a partir da qual a porta entra no filtro de problemas.
const SATURATION_PCT: f64 = 80.0;

pub struct DeviceInterfaces;

#[async_trait]
impl AiToolHandler for DeviceInterfaces {
    fn name(&self) -> &'static str {
        "get_device_interfaces"
    }

    fn description(&self) -> &'static str {
        "Interfaces de um dispositivo SNMP: status administrativo/operacional, velocidade, tráfego atual (bps) e utilização (%). Use problems_only para ver só portas caídas ou saturadas."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": DEVICE_ARG },
                "problems_only": { "type": "boolean", "description": "Só portas habilitadas e caídas ou acima de 80% de uso" }
            },
            "required": ["device"]
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let identifier = args.required_text("device", "Informe o dispositivo")?;
        let Some(device) = find_device(&ctx.db, &identifier).await? else {
            return Ok(device_not_found(&identifier));
        };
        let interfaces = device_interfaces_of(&ctx.db, device.id).await?;
        if interfaces.is_empty() {
            return Ok(ToolOutput::data(json!({
                "device": device.name,
                "total": 0,
                "note": "Nenhuma interface coletada — o dispositivo precisa de SNMP habilitado.",
            })));
        }

        let ids: Vec<i64> = interfaces.iter().map(|iface| iface.id).collect();
        let mut traffic: HashMap<(i64, String), f64> = HashMap::new();
        for metric in
            latest_for_interfaces(&ctx.db, Some(device.id), &ids, &["inBps", "outBps"]).await?
        {
            if let Some(interface_id) = metric.interface_id {
                traffic.insert((interface_id, metric.name), metric.value);
            }
        }

        let problems_only = args.flag("problems_only");
        let mut rows: Vec<Value> = Vec::new();
        for iface in &interfaces {
            let in_bps = traffic.get(&(iface.id, "inBps".to_string())).copied();
            let out_bps = traffic.get(&(iface.id, "outBps".to_string())).copied();
            let utilization = utilization_pct(in_bps, out_bps, iface.speed);
            let saturated = utilization.is_some_and(|pct| pct >= SATURATION_PCT);
            if problems_only && !is_down_but_enabled(iface) && !saturated {
                continue;
            }
            rows.push(json!({
                "id": iface.id,
                "name": iface.name,
                "alias": iface.alias,
                "admin": iface.admin_status,
                "oper": iface.oper_status,
                "speed_bps": iface.speed,
                "in_bps": in_bps.map(round2),
                "out_bps": out_bps.map(round2),
                "utilization_pct": utilization,
                "uplink": device.link_interface_id == Some(iface.id),
            }));
        }
        let matched = rows.len();
        rows.truncate(MAX_LIST_ROWS * 2);

        Ok(ToolOutput::data(json!({
            "device": device.name,
            "total": interfaces.len(),
            "matched": matched,
            "interfaces": rows,
        })))
    }
}

pub struct ListMonitors;

#[async_trait]
impl AiToolHandler for ListMonitors {
    fn name(&self) -> &'static str {
        "list_monitors"
    }

    fn description(&self) -> &'static str {
        "Lista monitores (id, nome, tipo, status, dispositivo). Use para achar o monitor_id antes de consultar histórico ou gráfico."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": DEVICE_ARG },
                "type": { "type": "string", "description": "Tipo do monitor: ping, http, tcp, dns, snmp..." },
                "status": { "type": "string", "enum": ["up", "down", "warning", "unknown"] }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let mut query = monitors::Entity::find().order_by_asc(monitors::Column::Name);
        if let Some(identifier) = args.text("device") {
            let Some(device) = find_device(&ctx.db, &identifier).await? else {
                return Ok(device_not_found(&identifier));
            };
            query = query.filter(monitors::Column::DeviceId.eq(device.id));
        }
        if let Some(kind) = args.text("type") {
            query = query.filter(monitors::Column::Type.eq(kind));
        }
        if let Some(status) = args.text("status") {
            query = query.filter(monitors::Column::Status.eq(status));
        }

        let found = query.all(&ctx.db).await?;
        let names = device_names(&ctx.db, found.iter().filter_map(|m| m.device_id)).await?;
        let rows: Vec<Value> = found
            .iter()
            .take(MAX_LIST_ROWS)
            .map(|monitor| {
                json!({
                    "id": monitor.id,
                    "name": monitor.name,
                    "type": monitor.r#type,
                    "status": monitor.status,
                    "enabled": monitor.enabled,
                    "device": monitor.device_id.and_then(|id| names.get(&id)),
                    "last_run": monitor.last_run_at,
                })
            })
            .collect();
        Ok(ToolOutput::data(json!({
            "total": found.len(),
            "shown": rows.len(),
            "monitors": rows,
        })))
    }
}

pub struct Alerts;

#[async_trait]
impl AiToolHandler for Alerts {
    fn name(&self) -> &'static str {
        "get_alerts"
    }

    fn description(&self) -> &'static str {
        "Alertas do sistema. status='open' (padrão) traz os abertos; 'resolved' ou 'all' consultam o histórico das últimas N horas, com início, fim e duração."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": { "type": "string", "enum": ["open", "resolved", "all"] },
                "severity": { "type": "string", "enum": ["critical", "warning", "info"] },
                "device": { "type": "string", "description": DEVICE_ARG },
                "hours": { "type": "integer", "description": "Janela do histórico em horas (1 a 720, padrão 24)" },
                "limit": { "type": "integer", "description": "Máximo de alertas (1 a 50, padrão 10)" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let status = args.text("status").unwrap_or_else(|| "open".into());
        let limit = args.integer_in("limit", 10, 1, MAX_LIST_ROWS as i64);
        let hours = args.integer_in("hours", 24, 1, 720);

        let mut query = alert_events::Entity::find().order_by_desc(alert_events::Column::StartedAt);
        query = match status.as_str() {
            "resolved" => query.filter(alert_events::Column::Status.eq(AlertStatus::Resolved)),
            "all" => query,
            _ => query.filter(alert_events::Column::Status.is_in(AlertStatus::OPEN)),
        };
        if status != "open" {
            let since = Utc::now() - Duration::hours(hours);
            query = query.filter(alert_events::Column::StartedAt.gte(since));
        }
        if let Some(severity) = args.text("severity") {
            query = query.filter(alert_events::Column::Severity.eq(severity));
        }
        if let Some(identifier) = args.text("device") {
            let Some(device) = find_device(&ctx.db, &identifier).await? else {
                return Ok(device_not_found(&identifier));
            };
            query = query.filter(alert_events::Column::DeviceId.eq(device.id));
        }

        let total = query.clone().count(&ctx.db).await?;
        let rows = query.limit(limit as u64).all(&ctx.db).await?;
        let names = device_names(&ctx.db, rows.iter().filter_map(|row| row.device_id)).await?;
        let alerts: Vec<Value> = rows
            .into_iter()
            .map(|row| {
                let duration_min = row
                    .resolved_at
                    .map(|end| (end - row.started_at).num_minutes());
                json!({
                    "id": row.id,
                    "device": row.device_id.and_then(|id| names.get(&id)),
                    "monitor_id": row.monitor_id,
                    "severity": row.severity,
                    "status": row.status,
                    "message": row.message,
                    "started_at": row.started_at,
                    "resolved_at": row.resolved_at,
                    "duration_min": duration_min,
                })
            })
            .collect();
        Ok(ToolOutput::data(json!({
            "filter": status,
            "total": total,
            "alerts": alerts,
        })))
    }
}

pub struct SearchDocs;

#[async_trait]
impl AiToolHandler for SearchDocs {
    fn name(&self) -> &'static str {
        "search_system_docs"
    }

    fn description(&self) -> &'static str {
        "Consulta a documentação interna do NetMonitor (SNMP, VPN, descoberta, janelas de manutenção, alertas, monitores)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Termo ou dúvida sobre o NetMonitor" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let query = args.text("query").unwrap_or_default();
        let results: Vec<Value> = knowledge::search_docs(&query)
            .into_iter()
            .map(|doc| json!({ "topic": doc.title, "content": doc.content }))
            .collect();
        Ok(ToolOutput::data(
            json!({ "query": query, "results": results }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utilizacao_usa_o_sentido_mais_carregado() {
        assert_eq!(
            utilization_pct(Some(900_000.0), Some(100_000.0), Some(1_000_000)),
            Some(90.0)
        );
        assert_eq!(utilization_pct(Some(1.0), None, None), None);
        assert_eq!(utilization_pct(Some(1.0), None, Some(0)), None);
    }
}
