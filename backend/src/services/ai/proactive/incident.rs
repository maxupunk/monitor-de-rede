//! Resumo automático de incidente: quando um alerta abre, a IA investiga com
//! as ferramentas de leitura e grava no próprio alerta a causa provável e a
//! primeira ação. A tela recebe o resumo pelo barramento (`alert:ai_summary`).

use std::sync::Arc;

use chrono::Utc;
use loco_rs::prelude::AppContext;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{broadcast::error::RecvError, Mutex};
use ts_rs::TS;

use super::{
    runner::{ask, DriverFactory},
    schedule::HourlyLimiter,
};
use crate::{
    models::alert_events,
    services::{
        ai::{harness::prompt::ChatContext, settings},
        alerts::{contracts::AlertStatus, feed},
        events::EventBus,
        shared::errors::AppResult,
    },
};

/// Campo de `alert_events.data` onde o resumo fica.
pub const SUMMARY_FIELD: &str = "aiSummary";

/// Evento publicado quando o resumo fica pronto.
pub const SUMMARY_EVENT: &str = "alert:ai_summary";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiIncidentSummary {
    pub text: String,
    pub generated_at: String,
    #[ts(type = "number")]
    pub prompt_tokens: u64,
    #[ts(type = "number")]
    pub completion_tokens: u64,
}

fn prompt(alert_id: i64) -> String {
    format!(
        "O alerta #{alert_id} acabou de abrir. Investigue com as ferramentas (detalhes do alerta e do dispositivo, \
analyze_root_cause, compare_with_baseline, get_logs_overview do dispositivo na última hora) e responda em até 4 linhas: \
causa provável, a evidência que a sustenta e a primeira ação recomendada. Sem introdução."
    )
}

/// Já tem resumo? Evita pagar duas vezes pelo mesmo alerta.
fn has_summary(event: &alert_events::Model) -> bool {
    event
        .data
        .as_ref()
        .and_then(|data| data.get(SUMMARY_FIELD))
        .is_some()
}

/// `data` com o resumo incluído, preservando o que já estava lá.
fn with_summary(data: Option<Value>, summary: &AiIncidentSummary) -> Value {
    let mut data = match data {
        Some(Value::Object(map)) => Value::Object(map),
        _ => json!({}),
    };
    data[SUMMARY_FIELD] = json!(summary);
    data
}

/// Gera e grava o resumo do alerta. `None` quando não há o que resumir
/// (alerta sumiu, já resolveu ou já tem resumo).
///
/// # Errors
///
/// Provedor indisponível ou erro do banco.
pub async fn summarize_alert(
    ctx: &AppContext,
    drivers: &dyn DriverFactory,
    alert_id: i64,
) -> AppResult<Option<AiIncidentSummary>> {
    let settings = settings::load(&ctx.db).await?;
    let Some(event) = alert_events::Entity::find_by_id(alert_id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(None);
    };
    if event.status == AlertStatus::Resolved.as_str() || has_summary(&event) {
        return Ok(None);
    }

    let answer = ask(
        ctx,
        &settings,
        drivers,
        prompt(alert_id),
        ChatContext {
            device_id: event.device_id,
            monitor_id: event.monitor_id,
            alert_id: Some(alert_id),
            ..ChatContext::default()
        },
    )
    .await?;
    let summary = AiIncidentSummary {
        text: answer.text,
        generated_at: Utc::now().to_rfc3339(),
        prompt_tokens: answer.usage.prompt_tokens,
        completion_tokens: answer.usage.completion_tokens,
    };

    // Relê antes de gravar: o motor de alertas pode ter mexido no `data`
    // (silêncio, reconhecimento) enquanto a IA pensava.
    let Some(current) = alert_events::Entity::find_by_id(alert_id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(None);
    };
    let data = with_summary(current.data.clone(), &summary);
    let mut active: alert_events::ActiveModel = current.into();
    active.data = Set(Some(data));
    let updated = active.update(&ctx.db).await?;

    feed::publish(
        ctx,
        SUMMARY_EVENT,
        json!({
            "id": updated.id,
            "alertEventId": updated.id,
            "deviceId": updated.device_id,
            "aiSummary": summary,
        }),
    )
    .await;
    Ok(Some(summary))
}

/// O alerta recém-aberto merece resumo, segundo as configurações atuais?
async fn wants_summary(ctx: &AppContext, severity: &str) -> Option<u32> {
    let settings = settings::load(&ctx.db).await.ok()?;
    let proactive = &settings.proactive;
    (settings.enabled
        && proactive.incident_summaries
        && proactive.incident_min_severity.accepts(severity))
    .then_some(proactive.max_summaries_per_hour)
}

/// Escuta `alert:triggered` e dispara os resumos. As configurações são lidas
/// a cada alerta, então ligar ou desligar vale sem reiniciar.
pub fn spawn_listener(ctx: AppContext, drivers: Arc<dyn DriverFactory>) {
    let Ok(bus) = EventBus::from_context(&ctx) else {
        tracing::warn!("barramento de eventos indisponível; resumos de incidente desligados");
        return;
    };
    let mut receiver = bus.subscribe();
    let limiter = Arc::new(Mutex::new(HourlyLimiter::default()));

    tokio::spawn(async move {
        loop {
            let event = match receiver.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "resumos de incidente: eventos perdidos");
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            if event.event_type != "alert:triggered" {
                continue;
            }
            let Some(alert_id) = event.payload.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let severity = event
                .payload
                .get("severity")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(max_per_hour) = wants_summary(&ctx, severity).await else {
                continue;
            };
            if !limiter.lock().await.try_acquire(Utc::now(), max_per_hour) {
                tracing::info!(
                    alert_id,
                    "resumo de incidente adiado: teto por hora atingido"
                );
                continue;
            }

            let ctx = ctx.clone();
            let drivers = Arc::clone(&drivers);
            tokio::spawn(async move {
                if let Err(error) = summarize_alert(&ctx, drivers.as_ref(), alert_id).await {
                    tracing::warn!(%error, alert_id, "falha ao gerar resumo de incidente");
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resumo() -> AiIncidentSummary {
        AiIncidentSummary {
            text: "Uplink caiu".into(),
            generated_at: "2026-09-21T10:00:00Z".into(),
            prompt_tokens: 10,
            completion_tokens: 5,
        }
    }

    #[test]
    fn resumo_entra_no_data_sem_apagar_o_resto() {
        let data = with_summary(Some(json!({ "silencedUntil": "x" })), &resumo());
        assert_eq!(data["silencedUntil"], "x");
        assert_eq!(data[SUMMARY_FIELD]["text"], "Uplink caiu");
        assert_eq!(data[SUMMARY_FIELD]["promptTokens"], 10);

        let vazio = with_summary(None, &resumo());
        assert_eq!(vazio[SUMMARY_FIELD]["completionTokens"], 5);
    }
}
