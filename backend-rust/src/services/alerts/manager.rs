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
            contracts::{
                AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey, OPEN_STATUSES,
                STATUS_ACTIVE,
            },
            datasets::monitor_result,
            evaluator::{self, AlertRuleCondition},
            recovery, repository,
        },
        events::EventBus,
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
            trigger_alert(ctx, &rule, context).await?;
        }
    }

    if !has_triggered_rule && context.recovered {
        recovery::resolve_scope(ctx, &context.scope_key, recovery::DEFAULT_REASON).await?;
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
/// ocorrências não geram evento nem notificação repetida (matriz #25).
async fn trigger_alert(
    ctx: &AppContext,
    rule: &alert_rules::Model,
    context: &AlertEvaluationContext,
) -> AppResult<()> {
    let already_open = alert_events::Entity::find()
        .filter(alert_events_entity::Column::AlertRuleId.eq(rule.id))
        .filter(alert_events_entity::Column::ScopeKey.eq(context.scope_key.as_str()))
        .filter(alert_events_entity::Column::Status.is_in(OPEN_STATUSES))
        .one(&ctx.db)
        .await?;
    if already_open.is_some() {
        return Ok(());
    }

    let message = context
        .message
        .clone()
        .unwrap_or_else(|| format!("Alerta disparado pela regra: {}", rule.name));
    let title = format!("{} — {}", rule.name, context.target_label);

    let mut data = context.data.clone();
    data.insert("title".into(), json!(title));
    data.insert("ruleName".into(), json!(rule.name));

    let started_at = Utc::now();
    let event = alert_events::ActiveModel {
        alert_rule_id: Set(Some(rule.id)),
        device_id: Set(context.scope.device_id),
        monitor_id: Set(context.scope.monitor_id),
        scope_key: Set(Some(context.scope_key.clone())),
        status: Set(STATUS_ACTIVE.into()),
        severity: Set(rule.severity.clone()),
        started_at: Set(started_at.into()),
        message: Set(Some(message.clone())),
        data: Set(Some(Value::Object(data))),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    let severity = Severity::parse(&rule.severity);
    NotificationService::with_default_channels()
        .notify(
            ctx,
            &formatter::alert_triggered(
                &rule.name,
                &context.target_label,
                &message,
                severity,
                json!({
                    "alertEventId": event.id,
                    "monitorId": context.scope.monitor_id,
                    "deviceId": context.scope.device_id,
                    "scopeKey": context.scope_key,
                }),
            ),
        )
        .await;

    if let Ok(bus) = EventBus::from_context(ctx) {
        let payload = json!({
            "id": event.id,
            "alertEventId": event.id,
            "alertRuleId": rule.id,
            "ruleName": rule.name,
            "scopeKey": context.scope_key,
            "monitorId": context.scope.monitor_id,
            "deviceId": context.scope.device_id,
            "targetLabel": context.target_label,
            "severity": rule.severity,
            "status": event.status,
            "title": title,
            "message": message,
            "startedAt": started_at.to_rfc3339(),
            "createdAt": event.created_at.to_rfc3339(),
        });
        if let Err(error) = bus.publish(&ctx.db, "alert:triggered", payload).await {
            tracing::warn!(%error, alert_event_id = event.id, "falha ao publicar alert:triggered");
        }
    }

    Ok(())
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
