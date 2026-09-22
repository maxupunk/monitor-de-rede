//! Logs da tela `/logs` (syslog dos equipamentos e eventos da aplicação).
//!
//! A visão geral agrupa milhares de linhas em padrões. Para procurar algo
//! específico a IA usa o `grep` (ver [`super::grep`]), que também lê esta
//! tabela.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use super::{
    log_digest::{parse_severity, summarize},
    lookup::{device_names, find_device},
    AiToolHandler, ToolArgs, ToolOutput,
};
use crate::{
    models::logs::device_logs,
    services::{
        shared::errors::AppResult,
        syslog::{
            db::LogsDb,
            repository::{self, LogFilters, LogQuery},
        },
    },
};

/// Linhas lidas para a visão geral. Acima disso o resumo é de uma amostra
/// das mais recentes, e a saída avisa.
const OVERVIEW_SAMPLE: u64 = 5_000;
/// Mesma janela máxima da tela de logs.
pub(super) const MAX_HOURS: i64 = repository::MAX_WINDOW_DAYS * 24;

fn filter_schema() -> Value {
    let properties = json!({
        "device": { "type": "string", "description": "Nome, IP ou id do dispositivo (omitido: todos)" },
        "severity": { "type": "string", "description": "Severidade máxima: error, warning, notice, info... ou 0-7. 'error' traz erro, crítico, alerta e emergência." },
        "query": { "type": "string", "description": "Restringe às mensagens com este termo (ex: 'link down')" },
        "hours": { "type": "integer", "description": "Janela em horas (1 a 168, padrão 24)" }
    });
    json!({ "type": "object", "properties": properties })
}

/// Filtro resolvido a partir dos argumentos, ou a mensagem para a IA.
async fn build_query(ctx: &AppContext, args: &ToolArgs) -> AppResult<Result<LogQuery, ToolOutput>> {
    let device_id = match args.text("device") {
        Some(identifier) => match find_device(&ctx.db, &identifier).await? {
            Some(device) => Some(device.id),
            None => {
                return Ok(Err(ToolOutput::not_found(format!(
                    "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
                ))))
            }
        },
        None => None,
    };
    let severity = match args.text("severity") {
        Some(raw) => match parse_severity(&raw) {
            Some(value) => Some(value),
            None => {
                return Ok(Err(ToolOutput::not_found(format!(
                    "Severidade '{raw}' inválida; use error, warning, notice, info ou 0-7"
                ))))
            }
        },
        None => None,
    };
    let now = Utc::now();
    let hours = args.integer_in("hours", 24, 1, MAX_HOURS);
    Ok(Ok(LogQuery::normalize(
        LogFilters {
            device_id,
            severity,
            from: Some(now - Duration::hours(hours)),
            to: Some(now),
            q: args.text("query"),
            ..LogFilters::default()
        },
        now,
    )))
}

/// Como nomear a origem de cada linha: dispositivo cadastrado, a própria
/// aplicação ou, sem vínculo, o hostname/IP que chegou no pacote.
pub(super) async fn origin_labeler(
    ctx: &AppContext,
    rows: &[device_logs::Model],
) -> AppResult<impl Fn(&device_logs::Model) -> String> {
    let names: HashMap<i64, String> =
        device_names(&ctx.db, rows.iter().filter_map(|row| row.device_id)).await?;
    Ok(move |row: &device_logs::Model| {
        if row.source == "application" {
            return "NetMonitor (aplicação)".to_string();
        }
        row.device_id
            .and_then(|id| names.get(&id).cloned())
            .or_else(|| row.hostname.clone())
            .unwrap_or_else(|| row.source_ip.clone())
    })
}

pub(super) const fn logs_disabled_message() -> &'static str {
    "A coleta de logs está desligada neste servidor (SYSLOG_ENABLED=false); não há logs para consultar."
}

fn logs_disabled() -> ToolOutput {
    ToolOutput::not_found(logs_disabled_message())
}

fn window(query: &LogQuery) -> Value {
    json!({ "from": query.from.to_rfc3339(), "to": query.to.to_rfc3339() })
}

pub struct LogsOverview;

#[async_trait]
impl AiToolHandler for LogsOverview {
    fn name(&self) -> &'static str {
        "get_logs_overview"
    }

    fn description(&self) -> &'static str {
        "Visão geral dos logs (syslog dos equipamentos e da aplicação) numa janela: contagem por severidade, dispositivos e apps que mais registram, horas de pico e as mensagens agrupadas por padrão, com os padrões de erro destacados. Use antes do grep."
    }

    fn parameters(&self) -> Value {
        filter_schema()
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Ok(logs) = LogsDb::from_context(ctx) else {
            return Ok(logs_disabled());
        };
        let query = match build_query(ctx, args).await? {
            Ok(query) => query,
            Err(output) => return Ok(output),
        };
        let rows = repository::export(logs.connection(), &query, OVERVIEW_SAMPLE).await?;
        if rows.is_empty() {
            return Ok(ToolOutput::data(json!({
                "window": window(&query),
                "total": 0,
                "note": "Nenhum log na janela com esses filtros.",
            })));
        }
        let label = origin_labeler(ctx, &rows).await?;
        let digest = summarize(&rows, label);
        Ok(ToolOutput::data(json!({
            "window": window(&query),
            "sampled": rows.len() as u64 >= OVERVIEW_SAMPLE,
            "digest": digest,
        })))
    }
}
