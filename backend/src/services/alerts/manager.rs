//! Motor de alertas (§8.7).
//!
//! Recebe *fatos* já traduzidos para o vocabulário das regras e decide o que
//! vira alerta. Toda política (o que é grave, quanto tolerar) mora nas regras
//! cadastradas — acrescentar um novo tipo de observação não exige tocar aqui,
//! basta publicar um dataset com os campos correspondentes.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Map, Value};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, alert_rules, monitors},
    services::{
        alerts::{
            baseline,
            contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey, AlertStatus},
            datasets::monitor_result,
            episode,
            evaluator::{self, AlertRuleCondition},
            feed, hysteresis, problem_kind, recovery, repository, silence,
            state_machine::{self, EpisodeInput, EpisodePolicy, Transition},
        },
        monitoring::contracts::{CheckResult, MonitorStatus},
        notifications::{
            formatter,
            outbox::{self, NotificationRequest},
            policy::{NotificationKind, NotificationPolicy},
            Severity,
        },
        shared::errors::AppResult,
    },
};

/// Avalia um conjunto de fatos contra as regras aplicáveis ao alvo.
///
/// # Errors
///
/// Propaga erro do banco. Falha de notificação nunca sobe (§8.9).
pub async fn evaluate(ctx: &AppContext, context: &AlertEvaluationContext) -> AppResult<()> {
    let rules = repository::find_enabled_for_scope(&ctx.db, context.scope).await?;
    let mut has_triggered_rule = false;

    for rule in rules {
        let Some(condition) = AlertRuleCondition::from_json(&rule.condition) else {
            tracing::warn!(
                rule_id = rule.id,
                "condição de regra ilegível; regra ignorada"
            );
            continue;
        };

        if !evaluator::evaluate(&condition, &context.dataset) {
            hysteresis::forget(rule.id, &context.scope_key);
            continue;
        }

        has_triggered_rule = true;
        if has_sustained_condition(ctx, &rule, &condition, context).await? {
            trigger_alert(ctx, &rule, &condition, context).await?;
        }
    }

    if !has_triggered_rule {
        if context.recovered {
            recovery::resolve_scope(ctx, &context.scope_key, recovery::DEFAULT_REASON).await?;
        } else if context.degraded {
            // `warning` não dispara regra (o evaluator não bateu), mas conta
            // como problema para a janela dos eventos abertos do escopo.
            let kind = problem_kind::classify(None, &context.dataset);
            recovery::note_degraded_scope(ctx, &context.scope_key, kind).await?;
        }
    }
    Ok(())
}

/// Adapta o resultado de um monitor ao contrato genérico de avaliação.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn evaluate_monitor_result(
    ctx: &AppContext,
    monitor: &monitors::Model,
    result: &CheckResult,
) -> AppResult<()> {
    // Monitores de checagem externa não têm dispositivo vinculado.
    let device = match monitor.device_id {
        Some(device_id) => {
            crate::models::devices::Entity::find_by_id(device_id)
                .one(&ctx.db)
                .await?
        }
        None => None,
    };

    let baseline = baseline::for_monitor(&ctx.db, monitor.id).await?;

    let mut data = Map::new();
    data.insert("resultData".into(), result.data.clone());
    data.insert("monitorType".into(), json!(monitor.r#type));

    evaluate(
        ctx,
        &AlertEvaluationContext {
            scope: AlertEvaluationScope {
                site_id: device.as_ref().and_then(|device| device.site_id),
                device_id: monitor.device_id,
                monitor_id: Some(monitor.id),
            },
            scope_key: AlertScopeKey::monitor(monitor.id),
            target_label: device
                .as_ref()
                .map_or_else(|| monitor.name.clone(), |device| device.name.clone()),
            dataset: monitor_result::build(&monitor.r#type, result, &baseline),
            message: result.message.clone().filter(|text| !text.is_empty()),
            data,
            recovered: result.status == MonitorStatus::Up,
            degraded: result.status == MonitorStatus::Warning,
        },
    )
    .await
}

/// Só libera o disparo quando a condição se mantém pelo tempo configurado em
/// `duration_seconds`, evitando alertas por oscilações momentâneas.
///
/// A contagem em si vive em [`hysteresis`], que desde a Fase 5 a reconstrói a
/// partir de `monitor_results` quando a memória do processo não a tem — um
/// restart do scheduler deixou de zerar a tolerância de todo mundo.
async fn has_sustained_condition(
    ctx: &AppContext,
    rule: &alert_rules::Model,
    condition: &AlertRuleCondition,
    context: &AlertEvaluationContext,
) -> AppResult<bool> {
    hysteresis::observe(
        &ctx.db,
        rule.id,
        i64::from(rule.duration_seconds),
        &context.scope_key,
        context.scope.monitor_id,
        condition,
        Utc::now(),
    )
    .await
}

/// Cria o evento, notifica os canais e publica no feed.
///
/// Um alerta aberto por (regra, `scope_key`): enquanto não for resolvido, novas
/// ocorrências não geram evento nem notificação repetida (matriz #25) — a
/// máquina de estados decide se é recaída ou continuação do problema.
async fn trigger_alert(
    ctx: &AppContext,
    rule: &alert_rules::Model,
    condition: &AlertRuleCondition,
    context: &AlertEvaluationContext,
) -> AppResult<()> {
    let already_open = alert_events::Entity::find()
        .filter(alert_events_entity::Column::AlertRuleId.eq(rule.id))
        .filter(alert_events_entity::Column::ScopeKey.eq(context.scope_key.as_str()))
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .one(&ctx.db)
        .await?;
    if let Some(open) = already_open {
        return update_open_event(ctx, &open, rule, condition, context).await;
    }

    let message = context
        .message
        .clone()
        .unwrap_or_else(|| format!("Alerta disparado pela regra: {}", rule.name));
    let title = format!("{} — {}", rule.name, context.target_label);

    let started_at = Utc::now();
    let mut data = context.data.clone();
    data.insert("title".into(), json!(title));
    data.insert("ruleName".into(), json!(rule.name));
    // O episódio nasce com o relógio da janela ancorado no disparo.
    data.insert(
        state_machine::LAST_PROBLEM_AT.into(),
        json!(started_at.to_rfc3339()),
    );
    data.insert(state_machine::RECURRENCE_COUNT.into(), json!(0));
    // Classificação do problema (Fase 2): o que o alerta está observando —
    // perda de pacotes, latência, DNS, interface, túnel — não só "caiu".
    let kind = problem_kind::classify(Some(&condition.field), &context.dataset);
    data.insert(problem_kind::PROBLEM_KIND.into(), json!(kind.as_str()));

    let event = alert_events::ActiveModel {
        alert_rule_id: Set(Some(rule.id)),
        device_id: Set(context.scope.device_id),
        monitor_id: Set(context.scope.monitor_id),
        scope_key: Set(Some(context.scope_key.clone())),
        status: Set(AlertStatus::Active.as_str().into()),
        severity: Set(rule.severity.clone()),
        started_at: Set(started_at.into()),
        message: Set(Some(message.clone())),
        data: Set(Some(Value::Object(data))),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    let severity = Severity::parse(&rule.severity);
    let detail = problem_kind::detail(kind, &context.dataset);
    // Desde a Fase 4 a notificação é **pedida**, não entregue: cooldown,
    // agrupamento e inibição decidem se ela chega ao canal (§2.3 da análise).
    // A falha aqui não desfaz o alerta já gravado — o `warn!` mantém a
    // disciplina "acessório nunca derruba essencial" (§8.9).
    let notification = NotificationRequest {
        alert_rule_id: Some(rule.id),
        scope_key: Some(context.scope_key.clone()),
        device_id: context.scope.device_id,
        kind: NotificationKind::Problem,
        policy: NotificationPolicy::from(rule),
        silenced: false,
        message: formatter::alert_triggered(
            &rule.name,
            &context.target_label,
            &message,
            severity,
            Some(&detail),
            json!({
                "alertEventId": event.id,
                "monitorId": context.scope.monitor_id,
                "deviceId": context.scope.device_id,
                "scopeKey": context.scope_key,
                "problemKind": kind.as_str(),
            }),
        ),
    };
    if let Err(error) = outbox::enqueue(ctx, notification).await {
        tracing::warn!(%error, alert_event_id = event.id, "falha ao enfileirar notificação");
    }

    let mut payload = feed::event_payload(&event);
    // O rótulo do alvo não vai ao `data` (só ao título), mas o feed em tempo
    // real o consome solto — por isso ele entra só aqui, no disparo.
    payload["targetLabel"] = json!(context.target_label);
    feed::publish(ctx, "alert:triggered", payload).await;

    Ok(())
}

/// Atualiza o evento já aberto de (regra, alvo): recaída dentro da janela ou
/// simples continuação do problema.
///
/// Em ambos os casos `lastProblemAt` avança para `now` — a janela de
/// resolução conta do **último** problema. Nunca há notificação aqui: recaída
/// é atualização silenciosa de tela (`alert:updated`), não alerta novo. A única
/// exceção é a **declaração de flapping** (Fase 3), que avisa uma vez que o
/// alvo é cronicamente instável e a partir daí volta ao silêncio. O
/// `problemKind` é reavaliado a cada passagem: a recaída pode ter causa
/// diferente da queda original.
async fn update_open_event(
    ctx: &AppContext,
    open: &alert_events::Model,
    rule: &alert_rules::Model,
    condition: &AlertRuleCondition,
    context: &AlertEvaluationContext,
) -> AppResult<()> {
    let now = Utc::now();
    let policy = EpisodePolicy::from(rule);
    let status = open.status.parse().unwrap_or(AlertStatus::Active);
    let transition = state_machine::decide(&EpisodeInput {
        status,
        data: open.data.as_ref().and_then(Value::as_object),
        started_at: open.started_at.with_timezone(&Utc),
        policy,
        condition_matched: true,
        degraded: false,
        recovered: false,
        // Um silêncio pedido pelo operador sobrevive à entrada em recovering:
        // a recaída devolve o alerta a silenced enquanto o prazo vigorar.
        silenced_now: silence::silenced_until(open).is_some_and(|until| until > now),
        now,
    });

    let mut data = open
        .data
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let kind = problem_kind::classify(Some(&condition.field), &context.dataset);
    data.insert(problem_kind::PROBLEM_KIND.into(), json!(kind.as_str()));
    match transition {
        Transition::Relapse { recurrence, status } => {
            episode::mark_relapse(&mut data, recurrence, now, policy);
            let saved = episode::persist(ctx, open, status, data).await?;
            feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
        }
        Transition::StartFlapping {
            recurrence,
            transitions,
        } => {
            episode::mark_relapse(&mut data, recurrence, now, policy);
            data.insert(
                state_machine::FLAPPING_SINCE.into(),
                json!(now.to_rfc3339()),
            );
            let saved = episode::persist(ctx, open, AlertStatus::Flapping, data).await?;
            feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
            notify_flapping(ctx, &saved, transitions, rule, policy, context).await;
        }
        Transition::ProblemOngoing => {
            data.insert(
                state_machine::LAST_PROBLEM_AT.into(),
                json!(now.to_rfc3339()),
            );
            let mut active: alert_events::ActiveModel = open.clone().into();
            active.data = Set(Some(Value::Object(data)));
            active.update(&ctx.db).await?;
        }
        // Com a condição batendo a máquina só devolve os três ramos acima.
        _ => {}
    }
    Ok(())
}

/// O aviso único de "alvo oscilando".
async fn notify_flapping(
    ctx: &AppContext,
    event: &alert_events::Model,
    transitions: u32,
    rule: &alert_rules::Model,
    policy: EpisodePolicy,
    context: &AlertEvaluationContext,
) {
    let request = NotificationRequest {
        alert_rule_id: event.alert_rule_id,
        scope_key: event.scope_key.clone(),
        device_id: event.device_id,
        kind: NotificationKind::Flapping,
        policy: NotificationPolicy::from(rule),
        silenced: false,
        message: formatter::alert_flapping(
            &episode::title_of(event, &context.target_label),
            transitions,
            policy.flap_window_seconds,
            Severity::parse(&event.severity),
            json!({
                "alertEventId": event.id,
                "monitorId": event.monitor_id,
                "deviceId": event.device_id,
                "scopeKey": event.scope_key,
                "transitions": transitions,
            }),
        ),
    };
    if let Err(error) = outbox::enqueue(ctx, request).await {
        tracing::warn!(%error, alert_event_id = event.id, "falha ao enfileirar aviso de oscilação");
    }
}
