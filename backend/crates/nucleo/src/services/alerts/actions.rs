//! Ações do operador sobre um alerta: reconhecer, silenciar e encerrar.
//!
//! Gravar o novo estado e avisar as telas pelo barramento são um passo só —
//! quem reconhece pela tela e quem reconhece pela IA passam pelo mesmo lugar.

use chrono::Utc;
use loco_rs::prelude::AppContext;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};

use super::{contracts::AlertStatus, feed, silence};
use crate::{models::alert_events, services::shared::errors::AppResult};

fn action_payload(event: &alert_events::Model) -> Value {
    json!({
        "id": event.id,
        "alertEventId": event.id,
        "monitorId": event.monitor_id,
        "deviceId": event.device_id,
        "status": event.status,
        "severity": event.severity,
        "message": event.message,
    })
}

/// Reconhece o alerta e publica `alert:acknowledged`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn acknowledge(
    ctx: &AppContext,
    event: alert_events::Model,
) -> AppResult<alert_events::Model> {
    let event = silence::acknowledge_alert(&ctx.db, event).await?;
    feed::publish(ctx, "alert:acknowledged", action_payload(&event)).await;
    Ok(event)
}

/// Silencia o alerta por `minutes` (padrão de 60 quando `<= 0`) e publica
/// `alert:silenced`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn silence(
    ctx: &AppContext,
    event: alert_events::Model,
    minutes: i64,
) -> AppResult<alert_events::Model> {
    let minutes = if minutes > 0 {
        minutes
    } else {
        silence::DEFAULT_SILENCE_MINUTES
    };
    let event = silence::silence_alert(&ctx.db, event, minutes).await?;
    let mut payload = action_payload(&event);
    payload["silencedUntil"] = json!(silence::silenced_until(&event).map(|at| at.to_rfc3339()));
    payload["durationMinutes"] = json!(minutes);
    feed::publish(ctx, "alert:silenced", payload).await;
    Ok(event)
}

/// Encerra o alerta à mão e publica `alert:resolved`.
///
/// É a saída para o alerta que a central não tem como reverificar — o de
/// evento (túnel VPN, status de interface) cuja volta nunca chegou — e para o
/// que o operador sabe que já passou. Não notifica: quem encerrou já sabe. Se
/// a condição ainda existir, a próxima checagem abre um alerta novo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn close(ctx: &AppContext, event: alert_events::Model) -> AppResult<alert_events::Model> {
    if event.status == AlertStatus::Resolved.as_str() {
        return Ok(event);
    }
    let now = Utc::now();
    let data = silence::merge_data(
        event.data.clone(),
        "closedManuallyAt",
        json!(now.to_rfc3339()),
    );
    let mut active: alert_events::ActiveModel = event.into();
    active.status = Set(AlertStatus::Resolved.as_str().into());
    active.resolved_at = Set(Some(now.into()));
    active.data = Set(Some(data));
    let saved = active.update(&ctx.db).await?;
    feed::publish(ctx, "alert:resolved", feed::event_payload(&saved)).await;
    Ok(saved)
}
