//! Motor de alertas (§8.7).
//!
//! Recebe *fatos* já traduzidos para o vocabulário das regras e decide o que
//! vira alerta. Toda política (o que é grave, quanto tolerar) mora nas regras
//! cadastradas — acrescentar um novo tipo de observação não exige tocar aqui,
//! basta publicar um dataset com os campos correspondentes.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    Set,
};
use serde_json::{json, Map, Value};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, alert_rules, monitors},
    services::{
        alerts::{
            adaptive_latency, baseline,
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
        shared::{errors::AppResult, transaction::begin_write},
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
) -> AppResult<adaptive_latency::Assessment> {
    // Monitores de checagem externa não têm dispositivo vinculado.
    let device = match monitor.device_id {
        Some(device_id) => {
            crate::models::devices::Entity::find_by_id(device_id)
                .one(&ctx.db)
                .await?
        }
        None => None,
    };
    evaluate_monitor_result_with_device(ctx, monitor, device.as_ref(), result).await
}

/// Variante de [`evaluate_monitor_result`] que reutiliza o dispositivo já carregado pelo chamador.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn evaluate_monitor_result_with_device(
    ctx: &AppContext,
    monitor: &monitors::Model,
    device: Option<&crate::models::devices::Model>,
    result: &CheckResult,
) -> AppResult<adaptive_latency::Assessment> {
    let baseline = baseline::for_monitor(&ctx.db, monitor.id).await?;
    let mut dataset = monitor_result::build(&monitor.r#type, result, &baseline);
    let current_latency_ms = dataset
        .get(crate::services::alerts::fields::LATENCY_MS)
        .and_then(Value::as_f64);
    let latency_assessment = match adaptive_latency::assess(
        &ctx.db,
        monitor,
        &baseline,
        current_latency_ms,
        result.finished_at,
    )
    .await
    {
        Ok(assessment) => assessment,
        Err(error) => {
            // Falha de contexto nunca pode silenciar uma regra existente. O
            // caminho seguro é liberar a avaliação normal e registrar o erro.
            tracing::warn!(
                %error,
                monitor_id = monitor.id,
                "falha ao avaliar a guarda adaptativa de latência"
            );
            adaptive_latency::Assessment::fail_open(current_latency_ms)
        }
    };
    if latency_assessment.applies && !latency_assessment.alert_eligible {
        adaptive_latency::suppress_latency_facts(&mut dataset);
    }

    let mut data = Map::new();
    data.insert("resultData".into(), result.data.clone());
    data.insert("monitorType".into(), json!(monitor.r#type));
    data.insert("adaptiveLatency".into(), json!(latency_assessment));

    evaluate(
        ctx,
        &AlertEvaluationContext {
            scope: AlertEvaluationScope {
                site_id: device.and_then(|device| device.site_id),
                device_id: monitor.device_id,
                monitor_id: Some(monitor.id),
            },
            scope_key: AlertScopeKey::monitor(monitor.id),
            target_label: device.map_or_else(|| monitor.name.clone(), |device| device.name.clone()),
            dataset,
            message: result.message.clone().filter(|text| !text.is_empty()),
            data,
            recovered: result.status == MonitorStatus::Up,
            degraded: result.status == MonitorStatus::Warning,
            refresh_open: false,
        },
    )
    .await?;
    Ok(latency_assessment)
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
    // A guarda adaptativa já reconstrói de forma persistente as X leituras
    // consecutivas. Aplicar também `duration_seconds` aqui criaria uma segunda
    // espera, fazendo "3 leituras" significar 3 leituras + 5 minutos.
    let adaptive_confirmed = adaptive_latency::is_latency_field(&condition.field)
        && context
            .data
            .get("adaptiveLatency")
            .and_then(Value::as_object)
            .is_some_and(|assessment| {
                assessment.get("applies").and_then(Value::as_bool) == Some(true)
                    && assessment.get("alertEligible").and_then(Value::as_bool) == Some(true)
            });
    if adaptive_confirmed {
        return Ok(true);
    }
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
    let txn = begin_write(&ctx.db).await?;
    if let Some(open) = find_open_event(&txn, rule.id, &context.scope_key).await? {
        return continue_open_event(ctx, txn, &open, rule, condition, context).await;
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
    .insert(&txn)
    .await;
    let event = match event {
        Ok(event) => event,
        // O índice único parcial `alert_events_open_rule_scope_unique` barrou
        // um segundo aberto: outra transação abriu o mesmo alerta entre a
        // leitura e o insert (PostgreSQL). É continuação, não alerta novo.
        Err(error) if netmonitor_core::shared::db_errors::is_unique_violation(&error) => {
            txn.rollback().await?;
            let txn = begin_write(&ctx.db).await?;
            return match find_open_event(&txn, rule.id, &context.scope_key).await? {
                Some(open) => continue_open_event(ctx, txn, &open, rule, condition, context).await,
                None => {
                    // Já foi resolvido de novo nesse meio-tempo: nada a fazer
                    // agora; o próximo resultado reavalia.
                    txn.commit().await?;
                    Ok(())
                }
            };
        }
        Err(error) => return Err(error.into()),
    };
    txn.commit().await?;

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

/// O alerta aberto da regra para o alvo, lido dentro da transação.
async fn find_open_event<C: ConnectionTrait>(
    db: &C,
    rule_id: i64,
    scope_key: &str,
) -> AppResult<Option<alert_events::Model>> {
    Ok(alert_events::Entity::find()
        .filter(alert_events_entity::Column::AlertRuleId.eq(rule_id))
        .filter(alert_events_entity::Column::ScopeKey.eq(scope_key))
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .one(db)
        .await?)
}

/// Nova ocorrência de um alerta já aberto: a máquina de estados decide, a
/// transação fecha e só então o feed e a notificação de flapping saem.
async fn continue_open_event(
    ctx: &AppContext,
    txn: DatabaseTransaction,
    open: &alert_events::Model,
    rule: &alert_rules::Model,
    condition: &AlertRuleCondition,
    context: &AlertEvaluationContext,
) -> AppResult<()> {
    let side_effect = update_open_event_in_txn(&txn, open, rule, condition, context).await?;
    txn.commit().await?;
    if let Some(effect) = side_effect {
        match effect {
            OpenEventEffect::Updated(saved) => {
                feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
            }
            OpenEventEffect::StartedFlapping {
                saved,
                transitions,
                policy,
            } => {
                feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
                notify_flapping(ctx, &saved, transitions, rule, policy, context).await;
            }
        }
    }
    Ok(())
}

enum OpenEventEffect {
    Updated(alert_events::Model),
    StartedFlapping {
        saved: alert_events::Model,
        transitions: u32,
        policy: EpisodePolicy,
    },
}

/// Atualiza o evento já aberto de (regra, alvo) dentro da transação `begin_write`:
/// recaída dentro da janela ou simples continuação do problema.
///
/// Em ambos os casos `lastProblemAt` avança para `now` — a janela de
/// resolução conta do **último** problema. Nunca há notificação aqui: recaída
/// é atualização silenciosa de tela (`alert:updated`), não alerta novo. A única
/// exceção é a **declaração de flapping** (Fase 3), que avisa uma vez que o
/// alvo é cronicamente instável e a partir daí volta ao silêncio. O
/// `problemKind` é reavaliado a cada passagem: a recaída pode ter causa
/// diferente da queda original.
async fn update_open_event_in_txn<C: sea_orm::ConnectionTrait>(
    txn: &C,
    open: &alert_events::Model,
    rule: &alert_rules::Model,
    condition: &AlertRuleCondition,
    context: &AlertEvaluationContext,
) -> AppResult<Option<OpenEventEffect>> {
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
            let saved = episode::persist_on(txn, open, status, data).await?;
            Ok(Some(OpenEventEffect::Updated(saved)))
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
            let saved = episode::persist_on(txn, open, AlertStatus::Flapping, data).await?;
            Ok(Some(OpenEventEffect::StartedFlapping {
                saved,
                transitions,
                policy,
            }))
        }
        Transition::ProblemOngoing => {
            data.insert(
                state_machine::LAST_PROBLEM_AT.into(),
                json!(now.to_rfc3339()),
            );
            let mut active: alert_events::ActiveModel = open.clone().into();
            // Fato de estado (ver `refresh_open`): a tela mostra o agora, não
            // o de quando o alerta abriu. O que é do episódio fica.
            let refreshed =
                context.refresh_open && (context.message.is_some() || !context.data.is_empty());
            if context.refresh_open {
                data.extend(context.data.clone());
                if let Some(message) = &context.message {
                    active.message = Set(Some(message.clone()));
                }
                active.updated_at = Set(now.into());
            }
            active.data = Set(Some(Value::Object(data)));
            let saved = active.update(txn).await?;
            Ok(refreshed.then_some(OpenEventEffect::Updated(saved)))
        }
        // Com a condição batendo a máquina só devolve os três ramos acima.
        _ => Ok(None),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_schema::{MigratorTrait, TestMigrator as Migrator};

    async fn banco() -> sea_orm::DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    async fn regra(db: &sea_orm::DatabaseConnection) -> i64 {
        let now = Utc::now();
        alert_rules::ActiveModel {
            name: Set("Queda".into()),
            r#type: Set("monitor".into()),
            condition: Set(json!({})),
            severity: Set("critical".into()),
            duration_seconds: Set(0),
            recovery_window_seconds: Set(0),
            flap_threshold: Set(0),
            flap_window_seconds: Set(0),
            notification_cooldown_seconds: Set(0),
            inhibit_when_parent_down: Set(false),
            enabled: Set(true),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("regra")
        .id
    }

    async fn evento(
        db: &sea_orm::DatabaseConnection,
        rule_id: i64,
        status: AlertStatus,
    ) -> Result<alert_events::Model, sea_orm::DbErr> {
        let now = Utc::now();
        alert_events::ActiveModel {
            alert_rule_id: Set(Some(rule_id)),
            scope_key: Set(Some("monitor:1".into())),
            status: Set(status.as_str().into()),
            severity: Set("critical".into()),
            started_at: Set(now.into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(db)
        .await
    }

    #[tokio::test]
    async fn o_banco_recusa_um_segundo_alerta_aberto_para_a_mesma_regra_e_alvo() {
        let db = banco().await;
        let rule_id = regra(&db).await;
        evento(&db, rule_id, AlertStatus::Active)
            .await
            .expect("primeiro");

        let segundo = evento(&db, rule_id, AlertStatus::Recovering)
            .await
            .expect_err("dois abertos");
        assert!(netmonitor_core::shared::db_errors::is_unique_violation(
            &segundo
        ));

        // O histórico fechado não colide com nada.
        evento(&db, rule_id, AlertStatus::Resolved)
            .await
            .expect("resolvido");
        evento(&db, rule_id, AlertStatus::Resolved)
            .await
            .expect("resolvido");
        let aberto = find_open_event(&db, rule_id, "monitor:1")
            .await
            .expect("busca");
        assert!(aberto.is_some());
    }
}
