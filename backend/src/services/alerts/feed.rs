//! Publicação SSE dos eventos de alerta (§8.7).
//!
//! Três tipos alimentam a Central de Alertas em tempo real:
//!
//! - `alert:triggered` — evento novo; o payload completo mais o `targetLabel`
//!   do contexto do disparo;
//! - `alert:updated` — transição `recovering`↔`active` (recaída) ou contador
//!   de recaídas atualizado. Mesmo formato do `triggered`, com o `data` já
//!   carregando `recurrenceCount`/`lastProblemAt` novos. É atualização
//!   silenciosa de tela: **não** existe notificação de recaída;
//! - `alert:resolved` — fechamento definitivo do episódio.
//!
//! Publicação é best-effort em todos: um relay indisponível não pode desfazer
//! a escrita no banco que originou o evento.

use loco_rs::app::AppContext;
use serde_json::{json, Value};

use crate::{models::alert_events, services::events::EventBus};

/// O evento serializado completo, no formato que a Central de Alertas consome.
///
/// Título e nome da regra vêm do `data` — gravados no disparo, eles sobrevivem
/// à regra apagada, e é por isso que o payload não precisa reconsultar nada.
#[must_use]
pub fn event_payload(event: &alert_events::Model) -> Value {
    let data_string = |key: &str| -> Option<String> {
        event
            .data
            .as_ref()?
            .get(key)?
            .as_str()
            .map(ToString::to_string)
    };
    json!({
        "id": event.id,
        "alertEventId": event.id,
        "alertRuleId": event.alert_rule_id,
        "ruleName": data_string("ruleName"),
        "scopeKey": event.scope_key,
        "monitorId": event.monitor_id,
        "deviceId": event.device_id,
        "severity": event.severity,
        "status": event.status,
        "title": data_string("title"),
        "message": event.message,
        "data": event.data,
        "startedAt": event.started_at.to_rfc3339(),
        "resolvedAt": event.resolved_at.map(|value| value.to_rfc3339()),
        "createdAt": event.created_at.to_rfc3339(),
        "updatedAt": event.updated_at.to_rfc3339(),
    })
}

/// Publica e engole a falha com `warn!`: o CRUD já concluiu quando chegamos
/// aqui.
pub async fn publish(ctx: &AppContext, kind: &str, payload: Value) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus.publish(&ctx.db, kind, payload).await {
            tracing::warn!(%error, event = kind, "falha ao publicar evento de alerta");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn payload_carrega_o_episodio_no_data() {
        let now = Utc::now().into();
        let event = alert_events::Model {
            id: 7,
            alert_rule_id: Some(3),
            device_id: Some(2),
            monitor_id: Some(5),
            scope_key: Some("monitor:5".into()),
            status: "recovering".into(),
            severity: "critical".into(),
            started_at: now,
            resolved_at: None,
            message: Some("Host inacessível".into()),
            data: Some(json!({
                "title": "Dispositivo sem resposta — Roteador",
                "ruleName": "Dispositivo sem resposta",
                "recurrenceCount": 2,
                "lastProblemAt": "2026-08-15T12:00:00Z",
            })),
            created_at: now,
            updated_at: now,
        };
        let payload = event_payload(&event);
        assert_eq!(payload["id"], 7);
        assert_eq!(payload["status"], "recovering");
        assert_eq!(payload["title"], "Dispositivo sem resposta — Roteador");
        assert_eq!(payload["ruleName"], "Dispositivo sem resposta");
        assert_eq!(payload["data"]["recurrenceCount"], 2);
        assert!(payload["resolvedAt"].is_null());
    }
}
