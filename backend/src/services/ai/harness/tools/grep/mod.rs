//! `grep` da IA: procurar nos dados sem trazer os dados.
//!
//! A IA descreve o que procura (padrão literal ou regex, exclusão, fonte,
//! dispositivo, janela) e escolhe **quanto** quer receber — contagem,
//! origens, padrões ou linhas. O servidor lê a janela, casa, e devolve só a
//! forma pedida. O padrão `auto` devolve as linhas quando cabem e, quando
//! não, um resumo com a orientação de como refinar.
//!
//! - [`matcher`]: o que casa (e o trecho da mensagem que importa).
//! - [`engine`]: as formas de saída, puras.
//! - [`sources`]: de onde ler (logs, alertas, falhas de checagem, Docker).

mod engine;
mod matcher;
mod sources;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use self::{
    engine::{
        auto_summary, count_view, fits_in_lines, lines_view, sources_view, top_patterns,
        ContextLine, GrepLine, GrepRecord, OutputMode,
    },
    matcher::Matcher,
    sources::{source_for, GrepFilter, GrepSource, SOURCE_NAMES},
};
use super::{
    log_digest::parse_severity, logs::MAX_HOURS, lookup::find_device, AiToolHandler, ToolArgs,
    ToolOutput,
};
use crate::services::shared::errors::AppResult;

const DEFAULT_MAX_LINES: i64 = 15;
const MAX_LINES: i64 = 50;
const MAX_CONTEXT: i64 = 3;
/// Linhas que ganham contexto: cada uma custa duas consultas.
const MAX_LINES_WITH_CONTEXT: usize = 10;

/// Linhas com o `-C` preenchido, quando a fonte tem contexto.
async fn attach_context(
    ctx: &AppContext,
    source: &dyn GrepSource,
    matcher: &Matcher,
    lines: &mut [GrepLine],
    context: u64,
) -> AppResult<()> {
    if context == 0 {
        return Ok(());
    }
    let to_line = |records: Vec<GrepRecord>| {
        records
            .iter()
            .map(|r| ContextLine::from_record(r, matcher))
            .collect()
    };
    for line in lines.iter_mut().take(MAX_LINES_WITH_CONTEXT) {
        if let Some(anchor) = line.anchor.as_ref() {
            let (before, after) = source.context(ctx, anchor, context).await?;
            line.before = to_line(before);
            line.after = to_line(after);
        }
    }
    Ok(())
}

pub struct Grep;

#[async_trait]
impl AiToolHandler for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Busca texto sem trazer os dados inteiros. source: logs (syslog e aplicação, padrão), alerts, checks (falhas) ou docker (exige container). output: count (totais por origem e hora) e sources (quem) para medir primeiro; patterns (mensagens distintas); lines (mais recentes); auto."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Vazio: tudo" },
                "regex": { "type": "boolean", "description": "pattern/exclude como regex" },
                "exclude": { "type": "string", "description": "Descarta linhas com este texto" },
                "source": { "type": "string", "enum": SOURCE_NAMES },
                "output": { "type": "string", "enum": ["auto", "count", "sources", "patterns", "lines"] },
                "device": { "type": "string", "description": "Nome, IP ou id" },
                "container": { "type": "string", "description": "Só docker" },
                "host": { "type": "string", "description": "Só docker; padrão: central" },
                "severity": { "type": "string", "description": "Só logs: a máxima (error, warning, info)" },
                "hours": { "type": "integer", "description": "1–168, padrão 24" },
                "max_lines": { "type": "integer", "description": "1–50, padrão 15" },
                "context": { "type": "integer", "description": "Só logs: 0–3 linhas ao redor" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let source_name = args.text("source").unwrap_or_else(|| "logs".into());
        let Some(source) = source_for(&source_name) else {
            return Ok(ToolOutput::not_found(format!(
                "Fonte '{source_name}' desconhecida; use {}",
                SOURCE_NAMES.join(", ")
            )));
        };
        let regex = args.flag("regex");
        let pattern = args.text("pattern");
        let matcher = match Matcher::new(pattern.as_deref(), args.text("exclude").as_deref(), regex)
        {
            Ok(matcher) => matcher,
            Err(message) => return Ok(ToolOutput::not_found(message)),
        };
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
        let severity = match args.text("severity") {
            Some(raw) => match parse_severity(&raw) {
                Some(value) => Some(value),
                None => {
                    return Ok(ToolOutput::not_found(format!(
                        "Severidade '{raw}' inválida; use error, warning, notice, info ou 0-7"
                    )))
                }
            },
            None => None,
        };
        let until = Utc::now();
        let since = until - Duration::hours(args.integer_in("hours", 24, 1, MAX_HOURS));
        let filter = GrepFilter {
            device,
            since,
            until,
            severity,
            // Só texto literal ajuda o banco: regex é casada em memória.
            needle: pattern.filter(|_| !regex),
            container: args.text("container"),
            host: args.text("host"),
        };

        let scan = match source.scan(ctx, &filter).await? {
            Ok(scan) => scan,
            Err(message) => return Ok(ToolOutput::not_found(message)),
        };
        let scanned = scan.records.len();
        let matched: Vec<GrepRecord> = scan
            .records
            .into_iter()
            .filter(|record| matcher.is_match(&record.text))
            .collect();

        let max_lines = args.integer_in("max_lines", DEFAULT_MAX_LINES, 1, MAX_LINES) as usize;
        let mode = OutputMode::parse(args.text("output").as_deref());
        let mode = match mode {
            OutputMode::Auto if fits_in_lines(&matched, max_lines) => OutputMode::Lines,
            other => other,
        };

        let mut data = json!({
            "source": source_name,
            "window": { "from": since.to_rfc3339(), "to": until.to_rfc3339() },
            "scanned": scanned,
            "matched": matched.len(),
            "output": match mode {
                OutputMode::Auto => "summary",
                OutputMode::Count => "count",
                OutputMode::Sources => "sources",
                OutputMode::Patterns => "patterns",
                OutputMode::Lines => "lines",
            },
        });
        if scan.capped {
            data["capped"] = json!(true);
            data["note"] = json!(
                "A janela tem mais registros que o teto de leitura; o resultado cobre só os mais recentes. Reduza hours ou filtre por device."
            );
        }

        match mode {
            OutputMode::Count => data["count"] = count_view(&matched),
            OutputMode::Sources => data["sources"] = json!(sources_view(&matched)),
            OutputMode::Patterns => data["patterns"] = json!(top_patterns(&matched, &matcher)),
            OutputMode::Auto => data["summary"] = auto_summary(&matched, &matcher),
            OutputMode::Lines => {
                let (mut lines, omitted) = lines_view(&matched, &matcher, max_lines);
                let context = args.integer_in("context", 0, 0, MAX_CONTEXT) as u64;
                attach_context(ctx, source.as_ref(), &matcher, &mut lines, context).await?;
                data["lines"] = json!(lines);
                if omitted > 0 {
                    data["omitted_lines"] = json!(omitted);
                }
            }
        }
        Ok(ToolOutput::data(data))
    }
}
