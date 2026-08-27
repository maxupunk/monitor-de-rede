//! DTO de apresentação compartilhado pela lista global e pela aba de dispositivos.

use std::collections::{hash_map::Entry, HashMap};

use sea_orm::{
    ColumnTrait, DatabaseBackend, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, Statement, Value,
};
use serde::Serialize;

use crate::{
    models::{
        _entities::metrics as metrics_entity, device_interfaces, devices, monitor_results,
        monitors, probes,
    },
    services::shared::errors::AppResult,
};

/// Limite por monitor, não limite global da consulta.
pub const RECENT_RESULTS_LIMIT: u64 = 30;
/// Quantidade de pontos para os sparklines de uso.
pub const GAUGE_HISTORY_LIMIT: u64 = 20;

/// Referência curta de um recurso relacionado, sem expor dados sensíveis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedResource {
    pub id: i64,
    pub name: String,
}

/// Resultado pronto para as barras de histórico do Vue.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorResultPresentation {
    pub id: i64,
    pub monitor_id: i64,
    pub probe_id: Option<i64>,
    pub status: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i32,
    pub latency_ms: Option<f64>,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}

impl From<monitor_results::Model> for MonitorResultPresentation {
    fn from(value: monitor_results::Model) -> Self {
        Self {
            id: value.id,
            monitor_id: value.monitor_id,
            probe_id: value.probe_id,
            status: value.status,
            started_at: value.started_at.to_rfc3339(),
            finished_at: value.finished_at.to_rfc3339(),
            duration_ms: value.duration_ms,
            latency_ms: value.latency_ms,
            message: value.message,
            data: value.data,
        }
    }
}

/// Última leitura de uma métrica-gauge e a série usada pelo sparkline.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GaugeReading {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub recorded_at: String,
}

/// Amostra compacta de um gauge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GaugeSample {
    pub value: f64,
    pub recorded_at: String,
}

/// Formato que ambas as telas de monitores consomem.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorPresentation {
    pub id: i64,
    pub device_id: Option<i64>,
    pub probe_id: Option<i64>,
    pub name: String,
    #[serde(rename = "type")]
    pub monitor_type: String,
    pub configuration: serde_json::Value,
    pub interval_seconds: i32,
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub enabled: bool,
    pub is_enabled: bool,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub target: String,
    pub port: Option<i64>,
    pub latency_ms: Option<f64>,
    pub device: Option<NamedResource>,
    pub probe: Option<NamedResource>,
    pub recent_results: Vec<MonitorResultPresentation>,
    pub gauge_metric: Option<GaugeReading>,
    pub gauge_history: Vec<GaugeSample>,
}

/// Nome da métrica de uso configurada por um monitor SNMP.
#[must_use]
pub fn gauge_metric_name(monitor: &monitors::Model) -> Option<&str> {
    (monitor.r#type == "snmp")
        .then(|| {
            monitor
                .configuration
                .get("metric")
                .and_then(serde_json::Value::as_str)
        })
        .flatten()
        .filter(|name| {
            matches!(
                *name,
                "cpu_usage" | "memory_usage" | "traffic" | "interface_traffic"
            )
        })
}

/// Busca e apresenta os monitores. Cada histórico é limitado individualmente;
/// esta separação evita a regressão clássica de aplicar `LIMIT 30` ao conjunto
/// inteiro e deixar os últimos monitores sem timeline.
pub async fn present_monitors(
    db: &DatabaseConnection,
    monitor_rows: Vec<monitors::Model>,
    recent_limit: u64,
) -> AppResult<Vec<MonitorPresentation>> {
    let mut device_cache: HashMap<i64, NamedResource> = HashMap::new();
    let mut probe_cache: HashMap<i64, NamedResource> = HashMap::new();
    let mut recent_results_by_monitor =
        fetch_recent_results(db, &monitor_rows, recent_limit).await?;
    let mut output = Vec::with_capacity(monitor_rows.len());
    for monitor in monitor_rows {
        let device = if let Some(id) = monitor.device_id {
            if let Entry::Vacant(entry) = device_cache.entry(id) {
                if let Some(model) = devices::Entity::find_by_id(id).one(db).await? {
                    entry.insert(NamedResource {
                        id,
                        name: model.name,
                    });
                }
            }
            device_cache.get(&id).cloned()
        } else {
            None
        };
        let probe = if let Some(id) = monitor.probe_id {
            if let Entry::Vacant(entry) = probe_cache.entry(id) {
                if let Some(model) = probes::Entity::find_by_id(id).one(db).await? {
                    entry.insert(NamedResource {
                        id,
                        name: model.name,
                    });
                }
            }
            probe_cache.get(&id).cloned()
        } else {
            None
        };
        let recent_results = recent_results_by_monitor
            .remove(&monitor.id)
            .unwrap_or_default();
        let latency_ms = recent_results.last().and_then(|result| result.latency_ms);
        let (gauge_metric, gauge_history) = fetch_gauge_metrics(db, &monitor).await?;
        let is_enabled = monitor.is_enabled();
        let target = monitor.target();
        let port = monitor.port();
        output.push(MonitorPresentation {
            id: monitor.id,
            device_id: monitor.device_id,
            probe_id: monitor.probe_id,
            name: monitor.name.clone(),
            monitor_type: monitor.r#type.clone(),
            configuration: monitor.configuration.clone(),
            interval_seconds: monitor.interval_seconds,
            timeout_seconds: monitor.timeout_seconds,
            retry_count: monitor.retry_count,
            enabled: monitor.enabled,
            is_enabled,
            next_run_at: monitor.next_run_at.map(|value| value.to_rfc3339()),
            last_run_at: monitor.last_run_at.map(|value| value.to_rfc3339()),
            status: monitor.status.clone(),
            created_at: monitor.created_at.to_rfc3339(),
            updated_at: monitor.updated_at.to_rfc3339(),
            target,
            port,
            latency_ms,
            device,
            probe,
            recent_results,
            gauge_metric,
            gauge_history,
        });
    }
    Ok(output)
}

/// Busca os últimos resultados com uma window function, preservando o limite
/// por monitor em uma única consulta nos dois dialetos suportados.
async fn fetch_recent_results(
    db: &DatabaseConnection,
    monitors: &[monitors::Model],
    recent_limit: u64,
) -> AppResult<HashMap<i64, Vec<MonitorResultPresentation>>> {
    if monitors.is_empty() || recent_limit == 0 {
        return Ok(HashMap::new());
    }

    let monitor_ids: Vec<i64> = monitors.iter().map(|monitor| monitor.id).collect();
    let backend = db.get_database_backend();
    let marker = |position: usize| match backend {
        DatabaseBackend::Postgres => format!("${position}"),
        _ => "?".to_string(),
    };
    let monitor_markers = (1..=monitor_ids.len())
        .map(marker)
        .collect::<Vec<_>>()
        .join(", ");
    let limit_marker = marker(monitor_ids.len() + 1);
    let sql = format!(
        "SELECT id, monitor_id, probe_id, status, started_at, finished_at, duration_ms, \
         latency_ms, message, data, created_at FROM (\
           SELECT mr.id, mr.monitor_id, mr.probe_id, mr.status, mr.started_at, mr.finished_at, \
                  mr.duration_ms, mr.latency_ms, mr.message, mr.data, mr.created_at, \
                  ROW_NUMBER() OVER (PARTITION BY mr.monitor_id ORDER BY mr.started_at DESC, mr.id DESC) AS result_rank \
           FROM monitor_results mr WHERE mr.monitor_id IN ({monitor_markers})\
         ) ranked WHERE result_rank <= {limit_marker} \
         ORDER BY monitor_id ASC, started_at ASC, id ASC"
    );
    let mut values: Vec<Value> = monitor_ids.into_iter().map(Into::into).collect();
    values.push((recent_limit.min(i64::MAX as u64) as i64).into());
    let rows = monitor_results::Model::find_by_statement(Statement::from_sql_and_values(
        backend, sql, values,
    ))
    .all(db)
    .await?;

    let mut grouped: HashMap<i64, Vec<MonitorResultPresentation>> = HashMap::new();
    for row in rows {
        grouped
            .entry(row.monitor_id)
            .or_default()
            .push(MonitorResultPresentation::from(row));
    }
    Ok(grouped)
}

async fn fetch_gauge_metrics(
    db: &DatabaseConnection,
    monitor: &monitors::Model,
) -> AppResult<(Option<GaugeReading>, Vec<GaugeSample>)> {
    let (Some(device_id), Some(metric_name)) = (monitor.device_id, gauge_metric_name(monitor))
    else {
        return Ok((None, Vec::new()));
    };

    let is_traffic = matches!(metric_name, "traffic" | "interface_traffic");
    let is_memory = metric_name == "memory_usage";

    let mut query =
        metrics_entity::Entity::find().filter(metrics_entity::Column::DeviceId.eq(device_id));

    if is_traffic {
        query = query.filter(metrics_entity::Column::Name.eq("inBps"));
    } else if is_memory {
        query = query.filter(metrics_entity::Column::Name.is_in(["memory_usage", "memory_used"]));
    } else {
        query = query.filter(metrics_entity::Column::Name.eq(metric_name));
    }

    if is_traffic {
        let configured_index = monitor
            .configuration
            .get("ifIndex")
            .and_then(serde_json::Value::as_i64)
            .map(|v| v as i32);
        let configured_name = monitor
            .configuration
            .get("ifName")
            .and_then(serde_json::Value::as_str)
            .or_else(|| monitor.name.strip_prefix("Interface "));

        let intf = if let Some(index) = configured_index {
            device_interfaces::Entity::find()
                .filter(crate::models::_entities::device_interfaces::Column::DeviceId.eq(device_id))
                .filter(
                    crate::models::_entities::device_interfaces::Column::SnmpIndex.eq(Some(index)),
                )
                .one(db)
                .await?
        } else {
            None
        };

        let intf = match intf {
            Some(i) => Some(i),
            None => {
                if let Some(name) = configured_name {
                    device_interfaces::Entity::find()
                        .filter(
                            crate::models::_entities::device_interfaces::Column::DeviceId
                                .eq(device_id),
                        )
                        .filter(
                            crate::models::_entities::device_interfaces::Column::Name
                                .eq(name.trim()),
                        )
                        .one(db)
                        .await?
                } else {
                    None
                }
            }
        };

        if let Some(intf) = intf {
            query = query.filter(metrics_entity::Column::InterfaceId.eq(Some(intf.id)));
        }
    }

    let rows = query
        .order_by_desc(metrics_entity::Column::RecordedAt)
        .limit(GAUGE_HISTORY_LIMIT)
        .all(db)
        .await?;
    let latest = rows.first().map(|row| GaugeReading {
        name: if is_traffic {
            "interface_traffic".to_string()
        } else {
            row.name.clone()
        },
        value: row.value,
        unit: row.unit.clone(),
        recorded_at: row.recorded_at.to_rfc3339(),
    });
    let mut history: Vec<_> = rows
        .into_iter()
        .map(|row| GaugeSample {
            value: row.value,
            recorded_at: row.recorded_at.to_rfc3339(),
        })
        .collect();
    history.reverse();
    Ok((latest, history))
}
