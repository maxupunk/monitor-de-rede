//! Histórico de instabilidade por alvo (Fase 3 do roadmap, §4).
//!
//! Responde à pergunta que a tela do monitor e o dashboard fazem — "este link
//! é estável?" — com o número que o operador entende: **quantas vezes o alvo
//! caiu nas últimas N horas**.
//!
//! A fonte é `alert_events`, não `monitor_results`: o episódio já é a unidade
//! que atravessa a oscilação (Fase 1) e já conta as recaídas
//! (`data.recurrenceCount`), então a agregação sai de uma consulta só — e
//! funciona igual para monitores, interfaces e túneis, que é mais do que
//! `monitor_results` cobriria.
//!
//! A contagem é `episódios + recaídas`: cada episódio começou com uma queda, e
//! cada recaída é outra. Um alvo que abriu 2 episódios e recaiu 10 vezes
//! oscilou 12 vezes.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use serde_json::Value;

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events},
    services::{
        alerts::{
            contracts::{AlertScopeKey, AlertStatus},
            state_machine,
        },
        shared::errors::AppResult,
    },
};

/// Janela padrão da pergunta: "nas últimas 24 h".
pub const DEFAULT_HOURS: i64 = 24;

/// Teto da janela consultável — meses de histórico não cabem num indicador de
/// tela, e a consulta varre `alert_events`, que cresce sem poda até a Fase 4.
pub const MAX_HOURS: i64 = 24 * 30;

/// Quanto um alvo oscilou na janela consultada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeInstability {
    /// `monitor:12`, `interface:34`, `vpn_peer:7`.
    pub scope_key: String,
    /// Quedas na janela: episódios abertos + recaídas acumuladas.
    pub oscillations: u32,
    /// Episódios abertos na janela.
    pub episodes: u32,
    /// O alvo está declarado oscilante **agora** (evento aberto em `flapping`).
    pub flapping: bool,
    /// Último problema registrado entre os episódios da janela.
    pub last_problem_at: Option<String>,
}

/// Agrega os eventos por escopo. Pura: a consulta fica no chamador, para que a
/// contagem seja testável sem banco.
#[must_use]
pub fn summarize(events: &[alert_events::Model]) -> Vec<ScopeInstability> {
    let mut by_scope: HashMap<String, ScopeInstability> = HashMap::new();

    for event in events {
        // Evento sem escopo é de antes da `scope_key`; o id do monitor ainda o
        // identifica, e sem nenhum dos dois não há alvo a atribuir.
        let Some(scope_key) = event
            .scope_key
            .clone()
            .or_else(|| event.monitor_id.map(AlertScopeKey::monitor))
        else {
            continue;
        };

        let data = event.data.as_ref().and_then(Value::as_object);
        let entry = by_scope
            .entry(scope_key.clone())
            .or_insert_with(|| ScopeInstability {
                scope_key,
                oscillations: 0,
                episodes: 0,
                flapping: false,
                last_problem_at: None,
            });

        entry.episodes += 1;
        // O episódio em si é uma queda; as recaídas somam a partir dele.
        entry.oscillations +=
            1 + u32::try_from(state_machine::recurrence_count(data)).unwrap_or(u32::MAX);
        if event.status.parse() == Ok(AlertStatus::Flapping) {
            entry.flapping = true;
        }

        let problem_at = state_machine::last_problem_at(data, event.started_at.with_timezone(&Utc));
        if entry
            .last_problem_at
            .as_deref()
            .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
            .is_none_or(|current| problem_at > current)
        {
            entry.last_problem_at = Some(problem_at.to_rfc3339());
        }
    }

    let mut summary: Vec<ScopeInstability> = by_scope.into_values().collect();
    // Mais instável primeiro; empate desempata pelo nome, para a ordem ser
    // determinística (a iteração de um HashMap não é).
    summary.sort_by(|a, b| {
        b.oscillations
            .cmp(&a.oscillations)
            .then_with(|| a.scope_key.cmp(&b.scope_key))
    });
    summary
}

/// Os episódios que tocaram a janela, opcionalmente de um único alvo.
///
/// Um episódio ainda aberto conta mesmo tendo começado antes da janela: ele
/// **é** a instabilidade de agora.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn load<C: ConnectionTrait>(
    db: &C,
    hours: i64,
    scope_key: Option<&str>,
) -> AppResult<Vec<ScopeInstability>> {
    let since = Utc::now() - Duration::hours(hours.clamp(1, MAX_HOURS));
    let mut query = alert_events::Entity::find().filter(
        sea_orm::Condition::any()
            .add(alert_events_entity::Column::StartedAt.gte(since))
            .add(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN)),
    );
    if let Some(scope_key) = scope_key {
        query = query.filter(alert_events_entity::Column::ScopeKey.eq(scope_key));
    }
    Ok(summarize(&query.all(db).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evento(
        id: i64,
        scope_key: Option<&str>,
        status: &str,
        data: Option<Value>,
    ) -> alert_events::Model {
        let now = Utc::now().into();
        alert_events::Model {
            id,
            alert_rule_id: Some(1),
            device_id: None,
            monitor_id: Some(3),
            scope_key: scope_key.map(ToString::to_string),
            status: status.into(),
            severity: "warning".into(),
            started_at: now,
            resolved_at: None,
            message: None,
            data,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn as_oscilacoes_somam_episodios_e_recaidas() {
        // Dois episódios do mesmo alvo: 1 + 3 recaídas, 1 + 7 recaídas = 12.
        let eventos = [
            evento(
                1,
                Some("monitor:3"),
                "resolved",
                Some(json!({ "recurrenceCount": 3 })),
            ),
            evento(
                2,
                Some("monitor:3"),
                "recovering",
                Some(json!({ "recurrenceCount": 7 })),
            ),
        ];
        let resumo = summarize(&eventos);
        assert_eq!(resumo.len(), 1);
        assert_eq!(resumo[0].scope_key, "monitor:3");
        assert_eq!(resumo[0].oscillations, 12);
        assert_eq!(resumo[0].episodes, 2);
        assert!(!resumo[0].flapping);
    }

    #[test]
    fn um_evento_em_flapping_marca_o_alvo_como_oscilante_agora() {
        let eventos = [evento(1, Some("interface:9"), "flapping", None)];
        let resumo = summarize(&eventos);
        assert!(resumo[0].flapping);
        // Sem recaída registrada, o episódio ainda conta como uma queda.
        assert_eq!(resumo[0].oscillations, 1);
    }

    #[test]
    fn a_ordem_e_do_mais_instavel_para_o_menos() {
        let eventos = [
            evento(1, Some("monitor:1"), "resolved", None),
            evento(
                2,
                Some("monitor:2"),
                "resolved",
                Some(json!({ "recurrenceCount": 9 })),
            ),
            evento(
                3,
                Some("monitor:3"),
                "resolved",
                Some(json!({ "recurrenceCount": 4 })),
            ),
        ];
        let resumo = summarize(&eventos);
        let chaves: Vec<&str> = resumo.iter().map(|item| item.scope_key.as_str()).collect();
        assert_eq!(chaves, ["monitor:2", "monitor:3", "monitor:1"]);
    }

    #[test]
    fn evento_antigo_sem_scope_key_cai_no_alvo_do_monitor() {
        let eventos = [evento(1, None, "active", None)];
        assert_eq!(summarize(&eventos)[0].scope_key, "monitor:3");
    }

    #[test]
    fn o_ultimo_problema_e_o_mais_recente_entre_os_episodios() {
        let eventos = [
            evento(
                1,
                Some("monitor:3"),
                "resolved",
                Some(json!({ "lastProblemAt": "2026-08-15T10:00:00+00:00" })),
            ),
            evento(
                2,
                Some("monitor:3"),
                "resolved",
                Some(json!({ "lastProblemAt": "2026-08-15T11:30:00+00:00" })),
            ),
        ];
        assert_eq!(
            summarize(&eventos)[0].last_problem_at.as_deref(),
            Some("2026-08-15T11:30:00+00:00")
        );
    }
}
