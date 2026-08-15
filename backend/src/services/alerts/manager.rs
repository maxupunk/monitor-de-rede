//! Motor de alertas (§8.7).
//!
//! Recebe *fatos* já traduzidos para o vocabulário das regras e decide o que
//! vira alerta. Toda política (o que é grave, quanto tolerar) mora nas regras
//! cadastradas — acrescentar um novo tipo de observação não exige tocar aqui,
//! basta publicar um dataset com os campos correspondentes.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Map, Value};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, alert_rules, monitors},
    services::{
        alerts::{
            contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey, AlertStatus},
            datasets::monitor_result,
            episode,
            evaluator::{self, AlertRuleCondition},
            feed, problem_kind, recovery, repository, silence,
            state_machine::{self, EpisodeInput, EpisodePolicy, Transition},
        },
        monitoring::contracts::{CheckResult, MonitorStatus},
        notifications::{formatter, NotificationService, Severity},
        shared::errors::AppResult,
    },
};

/// Momento em que cada regra passou a bater continuamente, por alvo.
///
/// Mora em memória de propósito: `duration_seconds` mede *continuidade
/// observada*, e o que interessa é o relógio monotônico do processo que está
/// observando. Persistir isso faria uma reinicialização do scheduler herdar uma
/// tolerância que ninguém acompanhou.
fn pending_since() -> &'static Mutex<HashMap<(i64, String), Instant>> {
    static PENDING: OnceLock<Mutex<HashMap<(i64, String), Instant>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

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
            forget_pending(rule.id, &context.scope_key);
            continue;
        }

        has_triggered_rule = true;
        if has_sustained_condition(&rule, &context.scope_key) {
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
            dataset: monitor_result::build(&monitor.r#type, result),
            message: result.message.clone().filter(|text| !text.is_empty()),
            data,
            recovered: result.status == MonitorStatus::Up,
            degraded: result.status == MonitorStatus::Warning,
        },
    )
    .await
}

fn forget_pending(rule_id: i64, scope_key: &str) {
    if let Ok(mut pending) = pending_since().lock() {
        pending.remove(&(rule_id, scope_key.to_string()));
    }
}

/// Só libera o disparo quando a condição se mantém pelo tempo configurado em
/// `duration_seconds`, evitando alertas por oscilações momentâneas.
fn has_sustained_condition(rule: &alert_rules::Model, scope_key: &str) -> bool {
    let tolerance = i64::from(rule.duration_seconds);
    if tolerance <= 0 {
        return true;
    }
    let Ok(mut pending) = pending_since().lock() else {
        // Um mutex envenenado não pode calar o alerta: na dúvida, dispara.
        return true;
    };
    let key = (rule.id, scope_key.to_string());
    match pending.get(&key) {
        // Primeira ocorrência: começa a contar e ainda não dispara.
        None => {
            pending.insert(key, Instant::now());
            false
        }
        #[allow(clippy::cast_sign_loss)]
        Some(first_seen) => first_seen.elapsed().as_secs() >= tolerance as u64,
    }
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
    NotificationService::with_default_channels()
        .notify(
            ctx,
            &formatter::alert_triggered(
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
        )
        .await;

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
            notify_flapping(ctx, &saved, transitions, policy, context).await;
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
    policy: EpisodePolicy,
    context: &AlertEvaluationContext,
) {
    NotificationService::with_default_channels()
        .notify(
            ctx,
            &formatter::alert_flapping(
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
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regra(id: i64, duration_seconds: i32) -> alert_rules::Model {
        let now = Utc::now().into();
        alert_rules::Model {
            id,
            site_id: None,
            device_id: None,
            monitor_id: None,
            name: "Regra".into(),
            r#type: "custom".into(),
            template_key: None,
            condition: json!({ "field": "status", "operator": "eq", "value": "down" }),
            severity: "warning".into(),
            duration_seconds,
            recovery_window_seconds: 0,
            flap_threshold: 0,
            flap_window_seconds: 900,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn sem_tolerancia_dispara_na_primeira_ocorrencia() {
        assert!(has_sustained_condition(&regra(9_001, 0), "monitor:1"));
    }

    #[test]
    fn com_tolerancia_a_primeira_ocorrencia_apenas_inicia_a_contagem() {
        // Matriz de paridade #24: `durationSeconds` só dispara após a condição
        // se sustentar — a primeira passagem nunca alerta.
        let rule = regra(9_002, 300);
        assert!(!has_sustained_condition(&rule, "monitor:2"));
        assert!(!has_sustained_condition(&rule, "monitor:2"));
        forget_pending(rule.id, "monitor:2");
    }

    #[test]
    fn condicao_que_deixa_de_bater_reinicia_a_contagem() {
        let rule = regra(9_003, 300);
        assert!(!has_sustained_condition(&rule, "monitor:3"));
        forget_pending(rule.id, "monitor:3");
        // Recomeçou do zero: continua sem disparar.
        assert!(!has_sustained_condition(&rule, "monitor:3"));
        forget_pending(rule.id, "monitor:3");
    }

    #[test]
    fn a_contagem_e_por_regra_e_por_alvo() {
        let rule = regra(9_004, 300);
        assert!(!has_sustained_condition(&rule, "monitor:10"));
        // Alvo diferente tem contagem própria, também começando agora.
        assert!(!has_sustained_condition(&rule, "monitor:11"));
        forget_pending(rule.id, "monitor:10");
        forget_pending(rule.id, "monitor:11");
    }
}
