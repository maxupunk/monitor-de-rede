//! Resumo automático de incidente: quando um alerta abre, a IA investiga com
//! as ferramentas de leitura e grava no próprio alerta a causa provável e a
//! primeira ação. A tela recebe o resumo pelo barramento (`alert:ai_summary`).

use std::{
    collections::HashSet,
    sync::{Arc, Mutex as StdMutex, PoisonError},
};

use chrono::Utc;
use loco_rs::prelude::AppContext;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{broadcast::error::RecvError, Mutex, Semaphore};
use ts_rs::TS;

use super::{
    runner::{ask, DriverFactory},
    schedule::HourlyLimiter,
    triage,
};
use crate::{harness::prompt::ChatContext, settings};
use netmonitor_core::{
    alerts::{contracts::AlertStatus, feed},
    events::EventBus,
    models::alert_events,
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

/// Máximo de resumos/triagens simultâneos disparados por alertas em tempestade.
const MAX_CONCURRENT_SUMMARIES: usize = 2;

/// Garante que o mesmo `alert_id` não rode em paralelo e libera na saída.
struct InFlightGuard {
    set: Arc<StdMutex<HashSet<i64>>>,
    alert_id: i64,
}

impl InFlightGuard {
    fn acquire(set: &Arc<StdMutex<HashSet<i64>>>, alert_id: i64) -> Option<Self> {
        let inserted = set
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(alert_id);
        inserted.then(|| Self {
            set: Arc::clone(set),
            alert_id,
        })
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.set
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.alert_id);
    }
}

/// O que a tela mostra quando pedem um resumo com a IA desligada.
pub const DISABLED_MESSAGE: &str =
    "A IA está desligada. Ligue em Configurações › Inteligência artificial para gerar o resumo.";

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
/// IA desligada (`BusinessRule`), provedor indisponível ou erro do banco.
pub async fn summarize_alert(
    ctx: &AppContext,
    drivers: &dyn DriverFactory,
    alert_id: i64,
) -> AppResult<Option<AiIncidentSummary>> {
    let settings = settings::load(&ctx.db).await?;
    if !settings.enabled {
        return Err(AppError::BusinessRule(DISABLED_MESSAGE.into()));
    }
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

    // Relê e grava numa transação de escrita: o motor de alertas pode ter
    // mexido no `data` (silêncio, reconhecimento) enquanto a IA pensava.
    let tx = begin_write(&ctx.db).await?;
    let Some(current) = alert_events::Entity::find_by_id(alert_id).one(&tx).await? else {
        return Ok(None);
    };
    if has_summary(&current) {
        return Ok(None);
    }
    let data = with_summary(current.data.clone(), &summary);
    let mut active: alert_events::ActiveModel = current.into();
    active.data = Set(Some(data));
    let updated = active.update(&tx).await?;
    tx.commit().await?;

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
/// a cada alerta, então ligar ou desligar vale sem reiniciar. Termina se o
/// barramento fechar.
pub async fn listen(ctx: AppContext, drivers: Arc<dyn DriverFactory>) {
    let Ok(bus) = EventBus::from_context(&ctx) else {
        tracing::warn!("barramento de eventos indisponível; resumos de incidente desligados");
        return;
    };
    let mut receiver = bus.subscribe();
    let limiter = Arc::new(Mutex::new(HourlyLimiter::default()));
    let concurrency = Arc::new(Semaphore::new(MAX_CONCURRENT_SUMMARIES));
    let in_flight = Arc::new(StdMutex::new(HashSet::<i64>::new()));

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
            .unwrap_or_default()
            .to_string();

        // Consulta de config, triagem e teto ficam na task para o laço do
        // barramento nunca bloquear e não perder eventos (`Lagged`).
        let ctx = ctx.clone();
        let drivers = Arc::clone(&drivers);
        let limiter = Arc::clone(&limiter);
        let concurrency = Arc::clone(&concurrency);
        let in_flight = Arc::clone(&in_flight);
        tokio::spawn(async move {
            let Some(_in_flight) = InFlightGuard::acquire(&in_flight, alert_id) else {
                return;
            };
            let Some(max_per_hour) = wants_summary(&ctx, &severity).await else {
                return;
            };
            let Ok(_permit) = concurrency.acquire_owned().await else {
                return;
            };
            match triage::assess(&ctx, alert_id).await {
                Ok(Some((opinion, _))) if opinion.suppressed => {
                    tracing::info!(
                        alert_id,
                        actionable = opinion.actionable,
                        "resumo de incidente pulado pela triagem do Decisor"
                    );
                    return;
                }
                Ok(_) => {}
                Err(error) => tracing::debug!(%error, alert_id, "triagem do Decisor falhou"),
            }
            if !limiter.lock().await.try_acquire(Utc::now(), max_per_hour) {
                tracing::info!(
                    alert_id,
                    "resumo de incidente adiado: teto por hora atingido"
                );
                return;
            }
            if let Err(error) = summarize_alert(&ctx, drivers.as_ref(), alert_id).await {
                tracing::warn!(%error, alert_id, "falha ao gerar resumo de incidente");
            }
        });
    }
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
