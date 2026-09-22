//! Linha do tempo de um incidente: alertas, mudanças de status de monitor e
//! logs relevantes, numa ordem só.
//!
//! O limite de entradas protege os tokens: alertas e mudanças de status
//! sempre entram; logs preenchem o que sobra, dos mais graves para os menos.

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::series::serialize_rfc3339;
use crate::models::monitor_results;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    AlertOpened,
    AlertResolved,
    StatusChange,
    Log,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimelineEntry {
    #[serde(serialize_with = "serialize_rfc3339")]
    pub at: DateTime<Utc>,
    pub kind: EntryKind,
    /// Dispositivo, monitor ou origem do log.
    pub source: String,
    pub detail: String,
    /// Gravidade para ordenar o corte dos logs (menor é mais grave).
    #[serde(skip)]
    pub rank: i16,
}

/// Mudanças de status de um monitor, a partir das checagens em ordem
/// cronológica. A primeira checagem só vira entrada se já não estiver `up`.
#[must_use]
pub fn status_transitions(
    monitor_name: &str,
    results: &[monitor_results::Model],
) -> Vec<TimelineEntry> {
    let mut entries = Vec::new();
    let mut previous: Option<&str> = None;
    for result in results {
        let status = result.status.as_str();
        let changed = match previous {
            Some(before) => before != status,
            None => status != "up",
        };
        if changed {
            let detail = match (previous, result.message.as_deref()) {
                (Some(before), Some(message)) => format!("{before} → {status}: {message}"),
                (Some(before), None) => format!("{before} → {status}"),
                (None, Some(message)) => format!("{status}: {message}"),
                (None, None) => status.to_string(),
            };
            entries.push(TimelineEntry {
                at: result.started_at.with_timezone(&Utc),
                kind: EntryKind::StatusChange,
                source: monitor_name.to_string(),
                detail,
                rank: 0,
            });
        }
        previous = Some(status);
    }
    entries
}

/// Junta as fontes, aplica o limite e devolve em ordem cronológica.
#[must_use]
pub fn merge(entries: Vec<TimelineEntry>, cap: usize) -> (Vec<TimelineEntry>, usize) {
    let total = entries.len();
    let (mut kept, mut logs): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|entry| entry.kind != EntryKind::Log);
    kept.sort_by_key(|entry| entry.at);
    kept.truncate(cap);
    logs.sort_by(|a, b| a.rank.cmp(&b.rank).then(b.at.cmp(&a.at)));
    let room = cap.saturating_sub(kept.len());
    kept.extend(logs.into_iter().take(room));
    kept.sort_by_key(|entry| entry.at);
    let omitted = total - kept.len();
    (kept, omitted)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;

    fn at(minutos: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 21, 14, 0, 0).unwrap() + Duration::minutes(minutos)
    }

    fn checagem(minutos: i64, status: &str, message: Option<&str>) -> monitor_results::Model {
        let quando = at(minutos).into();
        monitor_results::Model {
            id: minutos,
            monitor_id: 1,
            probe_id: None,
            status: status.into(),
            started_at: quando,
            finished_at: quando,
            duration_ms: 10,
            latency_ms: None,
            message: message.map(Into::into),
            data: None,
            created_at: quando,
        }
    }

    fn entrada(minutos: i64, kind: EntryKind, rank: i16) -> TimelineEntry {
        TimelineEntry {
            at: at(minutos),
            kind,
            source: "x".into(),
            detail: format!("{minutos}"),
            rank,
        }
    }

    #[test]
    fn so_mudancas_de_status_viram_entrada() {
        let results = vec![
            checagem(0, "up", None),
            checagem(1, "up", None),
            checagem(2, "down", Some("Timeout")),
            checagem(3, "down", Some("Timeout")),
            checagem(4, "up", None),
        ];
        let entries = status_transitions("Ping Borda", &results);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].detail, "up → down: Timeout");
        assert_eq!(entries[1].detail, "down → up");
    }

    #[test]
    fn primeira_checagem_fora_do_ar_ja_conta() {
        let entries = status_transitions("Ping", &[checagem(0, "down", None)]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].detail, "down");
    }

    #[test]
    fn corte_preserva_alertas_e_prefere_logs_graves() {
        let entries = vec![
            entrada(5, EntryKind::Log, 6),
            entrada(1, EntryKind::AlertOpened, 0),
            entrada(3, EntryKind::Log, 3),
            entrada(4, EntryKind::Log, 6),
            entrada(2, EntryKind::StatusChange, 0),
        ];
        let (kept, omitted) = merge(entries, 3);
        let details: Vec<&str> = kept.iter().map(|e| e.detail.as_str()).collect();
        assert_eq!(
            details,
            vec!["1", "2", "3"],
            "cronológico, com o log de erro"
        );
        assert_eq!(omitted, 2);
    }
}
