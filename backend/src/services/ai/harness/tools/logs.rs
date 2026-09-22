//! Logs da tela `/logs` (syslog dos equipamentos e eventos da aplicação).
//!
//! Duas ferramentas, na ordem em que a IA deve usá-las: a visão geral agrupa
//! milhares de linhas em padrões; a busca traz as linhas de um padrão ou de
//! um termo, com as repetições consecutivas colapsadas.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use super::{
    log_digest::{collapse_repeats, parse_severity, summarize},
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
const DEFAULT_SEARCH_LIMIT: i64 = 20;
const MAX_SEARCH_LIMIT: i64 = 50;
/// Mesma janela máxima da tela de logs.
const MAX_HOURS: i64 = repository::MAX_WINDOW_DAYS * 24;

fn filter_schema(extra: Value) -> Value {
    let mut properties = json!({
        "device": { "type": "string", "description": "Nome, IP ou id do dispositivo (omitido: todos)" },
        "severity": { "type": "string", "description": "Severidade máxima: error, warning, notice, info... ou 0-7. 'error' traz erro, crítico, alerta e emergência." },
        "query": { "type": "string", "description": "Termo a procurar na mensagem (ex: 'link down', 'login failure')" },
        "hours": { "type": "integer", "description": "Janela em horas (1 a 168, padrão 24)" }
    });
    if let (Some(base), Some(more)) = (properties.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    json!({ "type": "object", "properties": properties })
}

/// Filtro resolvido a partir dos argumentos, ou a mensagem para a IA.
async fn build_query(
    ctx: &AppContext,
    args: &ToolArgs,
    limit: Option<u64>,
) -> AppResult<Result<LogQuery, ToolOutput>> {
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
            limit,
            ..LogFilters::default()
        },
        now,
    )))
}

/// Como nomear a origem de cada linha: dispositivo cadastrado, a própria
/// aplicação ou, sem vínculo, o hostname/IP que chegou no pacote.
async fn origin_labeler(
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

fn logs_disabled() -> ToolOutput {
    ToolOutput::not_found(
        "A coleta de logs está desligada neste servidor (SYSLOG_ENABLED=false); não há logs para consultar.",
    )
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
        "Visão geral dos logs (syslog dos equipamentos e da aplicação) numa janela: contagem por severidade, dispositivos e apps que mais registram, horas de pico e as mensagens agrupadas por padrão, com os padrões de erro destacados. Use antes de search_logs."
    }

    fn parameters(&self) -> Value {
        filter_schema(json!({}))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Ok(logs) = LogsDb::from_context(ctx) else {
            return Ok(logs_disabled());
        };
        let query = match build_query(ctx, args, None).await? {
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

pub struct SearchLogs;

#[async_trait]
impl AiToolHandler for SearchLogs {
    fn name(&self) -> &'static str {
        "search_logs"
    }

    fn description(&self) -> &'static str {
        "Busca linhas de log (mais recentes primeiro) por termo, dispositivo, severidade e janela. Linhas repetidas em sequência vêm colapsadas com a contagem."
    }

    fn parameters(&self) -> Value {
        filter_schema(json!({
            "limit": { "type": "integer", "description": "Máximo de linhas lidas (1 a 50, padrão 20)" }
        }))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Ok(logs) = LogsDb::from_context(ctx) else {
            return Ok(logs_disabled());
        };
        let limit = args.integer_in("limit", DEFAULT_SEARCH_LIMIT, 1, MAX_SEARCH_LIMIT) as u64;
        let query = match build_query(ctx, args, Some(limit)).await? {
            Ok(query) => query,
            Err(output) => return Ok(output),
        };
        let page = repository::search(logs.connection(), &query).await?;
        let label = origin_labeler(ctx, &page.rows).await?;
        let lines = collapse_repeats(&page.rows, label);
        Ok(ToolOutput::data(json!({
            "window": window(&query),
            "read": page.rows.len(),
            "has_more": page.next_cursor.is_some(),
            "lines": lines,
        })))
    }
}
