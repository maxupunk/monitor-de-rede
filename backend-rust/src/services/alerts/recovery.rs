//! Normalização automática de alertas (§8.7).
//!
//! Fecha os alertas abertos de um alvo quando ele volta ao normal. Trabalha por
//! `scope_key`, então serve tanto para monitores quanto para alvos sem monitor
//! (interfaces e túneis, por exemplo).

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};
use serde_json::json;

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events},
    services::{
        alerts::contracts::{AlertScopeKey, OPEN_STATUSES, STATUS_RESOLVED},
        events::EventBus,
        notifications::{formatter, NotificationService},
        shared::errors::AppResult,
    },
};

/// Texto padrão quando quem resolve não informa o motivo.
pub const DEFAULT_REASON: &str = "Monitoramento normalizado";

/// Fecha todos os alertas abertos do escopo e devolve quantos foram fechados.
///
/// A busca inclui `monitor_id` quando a chave é `monitor:<n>`: eventos gravados
/// antes de `scope_key` existir só têm a coluna preenchida, e fechá-los apenas
/// pela chave os deixaria abertos para sempre na Central de Alertas.
///
/// # Errors
///
/// Propaga erro do banco. Falha de notificação **não** é erro: ver §8.9.
pub async fn resolve_scope(ctx: &AppContext, scope_key: &str, reason: &str) -> AppResult<usize> {
    let mut target = Condition::any().add(alert_events_entity::Column::ScopeKey.eq(scope_key));
    if let Some(monitor_id) = AlertScopeKey::monitor_id_of(scope_key) {
        target = target.add(alert_events_entity::Column::MonitorId.eq(monitor_id));
    }

    let open = alert_events::Entity::find()
        .filter(target)
        .filter(alert_events_entity::Column::Status.is_in(OPEN_STATUSES))
        .all(&ctx.db)
        .await?;
    if open.is_empty() {
        return Ok(0);
    }

    let notifications = NotificationService::with_default_channels();
    let mut resolved = 0;

    for event in open {
        let resolved_at = Utc::now();
        let mut active: alert_events::ActiveModel = event.clone().into();
        active.status = Set(STATUS_RESOLVED.into());
        active.resolved_at = Set(Some(resolved_at.into()));
        let saved = active.update(&ctx.db).await?;
        resolved += 1;

        let title = saved
            .data
            .as_ref()
            .and_then(|data| data.get("title"))
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| format!("Alerta #{}", saved.id), ToString::to_string);

        notifications
            .notify(
                ctx,
                &formatter::alert_resolved(
                    saved.id,
                    saved.message.as_deref(),
                    reason,
                    json!({
                        "alertEventId": saved.id,
                        "scopeKey": scope_key,
                        "monitorId": saved.monitor_id,
                    }),
                ),
            )
            .await;

        publish(
            ctx,
            json!({
                "id": saved.id,
                "alertEventId": saved.id,
                "scopeKey": scope_key,
                "monitorId": saved.monitor_id,
                "deviceId": saved.device_id,
                "severity": saved.severity,
                "status": saved.status,
                "title": title,
                "message": saved.message,
                "resolvedAt": resolved_at.to_rfc3339(),
            }),
        )
        .await;
    }

    Ok(resolved)
}

/// Atalho para o alvo mais comum: os alertas abertos de um monitor.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn resolve_alerts_for_monitor(
    ctx: &AppContext,
    monitor_id: i64,
    reason: &str,
) -> AppResult<usize> {
    resolve_scope(ctx, &AlertScopeKey::monitor(monitor_id), reason).await
}

/// Publicação é best-effort: um relay indisponível não pode reabrir um alerta
/// que já foi resolvido no banco.
async fn publish(ctx: &AppContext, payload: serde_json::Value) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus.publish(&ctx.db, "alert:resolved", payload).await {
            tracing::warn!(%error, "falha ao publicar alert:resolved");
        }
    }
}
