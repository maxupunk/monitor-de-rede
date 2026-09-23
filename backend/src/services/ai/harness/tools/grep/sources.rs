//! De onde o grep lê. Cada fonte sabe consultar a sua tabela com o maior
//! pré-filtro possível no banco e devolver [`GrepRecord`] em ordem
//! cronológica; o casamento fino e a forma de saída são do motor.
//!
//! Fonte nova é um tipo novo em [`source_for`] — a ferramenta não muda.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use super::{
    super::{
        docker::{resolve_container, unavailable_message},
        log_digest::label_of,
        logs::{logs_disabled_message, origin_labeler},
        lookup::device_names,
    },
    engine::GrepRecord,
};
use crate::{
    models::{
        _entities::{alert_events, monitor_results, monitors},
        devices,
        logs::device_logs,
    },
    services::{
        docker::{engine, source::LocalEngine},
        shared::errors::AppResult,
        syslog::{
            db::LogsDb,
            repository::{self, LogFilters, LogQuery},
        },
    },
};

/// Linhas lidas por fonte a cada grep. Acima disso a resposta avisa
/// (`capped`) e a IA estreita a janela ou o dispositivo.
pub const SCAN_CAP: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct GrepFilter {
    pub device: Option<devices::Model>,
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    /// Severidade syslog máxima (só logs).
    pub severity: Option<i16>,
    /// Trecho literal que o banco pode usar para pré-filtrar.
    pub needle: Option<String>,
    /// Nome ou id do container (só a fonte `docker`).
    pub container: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub records: Vec<GrepRecord>,
    pub capped: bool,
}

#[async_trait]
pub trait GrepSource: Send + Sync {
    /// Lê a janela. `Err(mensagem)` quando a fonte não existe neste servidor.
    async fn scan(&self, ctx: &AppContext, filter: &GrepFilter) -> AppResult<Result<Scan, String>>;

    /// Até `lines` registros antes e depois de `anchor` (o `-C`). Por padrão a
    /// fonte não tem contexto.
    async fn context(
        &self,
        _ctx: &AppContext,
        _anchor: &GrepRecord,
        _lines: u64,
    ) -> AppResult<(Vec<GrepRecord>, Vec<GrepRecord>)> {
        Ok((Vec::new(), Vec::new()))
    }
}

/// A fonte pelo nome que a IA usa.
#[must_use]
pub fn source_for(name: &str) -> Option<Box<dyn GrepSource>> {
    match name {
        "logs" => Some(Box::new(Logs)),
        "alerts" => Some(Box::new(Alerts)),
        "checks" => Some(Box::new(Checks)),
        "docker" => Some(Box::new(DockerLogs)),
        _ => None,
    }
}

/// Nomes aceitos, para a mensagem de erro e o schema.
pub const SOURCE_NAMES: [&str; 4] = ["logs", "alerts", "checks", "docker"];

/// Syslog dos equipamentos e eventos da aplicação (tela `/logs`).
pub struct Logs;

impl Logs {
    fn records(
        rows: &[device_logs::Model],
        label: &impl Fn(&device_logs::Model) -> String,
    ) -> Vec<GrepRecord> {
        rows.iter()
            .map(|row| GrepRecord {
                at: row.received_at.with_timezone(&Utc),
                origin: label(row),
                severity: label_of(row.severity),
                rank: row.severity.unwrap_or(i16::MAX),
                text: row.message.clone(),
                log_id: Some(row.id),
            })
            .collect()
    }
}

#[async_trait]
impl GrepSource for Logs {
    async fn scan(&self, ctx: &AppContext, filter: &GrepFilter) -> AppResult<Result<Scan, String>> {
        let Ok(logs) = LogsDb::from_context(ctx) else {
            return Ok(Err(logs_disabled_message().to_string()));
        };
        let query = LogQuery::normalize(
            LogFilters {
                device_id: filter.device.as_ref().map(|device| device.id),
                severity: filter.severity,
                from: Some(filter.since),
                to: Some(filter.until),
                ..LogFilters::default()
            },
            filter.until,
        );
        let rows = repository::scan_containing(
            logs.connection(),
            &query,
            filter.needle.as_deref(),
            SCAN_CAP,
        )
        .await?;
        let label = origin_labeler(ctx, &rows).await?;
        Ok(Ok(Scan {
            capped: rows.len() as u64 >= SCAN_CAP,
            records: Self::records(&rows, &label),
        }))
    }

    async fn context(
        &self,
        ctx: &AppContext,
        anchor: &GrepRecord,
        lines: u64,
    ) -> AppResult<(Vec<GrepRecord>, Vec<GrepRecord>)> {
        let (Ok(logs), Some(id)) = (LogsDb::from_context(ctx), anchor.log_id) else {
            return Ok((Vec::new(), Vec::new()));
        };
        let Some(row) = device_logs::Entity::find_by_id(id)
            .one(logs.connection())
            .await?
        else {
            return Ok((Vec::new(), Vec::new()));
        };
        let (before, after) = repository::neighbors(logs.connection(), &row, lines, lines).await?;
        let all: Vec<device_logs::Model> = before.iter().chain(&after).cloned().collect();
        let label = origin_labeler(ctx, &all).await?;
        Ok((
            Self::records(&before, &label),
            Self::records(&after, &label),
        ))
    }
}

fn alert_rank(severity: &str) -> i16 {
    match severity {
        "critical" => 0,
        "warning" => 1,
        "info" => 2,
        _ => 3,
    }
}

/// Mensagens dos alertas (abertos e resolvidos) da janela.
pub struct Alerts;

#[async_trait]
impl GrepSource for Alerts {
    async fn scan(&self, ctx: &AppContext, filter: &GrepFilter) -> AppResult<Result<Scan, String>> {
        let mut query = alert_events::Entity::find()
            .filter(alert_events::Column::StartedAt.gte(filter.since))
            .filter(alert_events::Column::StartedAt.lte(filter.until))
            .order_by_desc(alert_events::Column::StartedAt)
            .limit(SCAN_CAP);
        if let Some(device) = &filter.device {
            query = query.filter(alert_events::Column::DeviceId.eq(device.id));
        }
        let mut rows = query.all(&ctx.db).await?;
        rows.reverse();
        let names = device_names(&ctx.db, rows.iter().filter_map(|row| row.device_id)).await?;
        let records = rows
            .iter()
            .map(|row| GrepRecord {
                at: row.started_at.with_timezone(&Utc),
                origin: row
                    .device_id
                    .and_then(|id| names.get(&id).cloned())
                    .unwrap_or_else(|| "sistema".into()),
                severity: format!("{} ({})", row.severity, row.status),
                rank: alert_rank(&row.severity),
                text: format!("#{} {}", row.id, row.message.as_deref().unwrap_or_default()),
                log_id: None,
            })
            .collect();
        Ok(Ok(Scan {
            capped: rows.len() as u64 >= SCAN_CAP,
            records,
        }))
    }
}

fn check_rank(status: &str) -> i16 {
    match status {
        "down" => 0,
        "warning" => 1,
        _ => 2,
    }
}

/// Mensagens das checagens que falharam (monitor fora de `up`).
pub struct Checks;

#[async_trait]
impl GrepSource for Checks {
    async fn scan(&self, ctx: &AppContext, filter: &GrepFilter) -> AppResult<Result<Scan, String>> {
        let mut monitor_query = monitors::Entity::find();
        if let Some(device) = &filter.device {
            monitor_query = monitor_query.filter(monitors::Column::DeviceId.eq(device.id));
        }
        let monitor_names: HashMap<i64, String> = monitor_query
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|monitor| (monitor.id, monitor.name))
            .collect();
        if monitor_names.is_empty() {
            return Ok(Ok(Scan::default()));
        }

        let mut query = monitor_results::Entity::find()
            .filter(monitor_results::Column::Status.ne("up"))
            .filter(monitor_results::Column::Message.is_not_null())
            .filter(monitor_results::Column::StartedAt.gte(filter.since))
            .filter(monitor_results::Column::StartedAt.lte(filter.until))
            .order_by_desc(monitor_results::Column::StartedAt)
            .limit(SCAN_CAP);
        if filter.device.is_some() {
            query = query
                .filter(monitor_results::Column::MonitorId.is_in(monitor_names.keys().copied()));
        }
        let mut rows = query.all(&ctx.db).await?;
        rows.reverse();
        let records = rows
            .iter()
            .map(|row| GrepRecord {
                at: row.started_at.with_timezone(&Utc),
                origin: monitor_names
                    .get(&row.monitor_id)
                    .cloned()
                    .unwrap_or_else(|| format!("monitor #{}", row.monitor_id)),
                severity: row.status.clone(),
                rank: check_rank(&row.status),
                text: row.message.clone().unwrap_or_default(),
                log_id: None,
            })
            .collect();
        Ok(Ok(Scan {
            capped: rows.len() as u64 >= SCAN_CAP,
            records,
        }))
    }
}

/// Saída de um container Docker (stdout e stderr) na janela.
pub struct DockerLogs;

impl DockerLogs {
    /// O `stderr` pesa como erro na ordenação por gravidade.
    fn rank(stream: &str) -> i16 {
        if stream == "stderr" {
            3
        } else {
            6
        }
    }
}

#[async_trait]
impl GrepSource for DockerLogs {
    async fn scan(
        &self,
        _ctx: &AppContext,
        filter: &GrepFilter,
    ) -> AppResult<Result<Scan, String>> {
        let Some(identifier) = filter.container.as_deref() else {
            return Ok(Err(
                "Informe 'container' (nome ou id; get_docker_containers lista)".into(),
            ));
        };
        let container = match resolve_container(identifier).await {
            Ok(container) => container,
            Err(message) => return Ok(Err(message)),
        };
        let entries = match engine::container_logs(
            &LocalEngine,
            &container.id,
            engine::LogFilters {
                tail: SCAN_CAP.to_string(),
                since: filter.since.timestamp(),
                until: filter.until.timestamp(),
                timestamps: true,
            },
        )
        .await
        {
            Ok(entries) => entries,
            Err(error) => return Ok(Err(unavailable_message(&error))),
        };
        let origin = container.display_name();
        let records: Vec<GrepRecord> = entries
            .into_iter()
            .map(|entry| GrepRecord {
                at: DateTime::parse_from_rfc3339(&entry.timestamp)
                    .map_or(filter.until, |at| at.with_timezone(&Utc)),
                origin: origin.clone(),
                rank: Self::rank(&entry.stream),
                severity: entry.stream,
                text: entry.message,
                log_id: None,
            })
            .collect();
        Ok(Ok(Scan {
            capped: records.len() as u64 >= SCAN_CAP,
            records,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toda_fonte_listada_existe() {
        for name in SOURCE_NAMES {
            assert!(source_for(name).is_some(), "{name}");
        }
        assert!(source_for("etc").is_none());
    }

    #[test]
    fn gravidade_ordena_do_pior_para_o_melhor() {
        assert!(alert_rank("critical") < alert_rank("warning"));
        assert!(check_rank("down") < check_rank("warning"));
        assert!(DockerLogs::rank("stderr") < DockerLogs::rank("stdout"));
    }
}
