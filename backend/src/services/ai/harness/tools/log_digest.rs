//! Leitura inteligente de logs para a IA: agrupar antes de ler.
//!
//! Mil linhas de "link down on ether3" diferem só no horário e num contador.
//! Trocando números, IPs e MACs por `#`, cada mensagem vira um padrão; a IA
//! recebe os padrões com contagem, severidade e um exemplo — o que custa
//! poucas dezenas de tokens em vez de milhares, e já aponta o que se repete.

use std::collections::HashMap;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde::Serialize;

use super::series::{serialize_rfc3339, serialize_rfc3339_opt};
use crate::{models::logs::device_logs, views::logs::severity_label};

/// Caracteres máximos de uma mensagem entregue à IA.
pub const MESSAGE_CHARS: usize = 240;
/// Caracteres máximos de um padrão.
const PATTERN_CHARS: usize = 120;
const TOP_PATTERNS: usize = 8;
const TOP_ERROR_PATTERNS: usize = 5;
const TOP_DEVICES: usize = 8;
const TOP_APPS: usize = 5;
const BUSIEST_HOURS: usize = 3;
/// Severidade numérica a partir da qual a linha conta como erro (erro,
/// crítico, alerta, emergência).
const ERROR_SEVERITY: i16 = 3;

/// Corta em `max` caracteres (não bytes), marcando o corte.
#[must_use]
pub fn truncate_chars(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(max).collect();
    cut.push('…');
    cut
}

/// Token que é só identificador variável: número, IP, MAC, hexadecimal.
fn is_variable_token(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_digit())
        && token
            .chars()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.' | '-' | '/' | 'x' | 'X'))
}

/// Padrão da mensagem: identificadores viram `#`, dígitos soltos também.
#[must_use]
pub fn normalize_message(message: &str) -> String {
    let normalized: Vec<String> = message
        .split_whitespace()
        .map(|token| {
            let core = token.trim_matches(|c: char| matches!(c, ',' | ';' | '(' | ')' | '[' | ']'));
            if !core.is_empty() && is_variable_token(core) {
                return token.replace(core, "#");
            }
            let mut out = String::with_capacity(token.len());
            let mut in_digits = false;
            for c in token.chars() {
                if c.is_ascii_digit() {
                    if !in_digits {
                        out.push('#');
                    }
                    in_digits = true;
                } else {
                    in_digits = false;
                    out.push(c);
                }
            }
            out
        })
        .collect();
    truncate_chars(&normalized.join(" "), PATTERN_CHARS)
}

/// Rótulo em português da severidade syslog (0–7).
#[must_use]
pub fn label_of(severity: Option<i16>) -> String {
    severity
        .and_then(severity_label)
        .map_or_else(|| "sem severidade".to_string(), str::to_string)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeverityCount {
    pub severity: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviceCount {
    pub device: String,
    pub count: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppCount {
    pub app: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PatternSummary {
    pub pattern: String,
    pub count: usize,
    /// A pior severidade vista no padrão.
    pub severity: String,
    pub devices: Vec<String>,
    pub example: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HourCount {
    pub hour: String,
    pub count: usize,
    pub errors: usize,
}

/// Visão geral de um conjunto de linhas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogDigest {
    pub total: usize,
    pub errors: usize,
    pub by_severity: Vec<SeverityCount>,
    pub by_source: HashMap<String, usize>,
    pub top_devices: Vec<DeviceCount>,
    pub top_apps: Vec<AppCount>,
    /// Padrões que mais se repetem, de qualquer severidade.
    pub top_patterns: Vec<PatternSummary>,
    /// Padrões de erro ou pior — o que costuma importar no diagnóstico.
    pub top_error_patterns: Vec<PatternSummary>,
    pub busiest_hours: Vec<HourCount>,
}

#[derive(Default)]
struct PatternAcc {
    count: usize,
    worst: Option<i16>,
    devices: Vec<String>,
    example: String,
    last_seen: Option<DateTime<Utc>>,
}

impl PatternAcc {
    fn summary(&self, pattern: &str) -> PatternSummary {
        PatternSummary {
            pattern: pattern.to_string(),
            count: self.count,
            severity: label_of(self.worst),
            devices: self.devices.clone(),
            example: self.example.clone(),
            last_seen: self.last_seen.map(|at| at.to_rfc3339()).unwrap_or_default(),
        }
    }
}

fn is_error(severity: Option<i16>) -> bool {
    severity.is_some_and(|value| value <= ERROR_SEVERITY)
}

/// Pior severidade: número menor é mais grave; sem severidade perde.
fn worst(a: Option<i16>, b: Option<i16>) -> Option<i16> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Resume as linhas. `device_label` diz como nomear a origem de cada uma.
pub fn summarize(
    rows: &[device_logs::Model],
    device_label: impl Fn(&device_logs::Model) -> String,
) -> LogDigest {
    let mut by_severity: HashMap<Option<i16>, usize> = HashMap::new();
    let mut by_source: HashMap<String, usize> = HashMap::new();
    let mut devices: HashMap<String, (usize, usize)> = HashMap::new();
    let mut apps: HashMap<String, usize> = HashMap::new();
    let mut patterns: HashMap<String, PatternAcc> = HashMap::new();
    let mut hours: HashMap<DateTime<Utc>, (usize, usize)> = HashMap::new();
    let mut errors = 0;

    for row in rows {
        let error = is_error(row.severity);
        errors += usize::from(error);
        *by_severity.entry(row.severity).or_default() += 1;
        *by_source.entry(row.source.clone()).or_default() += 1;

        let device = device_label(row);
        let slot = devices.entry(device.clone()).or_default();
        slot.0 += 1;
        slot.1 += usize::from(error);

        if let Some(app) = row.app_name.as_deref().filter(|app| !app.trim().is_empty()) {
            *apps.entry(app.trim().to_string()).or_default() += 1;
        }

        let at = row.received_at.with_timezone(&Utc);
        let acc = patterns.entry(normalize_message(&row.message)).or_default();
        acc.count += 1;
        acc.worst = worst(acc.worst, row.severity);
        if acc.devices.len() < 3 && !acc.devices.contains(&device) {
            acc.devices.push(device);
        }
        if acc.last_seen.is_none_or(|seen| at >= seen) {
            acc.last_seen = Some(at);
            acc.example = truncate_chars(&row.message, MESSAGE_CHARS);
        }

        let hour = at.duration_trunc(TimeDelta::hours(1)).unwrap_or(at);
        let bucket = hours.entry(hour).or_default();
        bucket.0 += 1;
        bucket.1 += usize::from(error);
    }

    let mut severities: Vec<(Option<i16>, usize)> = by_severity.into_iter().collect();
    severities.sort_by_key(|(severity, _)| severity.unwrap_or(i16::MAX));

    let mut top_devices: Vec<DeviceCount> = devices
        .into_iter()
        .map(|(device, (count, errors))| DeviceCount {
            device,
            count,
            errors,
        })
        .collect();
    top_devices.sort_by(|a, b| b.count.cmp(&a.count).then(a.device.cmp(&b.device)));
    top_devices.truncate(TOP_DEVICES);

    let mut top_apps: Vec<AppCount> = apps
        .into_iter()
        .map(|(app, count)| AppCount { app, count })
        .collect();
    top_apps.sort_by(|a, b| b.count.cmp(&a.count).then(a.app.cmp(&b.app)));
    top_apps.truncate(TOP_APPS);

    let mut ranked: Vec<(&String, &PatternAcc)> = patterns.iter().collect();
    ranked.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(b.0)));
    let top_patterns = ranked
        .iter()
        .take(TOP_PATTERNS)
        .map(|(pattern, acc)| acc.summary(pattern))
        .collect();
    let top_error_patterns = ranked
        .iter()
        .filter(|(_, acc)| is_error(acc.worst))
        .take(TOP_ERROR_PATTERNS)
        .map(|(pattern, acc)| acc.summary(pattern))
        .collect();

    let mut busiest: Vec<(DateTime<Utc>, (usize, usize))> = hours.into_iter().collect();
    busiest.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(b.0.cmp(&a.0)));
    busiest.truncate(BUSIEST_HOURS);

    LogDigest {
        total: rows.len(),
        errors,
        by_severity: severities
            .into_iter()
            .map(|(severity, count)| SeverityCount {
                severity: label_of(severity),
                count,
            })
            .collect(),
        by_source,
        top_devices,
        top_apps,
        top_patterns,
        top_error_patterns,
        busiest_hours: busiest
            .into_iter()
            .map(|(hour, (count, errors))| HourCount {
                hour: hour.to_rfc3339(),
                count,
                errors,
            })
            .collect(),
    }
}

/// Linha entregue na busca, já com repetições consecutivas colapsadas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollapsedLine {
    #[serde(serialize_with = "serialize_rfc3339")]
    pub at: DateTime<Utc>,
    pub device: String,
    pub severity: String,
    /// Severidade numérica (menor é mais grave), para quem ordena as linhas.
    #[serde(skip)]
    pub severity_value: Option<i16>,
    pub app: Option<String>,
    pub message: String,
    /// Quantas linhas seguidas tinham o mesmo padrão e origem (1 = única).
    pub repeated: usize,
    /// Instante da outra ponta do grupo (a mais antiga, na ordem "recentes
    /// primeiro" da busca), quando `repeated > 1`.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_rfc3339_opt"
    )]
    pub since: Option<DateTime<Utc>>,
}

/// Colapsa linhas consecutivas iguais (mesmo padrão e mesma origem),
/// preservando a ordem recebida.
pub fn collapse_repeats(
    rows: &[device_logs::Model],
    device_label: impl Fn(&device_logs::Model) -> String,
) -> Vec<CollapsedLine> {
    let labels: Vec<String> = rows.iter().map(&device_label).collect();
    group_consecutive(rows, |index, row| {
        (labels[index].clone(), normalize_message(&row.message))
    })
    .into_iter()
    .map(|group| {
        let first = &rows[group.start];
        let last = &rows[group.end - 1];
        CollapsedLine {
            at: first.received_at.with_timezone(&Utc),
            device: labels[group.start].clone(),
            severity: label_of(first.severity),
            severity_value: first.severity,
            app: first.app_name.clone(),
            message: truncate_chars(&first.message, MESSAGE_CHARS),
            repeated: group.len(),
            since: (group.len() > 1).then(|| last.received_at.with_timezone(&Utc)),
        }
    })
    .collect()
}

/// Faixas de itens consecutivos com a mesma chave — base do "×N" que
/// colapsa repetições. A ordem dos itens é preservada.
pub fn group_consecutive<T, K: PartialEq>(
    items: &[T],
    key: impl Fn(usize, &T) -> K,
) -> Vec<std::ops::Range<usize>> {
    let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
    let mut last_key: Option<K> = None;
    for (index, item) in items.iter().enumerate() {
        let current = key(index, item);
        match groups.last_mut() {
            Some(group) if last_key.as_ref() == Some(&current) => group.end = index + 1,
            _ => groups.push(index..index + 1),
        }
        last_key = Some(current);
    }
    groups
}

/// Severidade máxima pedida pela IA: número 0–7 ou nome (inglês ou português).
#[must_use]
pub fn parse_severity(raw: &str) -> Option<i16> {
    let raw = raw.trim().to_lowercase();
    if let Ok(value) = raw.parse::<i16>() {
        return (0..=7).contains(&value).then_some(value);
    }
    let value = match raw.as_str() {
        "emerg" | "emergency" | "emergência" | "emergencia" => 0,
        "alert" | "alerta" => 1,
        "crit" | "critical" | "crítico" | "critico" => 2,
        "err" | "error" | "erro" => 3,
        "warn" | "warning" | "aviso" => 4,
        "notice" | "notícia" | "noticia" => 5,
        "info" | "informational" | "informação" | "informacao" => 6,
        "debug" | "depuração" | "depuracao" => 7,
        _ => return None,
    };
    Some(value)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;

    fn linha(
        id: i64,
        minutos: i64,
        severity: Option<i16>,
        host: &str,
        message: &str,
    ) -> device_logs::Model {
        let base = Utc.with_ymd_and_hms(2026, 9, 21, 10, 0, 0).unwrap();
        let at = (base + Duration::minutes(minutos)).into();
        device_logs::Model {
            id,
            device_id: None,
            source_ip: host.into(),
            received_at: at,
            device_time: None,
            facility: None,
            severity,
            hostname: None,
            app_name: Some("kernel".into()),
            pid: None,
            topics: None,
            message: message.into(),
            source: "syslog".into(),
            created_at: at,
        }
    }

    fn origem(row: &device_logs::Model) -> String {
        row.source_ip.clone()
    }

    #[test]
    fn padrao_troca_identificadores_por_marcador() {
        assert_eq!(
            normalize_message("ether3 link down (10.0.0.1, aa:bb:cc:00:11:22) after 350ms"),
            "ether# link down (#, #) after #ms"
        );
        assert_eq!(
            normalize_message("login failure for user admin from 192.168.88.10 via ssh"),
            "login failure for user admin from # via ssh"
        );
        assert_eq!(
            normalize_message("dhcp  lease   renewed"),
            "dhcp lease renewed"
        );
    }

    #[test]
    fn corte_respeita_caracteres_multibyte() {
        assert_eq!(truncate_chars("ação", 10), "ação");
        assert_eq!(truncate_chars("ççççç", 3), "ççç…");
    }

    #[test]
    fn resumo_agrupa_padroes_e_destaca_erros() {
        let rows = vec![
            linha(1, 0, Some(6), "10.0.0.1", "dhcp lease 10.0.0.50 renewed"),
            linha(2, 1, Some(6), "10.0.0.1", "dhcp lease 10.0.0.51 renewed"),
            linha(3, 2, Some(6), "10.0.0.1", "dhcp lease 10.0.0.52 renewed"),
            linha(4, 3, Some(3), "10.0.0.2", "ether3 link down"),
            linha(5, 64, Some(3), "10.0.0.2", "ether3 link down"),
        ];
        let resumo = summarize(&rows, origem);

        assert_eq!(resumo.total, 5);
        assert_eq!(resumo.errors, 2);
        assert_eq!(resumo.top_patterns[0].pattern, "dhcp lease # renewed");
        assert_eq!(resumo.top_patterns[0].count, 3);
        assert_eq!(resumo.top_error_patterns.len(), 1);
        assert_eq!(resumo.top_error_patterns[0].pattern, "ether# link down");
        assert_eq!(resumo.top_error_patterns[0].severity, "erro");
        assert_eq!(
            resumo.by_severity[0].severity, "erro",
            "mais grave primeiro"
        );
        assert_eq!(resumo.top_devices[0].device, "10.0.0.1");
        assert_eq!(resumo.top_devices[1].errors, 2);
        assert_eq!(resumo.busiest_hours[0].count, 4);
        assert_eq!(resumo.top_apps[0].count, 5);
    }

    #[test]
    fn repeticoes_consecutivas_viram_uma_linha() {
        let rows = vec![
            linha(3, 2, Some(3), "10.0.0.2", "ether3 link down"),
            linha(2, 1, Some(3), "10.0.0.2", "ether3 link down"),
            linha(1, 0, Some(6), "10.0.0.2", "ether3 link up"),
        ];
        let linhas = collapse_repeats(&rows, origem);
        assert_eq!(linhas.len(), 2);
        assert_eq!(linhas[0].repeated, 2);
        assert!(linhas[0].since.is_some());
        assert_eq!(linhas[1].repeated, 1);
    }

    #[test]
    fn agrupa_so_vizinhos_iguais() {
        let grupos = group_consecutive(&["a", "a", "b", "a"], |_, item| *item);
        assert_eq!(grupos, vec![0..2, 2..3, 3..4]);
        assert!(group_consecutive(&[] as &[&str], |_, item| *item).is_empty());
    }

    #[test]
    fn severidade_por_numero_ou_nome() {
        assert_eq!(parse_severity("3"), Some(3));
        assert_eq!(parse_severity("Warning"), Some(4));
        assert_eq!(parse_severity("erro"), Some(3));
        assert_eq!(parse_severity("9"), None);
        assert_eq!(parse_severity("qualquer"), None);
    }
}
