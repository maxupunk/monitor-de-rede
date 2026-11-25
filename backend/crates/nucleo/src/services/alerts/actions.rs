//! Ações do operador sobre um alerta: reconhecer, silenciar e encerrar.
//!
//! Gravar o novo estado e avisar as telas pelo barramento são um passo só —
//! quem reconhece pela tela e quem reconhece pela IA passam pelo mesmo lugar.

use chrono::Utc;
use loco_rs::prelude::AppContext;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};

use super::{contracts::AlertStatus, feed, silence};
use crate::{
    models::alert_events,
    services::shared::{errors::AppResult, transaction::begin_write},
};

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
    let txn = begin_write(&ctx.db).await?;
    let current = alert_events::Entity::find_by_id(event.id)
        .one(&txn)
        .await?
        .unwrap_or(event);
    // Resolvido entre a leitura do chamador e esta transação: reconhecer não
    // reabre o episódio (e o índice de "um aberto por alvo" recusaria).
    if current.status == AlertStatus::Resolved.as_str() {
        txn.commit().await?;
        return Ok(current);
    }
    let saved = silence::acknowledge_alert(&txn, current).await?;
    txn.commit().await?;
    feed::publish(ctx, "alert:acknowledged", action_payload(&saved)).await;
    Ok(saved)
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
    let txn = begin_write(&ctx.db).await?;
    let current = alert_events::Entity::find_by_id(event.id)
        .one(&txn)
        .await?
        .unwrap_or(event);
    // Silenciar um alerta já resolvido não o reabre.
    if current.status == AlertStatus::Resolved.as_str() {
        txn.commit().await?;
        return Ok(current);
    }
    let saved = silence::silence_alert(&txn, current, minutes).await?;
    txn.commit().await?;
    let mut payload = action_payload(&saved);
    payload["silencedUntil"] = json!(silence::silenced_until(&saved).map(|at| at.to_rfc3339()));
    payload["durationMinutes"] = json!(minutes);
    feed::publish(ctx, "alert:silenced", payload).await;
    Ok(saved)
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
    let txn = begin_write(&ctx.db).await?;
    let current = alert_events::Entity::find_by_id(event.id)
        .one(&txn)
        .await?
        .unwrap_or(event);
    if current.status == AlertStatus::Resolved.as_str() {
        txn.commit().await?;
        return Ok(current);
    }
    let now = Utc::now();
    let data = silence::merge_data(
        current.data.clone(),
        "closedManuallyAt",
        json!(now.to_rfc3339()),
    );
    let mut active: alert_events::ActiveModel = current.into();
    active.status = Set(AlertStatus::Resolved.as_str().into());
    active.resolved_at = Set(Some(now.into()));
    active.data = Set(Some(data));
    let saved = active.update(&txn).await?;
    txn.commit().await?;
    feed::publish(ctx, "alert:resolved", feed::event_payload(&saved)).await;
    Ok(saved)
}
