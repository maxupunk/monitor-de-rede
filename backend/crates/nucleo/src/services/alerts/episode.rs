//! Escrita compartilhada do episódio de alerta.
//!
//! Manager e recovery chegam à mesma recaída por caminhos diferentes — a
//! condição da regra batendo (`manager`) ou a degradação da Fase 2
//! (`recovery::note_degraded_scope`) — e precisam gravar exatamente o mesmo
//! conjunto de campos: contador, carimbo do último problema e linha do tempo
//! da detecção de flapping. Esta duplicação viveu solta durante a Fase 2; a
//! Fase 3, que acrescenta um terceiro campo à mesma escrita, é o momento de
//! reuni-la num lugar só.
//!
//! Aqui não há decisão nenhuma: quem decide é a [`state_machine`]. Este módulo
//! só materializa a transição já escolhida.

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Map, Value};

use crate::{
    models::alert_events,
    services::{
        alerts::{
            contracts::AlertStatus,
            state_machine::{self, EpisodePolicy},
        },
        shared::errors::AppResult,
    },
};

/// Carimba no `data` tudo que uma recaída acrescenta ao episódio.
///
/// `lastProblemAt` reinicia a janela de estabilidade (Fase 1), o contador
/// acumula a recaída (Fase 1) e a linha do tempo alimenta a janela deslizante
/// da detecção de flapping (Fase 3) — esta última só quando a regra a
/// configurou.
pub fn mark_relapse(
    data: &mut Map<String, Value>,
    recurrence: u64,
    now: DateTime<Utc>,
    policy: EpisodePolicy,
) {
    data.insert(state_machine::RECURRENCE_COUNT.into(), json!(recurrence));
    data.insert(
        state_machine::LAST_PROBLEM_AT.into(),
        json!(now.to_rfc3339()),
    );
    state_machine::record_transition(data, now, policy);
}

/// Grava status e `data` do evento, devolvendo a linha recarregada.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn persist(
    ctx: &AppContext,
    event: &alert_events::Model,
    status: AlertStatus,
    data: Map<String, Value>,
) -> AppResult<alert_events::Model> {
    let mut active: alert_events::ActiveModel = event.clone().into();
    active.status = Set(status.as_str().into());
    active.data = Set(Some(Value::Object(data)));
    Ok(active.update(&ctx.db).await?)
}

/// Rótulo do episódio para as notificações: o título gravado no disparo, que
/// sobrevive à regra apagada; na falta dele, o que o chamador souber do alvo.
#[must_use]
pub fn title_of(event: &alert_events::Model, fallback: &str) -> String {
    event
        .data
        .as_ref()
        .and_then(|data| data.get("title"))
        .and_then(Value::as_str)
        .map_or_else(|| fallback.to_string(), ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn evento(data: Option<Value>) -> alert_events::Model {
        let now = Utc::now().into();
        alert_events::Model {
            id: 1,
            alert_rule_id: Some(1),
            device_id: None,
            monitor_id: Some(3),
            scope_key: Some("monitor:3".into()),
            status: "active".into(),
            severity: "warning".into(),
            started_at: now,
            resolved_at: None,
            message: Some("Host inacessível".into()),
            data,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn a_recaida_carimba_contador_e_ultimo_problema() {
        let mut data = Map::new();
        mark_relapse(&mut data, 4, t0(), EpisodePolicy::default());
        assert_eq!(data[state_machine::RECURRENCE_COUNT], json!(4));
        assert_eq!(
            data[state_machine::LAST_PROBLEM_AT],
            json!(t0().to_rfc3339())
        );
        // Sem detecção configurada, a linha do tempo não é gravada.
        assert!(!data.contains_key(state_machine::PROBLEM_TIMELINE));
    }

    #[test]
    fn com_deteccao_ligada_a_recaida_entra_na_linha_do_tempo() {
        let policy = EpisodePolicy {
            recovery_window_seconds: 300,
            flap_threshold: 3,
            flap_window_seconds: 900,
        };
        let mut data = Map::new();
        mark_relapse(&mut data, 1, t0(), policy);
        mark_relapse(&mut data, 2, t0() + Duration::seconds(60), policy);
        assert_eq!(state_machine::timeline(Some(&data)).len(), 2);
    }

    #[test]
    fn o_titulo_gravado_no_disparo_vence_o_fallback() {
        assert_eq!(
            title_of(&evento(Some(json!({ "title": "Regra — Alvo" }))), "outro"),
            "Regra — Alvo"
        );
        assert_eq!(title_of(&evento(None), "outro"), "outro");
    }
}
