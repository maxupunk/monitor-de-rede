//! Normalização automática de alertas (§8.7).
//!
//! Fecha os alertas abertos de um alvo quando ele volta ao normal — mas não na
//! primeira checagem ok: desde a Fase 1 do roadmap de monitoramento
//! inteligente, cada evento segue a `recovery_window_seconds` da **sua** regra
//! e só fecha depois de uma janela de estabilidade sem recaída. A decisão é da
//! máquina de estados pura em [`state_machine`]; aqui ficam a carga das
//! janelas, a persistência e os efeitos (notificação e SSE).
//!
//! Trabalha por `scope_key`, então serve tanto para monitores quanto para
//! alvos sem monitor (interfaces e túneis, por exemplo).

use std::collections::HashMap;

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, alert_rules},
    services::{
        alerts::{
            contracts::{AlertScopeKey, AlertStatus},
            feed, silence,
            state_machine::{self, EpisodeInput, Transition},
        },
        notifications::{formatter, NotificationService},
        shared::errors::AppResult,
    },
};

/// Texto padrão quando quem resolve não informa o motivo.
pub const DEFAULT_REASON: &str = "Monitoramento normalizado";

/// Recuperação automática: o alvo voltou ao normal e cada evento aberto segue
/// a janela da sua regra. Devolve quantos eventos **fecharam** (eventos que
/// entraram em `recovering` continuam abertos e não contam).
///
/// A busca inclui `monitor_id` quando a chave é `monitor:<n>`: eventos gravados
/// antes de `scope_key` existir só têm a coluna preenchida, e tratá-los apenas
/// pela chave os deixaria abertos para sempre na Central de Alertas.
///
/// # Errors
///
/// Propaga erro do banco. Falha de notificação **não** é erro: ver §8.9.
pub async fn resolve_scope(ctx: &AppContext, scope_key: &str, reason: &str) -> AppResult<usize> {
    run(ctx, scope_key, reason, true).await
}

/// Fechamento administrativo (monitor desativado ou removido): ignora a janela
/// e resolve na hora.
///
/// Sem mais checagens do alvo, um evento mandado para `recovering` ficaria
/// preso no estado para sempre — desativar não é "voltou ao normal", é "não
/// observar mais", e a única saída honesta é fechar.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn close_scope(ctx: &AppContext, scope_key: &str, reason: &str) -> AppResult<usize> {
    run(ctx, scope_key, reason, false).await
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
    close_scope(ctx, &AlertScopeKey::monitor(monitor_id), reason).await
}

async fn run(
    ctx: &AppContext,
    scope_key: &str,
    reason: &str,
    respect_windows: bool,
) -> AppResult<usize> {
    let mut target = Condition::any().add(alert_events_entity::Column::ScopeKey.eq(scope_key));
    if let Some(monitor_id) = AlertScopeKey::monitor_id_of(scope_key) {
        target = target.add(alert_events_entity::Column::MonitorId.eq(monitor_id));
    }

    let open = alert_events::Entity::find()
        .filter(target)
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .all(&ctx.db)
        .await?;
    if open.is_empty() {
        return Ok(0);
    }

    // A janela de cada evento vem da sua regra, carregada de uma vez. Regra
    // apagada (ou fechamento administrativo) = janela 0: resolve na hora.
    let windows: HashMap<i64, i32> = if respect_windows {
        let rule_ids: Vec<i64> = open
            .iter()
            .filter_map(|event| event.alert_rule_id)
            .collect();
        alert_rules::Entity::find()
            .filter(alert_rules::Column::Id.is_in(rule_ids))
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|rule| (rule.id, rule.recovery_window_seconds))
            .collect()
    } else {
        HashMap::new()
    };

    let notifications = NotificationService::with_default_channels();
    let mut resolved = 0;

    for event in open {
        let now = Utc::now();
        let window = event
            .alert_rule_id
            .and_then(|rule_id| windows.get(&rule_id))
            .copied()
            .unwrap_or(0);
        // Status fora do vocabulário (linha antiga) é tratado como `active`:
        // o pior caso é entrar em observação quando poderia fechar.
        let status = event.status.parse().unwrap_or(AlertStatus::Active);
        let transition = state_machine::decide(&EpisodeInput {
            status,
            data: event.data.as_ref().and_then(Value::as_object),
            started_at: event.started_at.with_timezone(&Utc),
            recovery_window_seconds: i64::from(window),
            condition_matched: false,
            recovered: true,
            silenced_now: silence::silenced_until(&event).is_some_and(|until| until > now),
            now,
        });

        match transition {
            Transition::Resolve { summary } => {
                resolved += 1;
                let saved = persist_resolve(ctx, event, now).await?;
                notifications
                    .notify(
                        ctx,
                        &formatter::alert_resolved(
                            saved.id,
                            saved.message.as_deref(),
                            reason,
                            Some(summary),
                            json!({
                                "alertEventId": saved.id,
                                "scopeKey": scope_key,
                                "monitorId": saved.monitor_id,
                            }),
                        ),
                    )
                    .await;
                feed::publish(ctx, "alert:resolved", feed::event_payload(&saved)).await;
            }
            Transition::EnterRecovering => {
                // Entrada em recovering não notifica: a notícia boa de verdade
                // é a resolução ao fim da janela.
                let saved = persist_enter_recovering(ctx, event).await?;
                feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
            }
            // `None` não aparece neste caminho: `condition_matched` é sempre
            // falso aqui, então recaída e "problema em curso" são impossíveis.
            _ => {}
        }
    }

    Ok(resolved)
}

/// Persiste o fechamento do episódio.
async fn persist_resolve(
    ctx: &AppContext,
    event: alert_events::Model,
    resolved_at: chrono::DateTime<Utc>,
) -> AppResult<alert_events::Model> {
    let mut active: alert_events::ActiveModel = event.into();
    active.status = Set(AlertStatus::Resolved.as_str().into());
    active.resolved_at = Set(Some(resolved_at.into()));
    Ok(active.update(&ctx.db).await?)
}

/// Persiste a entrada em observação de estabilidade.
///
/// O `lastProblemAt` registrado é mantido — é ele que ancora a janela no
/// último problema. Na falta (evento aberto antes da Fase 1), ancora na
/// abertura do evento, e o contador de recaídas nasce em zero.
async fn persist_enter_recovering(
    ctx: &AppContext,
    event: alert_events::Model,
) -> AppResult<alert_events::Model> {
    let mut data = event
        .data
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    data.entry(state_machine::LAST_PROBLEM_AT)
        .or_insert_with(|| json!(event.started_at.to_rfc3339()));
    data.entry(state_machine::RECURRENCE_COUNT)
        .or_insert(json!(0));

    let mut active: alert_events::ActiveModel = event.into();
    active.status = Set(AlertStatus::Recovering.as_str().into());
    active.data = Set(Some(Value::Object(data)));
    Ok(active.update(&ctx.db).await?)
}
