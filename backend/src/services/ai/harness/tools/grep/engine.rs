//! Formas de devolver o que casou, da mais barata para a mais cara.
//!
//! | modo       | equivalente | custa                         | serve para                      |
//! |------------|-------------|-------------------------------|---------------------------------|
//! | `count`    | `grep -c`   | dezenas de tokens             | "quanto e quando?"              |
//! | `sources`  | `grep -l`   | uma linha por origem          | "onde?"                         |
//! | `patterns` | —           | uma linha por padrão          | "o quê, sem repetição?"         |
//! | `lines`    | `grep`      | uma linha por ocorrência      | ler as mensagens de verdade     |
//! | `auto`     | —           | `lines` se couber, senão resumo | padrão: nunca devolve enxurrada |

use std::collections::HashMap;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use super::{
    super::{
        log_digest::{group_consecutive, normalize_message},
        series::{serialize_rfc3339, serialize_rfc3339_opt},
    },
    matcher::Matcher,
};

/// Caracteres de uma mensagem devolvida em `lines`.
pub const LINE_CHARS: usize = 240;
const TOP_ORIGINS: usize = 10;
const TOP_PATTERNS: usize = 10;
const AUTO_PATTERNS: usize = 5;
const MAX_HOURS_LISTED: usize = 48;

/// Uma ocorrência vinda de qualquer fonte (log, alerta, checagem).
#[derive(Debug, Clone, PartialEq)]
pub struct GrepRecord {
    pub at: DateTime<Utc>,
    /// Dispositivo, monitor ou IP de origem.
    pub origin: String,
    pub severity: String,
    /// Gravidade para ordenar (menor é pior).
    pub rank: i16,
    pub text: String,
    /// Id no banco de logs, para buscar o contexto (`-C`).
    pub log_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Auto,
    Count,
    Sources,
    Patterns,
    Lines,
}

impl OutputMode {
    #[must_use]
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::to_lowercase).as_deref() {
            Some("count") => Self::Count,
            Some("sources") => Self::Sources,
            Some("patterns") => Self::Patterns,
            Some("lines") => Self::Lines,
            _ => Self::Auto,
        }
    }
}

fn worst<'a>(records: impl Iterator<Item = &'a GrepRecord>) -> Option<&'a GrepRecord> {
    records.min_by_key(|record| record.rank)
}

/// `grep -c`, com a distribuição no tempo: quando começou e quando parou.
#[must_use]
pub fn count_view(matched: &[GrepRecord]) -> Value {
    let mut by_origin: HashMap<&str, usize> = HashMap::new();
    let mut by_hour: HashMap<DateTime<Utc>, usize> = HashMap::new();
    for record in matched {
        *by_origin.entry(record.origin.as_str()).or_default() += 1;
        let hour = record
            .at
            .duration_trunc(TimeDelta::hours(1))
            .unwrap_or(record.at);
        *by_hour.entry(hour).or_default() += 1;
    }
    let mut origins: Vec<(&str, usize)> = by_origin.into_iter().collect();
    origins.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let mut hours: Vec<(DateTime<Utc>, usize)> = by_hour.into_iter().collect();
    hours.sort_by_key(|(hour, _)| *hour);
    let hours_omitted = hours.len().saturating_sub(MAX_HOURS_LISTED);
    let hours = &hours[hours_omitted..];

    json!({
        "matched": matched.len(),
        "first": matched.first().map(|record| record.at.to_rfc3339()),
        "last": matched.last().map(|record| record.at.to_rfc3339()),
        "worst_severity": worst(matched.iter()).map(|record| &record.severity),
        "by_origin": origins
            .iter()
            .take(TOP_ORIGINS)
            .map(|(origin, count)| json!({ "origin": origin, "count": count }))
            .collect::<Vec<_>>(),
        "by_hour": hours
            .iter()
            .map(|(hour, count)| json!({ "hour": hour.to_rfc3339(), "count": count }))
            .collect::<Vec<_>>(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OriginSummary {
    pub origin: String,
    pub count: usize,
    pub worst_severity: String,
    #[serde(serialize_with = "serialize_rfc3339")]
    pub first: DateTime<Utc>,
    #[serde(serialize_with = "serialize_rfc3339")]
    pub last: DateTime<Utc>,
}

/// `grep -l`: quais origens casaram, com contagem e período.
#[must_use]
pub fn sources_view(matched: &[GrepRecord]) -> Vec<OriginSummary> {
    let mut groups: HashMap<&str, Vec<&GrepRecord>> = HashMap::new();
    for record in matched {
        groups
            .entry(record.origin.as_str())
            .or_default()
            .push(record);
    }
    let mut out: Vec<OriginSummary> = groups
        .into_iter()
        .filter_map(|(origin, records)| {
            Some(OriginSummary {
                origin: origin.to_string(),
                count: records.len(),
                worst_severity: worst(records.iter().copied())?.severity.clone(),
                first: records.first()?.at,
                last: records.last()?.at,
            })
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.origin.cmp(&b.origin)));
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PatternCount {
    pub pattern: String,
    pub count: usize,
    pub worst_severity: String,
    pub origins: Vec<String>,
    pub example: String,
    #[serde(serialize_with = "serialize_rfc3339")]
    pub last_seen: DateTime<Utc>,
}

/// O que casou, sem repetição: mensagens iguais a menos de números e IPs
/// viram um padrão com contagem.
#[must_use]
pub fn patterns_view(matched: &[GrepRecord], matcher: &Matcher, limit: usize) -> Vec<PatternCount> {
    let mut groups: HashMap<String, Vec<&GrepRecord>> = HashMap::new();
    for record in matched {
        groups
            .entry(normalize_message(&record.text))
            .or_default()
            .push(record);
    }
    let mut out: Vec<PatternCount> = groups
        .into_iter()
        .filter_map(|(pattern, records)| {
            let last = records.last()?;
            let mut origins: Vec<String> = Vec::new();
            for record in &records {
                if origins.len() < 3 && !origins.contains(&record.origin) {
                    origins.push(record.origin.clone());
                }
            }
            Some(PatternCount {
                pattern,
                count: records.len(),
                worst_severity: worst(records.iter().copied())?.severity.clone(),
                origins,
                example: matcher.snippet(&last.text, LINE_CHARS),
                last_seen: last.at,
            })
        })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.pattern.cmp(&b.pattern)));
    out.truncate(limit);
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContextLine {
    #[serde(serialize_with = "serialize_rfc3339")]
    pub at: DateTime<Utc>,
    pub severity: String,
    pub text: String,
}

impl ContextLine {
    #[must_use]
    pub fn from_record(record: &GrepRecord, matcher: &Matcher) -> Self {
        Self {
            at: record.at,
            severity: record.severity.clone(),
            text: matcher.snippet(&record.text, LINE_CHARS),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GrepLine {
    #[serde(serialize_with = "serialize_rfc3339")]
    pub at: DateTime<Utc>,
    pub origin: String,
    pub severity: String,
    pub text: String,
    #[serde(skip_serializing_if = "is_one")]
    pub repeated: usize,
    /// A ocorrência mais antiga do grupo repetido.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_rfc3339_opt"
    )]
    pub since: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub before: Vec<ContextLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<ContextLine>,
    /// Registro de onde a linha saiu (para buscar contexto); não vai à IA.
    #[serde(skip)]
    pub anchor: Option<GrepRecord>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_one(value: &usize) -> bool {
    *value == 1
}

/// As linhas mais recentes, repetições seguidas colapsadas (`×N`). Devolve
/// também quantas linhas colapsadas ficaram de fora do teto.
#[must_use]
pub fn lines_view(
    matched: &[GrepRecord],
    matcher: &Matcher,
    max_lines: usize,
) -> (Vec<GrepLine>, usize) {
    let newest_first: Vec<&GrepRecord> = matched.iter().rev().collect();
    let groups = group_consecutive(&newest_first, |_, record| {
        (record.origin.clone(), normalize_message(&record.text))
    });
    let omitted = groups.len().saturating_sub(max_lines);
    let lines = groups
        .into_iter()
        .take(max_lines)
        .map(|group| {
            let newest = newest_first[group.start];
            let oldest = newest_first[group.end - 1];
            GrepLine {
                at: newest.at,
                origin: newest.origin.clone(),
                severity: newest.severity.clone(),
                text: matcher.snippet(&newest.text, LINE_CHARS),
                repeated: group.len(),
                since: (group.len() > 1).then_some(oldest.at),
                before: Vec::new(),
                after: Vec::new(),
                anchor: Some(newest.clone()),
            }
        })
        .collect();
    (lines, omitted)
}

/// `auto`: as linhas quando cabem; senão contagem + principais padrões e a
/// orientação de como refinar — a IA nunca recebe uma enxurrada.
#[must_use]
pub fn fits_in_lines(matched: &[GrepRecord], max_lines: usize) -> bool {
    matched.len() <= max_lines
}

/// Resumo do modo `auto` quando as linhas não cabem.
#[must_use]
pub fn auto_summary(matched: &[GrepRecord], matcher: &Matcher) -> Value {
    let mut summary = count_view(matched);
    summary["top_patterns"] = json!(patterns_view(matched, matcher, AUTO_PATTERNS));
    summary["hint"] = json!(
        "Resultado grande: refine com device, hours ou um padrão mais específico, ou peça output='lines' (as mais recentes) ou output='patterns'."
    );
    summary
}

/// Padrões do modo `patterns`.
#[must_use]
pub fn top_patterns(matched: &[GrepRecord], matcher: &Matcher) -> Vec<PatternCount> {
    patterns_view(matched, matcher, TOP_PATTERNS)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;

    fn registro(minutos: i64, origin: &str, rank: i16, text: &str) -> GrepRecord {
        GrepRecord {
            at: Utc.with_ymd_and_hms(2026, 9, 22, 10, 0, 0).unwrap() + Duration::minutes(minutos),
            origin: origin.into(),
            severity: if rank <= 3 { "erro" } else { "aviso" }.into(),
            rank,
            text: text.into(),
            log_id: None,
        }
    }

    fn amostra() -> Vec<GrepRecord> {
        vec![
            registro(0, "borda", 4, "ether1 link down"),
            registro(5, "borda", 3, "ether1 link down"),
            registro(70, "core", 3, "sfp2 link down"),
            registro(71, "borda", 4, "ether1 link down"),
            registro(72, "borda", 4, "ether1 link down"),
        ]
    }

    #[test]
    fn modo_de_saida_tem_padrao_auto() {
        assert_eq!(OutputMode::parse(Some("COUNT")), OutputMode::Count);
        assert_eq!(OutputMode::parse(Some("x")), OutputMode::Auto);
        assert_eq!(OutputMode::parse(None), OutputMode::Auto);
    }

    #[test]
    fn contagem_por_origem_e_por_hora() {
        let visao = count_view(&amostra());
        assert_eq!(visao["matched"], 5);
        assert_eq!(visao["worst_severity"], "erro");
        assert_eq!(
            visao["by_origin"][0],
            json!({ "origin": "borda", "count": 4 })
        );
        assert_eq!(visao["by_hour"].as_array().unwrap().len(), 2);
        assert_eq!(visao["by_hour"][1]["count"], 3);
    }

    #[test]
    fn origens_com_periodo() {
        let origens = sources_view(&amostra());
        assert_eq!(origens[0].origin, "borda");
        assert_eq!(origens[0].count, 4);
        assert_eq!(origens[1].worst_severity, "erro");
    }

    #[test]
    fn linhas_mais_recentes_primeiro_com_repeticao_colapsada() {
        let matcher = Matcher::new(Some("link down"), None, false).unwrap();
        let (linhas, omitidas) = lines_view(&amostra(), &matcher, 2);
        assert_eq!(linhas[0].origin, "borda");
        assert_eq!(linhas[0].repeated, 2, "as duas últimas da borda viram uma");
        assert_eq!(linhas[1].origin, "core");
        assert_eq!(omitidas, 1);

        let json = serde_json::to_value(&linhas[1]).unwrap();
        assert!(json.get("repeated").is_none(), "1 não ocupa token");
        assert!(json.get("anchor").is_none());
    }

    #[test]
    fn auto_resume_quando_nao_cabe() {
        let matcher = Matcher::new(Some("link"), None, false).unwrap();
        assert!(fits_in_lines(&amostra(), 5));
        assert!(!fits_in_lines(&amostra(), 4));
        let resumo = auto_summary(&amostra(), &matcher);
        assert_eq!(resumo["top_patterns"][0]["pattern"], "ether# link down");
        assert_eq!(resumo["top_patterns"][0]["count"], 4);
        assert!(resumo["hint"].is_string());
    }
}
