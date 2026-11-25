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
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, alert_rules},
    services::{
        alerts::{
            contracts::{AlertScopeKey, AlertStatus},
            episode, feed, problem_kind, silence,
            state_machine::{self, EpisodeInput, EpisodePolicy, Transition},
        },
        notifications::{
            formatter,
            outbox::{self, NotificationRequest},
            policy::{NotificationKind, NotificationPolicy},
            Severity,
        },
        shared::{errors::AppResult, transaction::begin_write},
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

/// Degradação observada (`warning`: perda parcial de pacotes, DNS parcial)
/// num escopo com eventos abertos (Fase 2).
///
/// O alvo não está recuperado nem a regra bateu — mas o episódio não pode
/// seguir contando estabilidade: cada evento aberto carimba
/// `lastProblemAt = now` e, se estava em `recovering`, vira recaída (sem
/// notificação, como qualquer recaída). Nenhum evento novo nasce aqui:
/// `warning` nunca dispara uma regra cuja condição não bateu.
///
/// Devolve quantos eventos foram tocados.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn note_degraded_scope(
    ctx: &AppContext,
    scope_key: &str,
    kind: problem_kind::ProblemKind,
) -> AppResult<usize> {
    let open = open_events_of(ctx, scope_key).await?;
    if open.is_empty() {
        return Ok(0);
    }
    // A degradação também conta para a detecção de flapping (Fase 3), então
    // precisa da política da regra — não basta a janela zerada da Fase 2.
    let policies = policies_of(ctx, &open).await?;
    let mut touched = 0;

    for listed in open {
        // Releitura dentro da transação de escrita: o `data` gravado sai da
        // linha atual, não da lista lida antes — senão um reconhecimento ou
        // silêncio concorrente (`acknowledgedAt`, `silencedUntil`) some.
        let txn = begin_write(&ctx.db).await?;
        let Some(event) = reload_open(&txn, listed.id).await? else {
            txn.commit().await?;
            continue;
        };
        let now = Utc::now();
        let rule_policies = policy_for(&policies, &event);
        let policy = rule_policies.episode;
        let status = event.status.parse().unwrap_or(AlertStatus::Active);
        let transition = state_machine::decide(&EpisodeInput {
            status,
            data: event.data.as_ref().and_then(Value::as_object),
            started_at: event.started_at.with_timezone(&Utc),
            policy,
            condition_matched: false,
            degraded: true,
            recovered: false,
            silenced_now: silence::silenced_until(&event).is_some_and(|until| until > now),
            now,
        });

        let mut data = event
            .data
            .as_ref()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        // A degradação pode ter causa diferente da queda original.
        data.insert(problem_kind::PROBLEM_KIND.into(), json!(kind.as_str()));

        match transition {
            Transition::Relapse { recurrence, status } => {
                touched += 1;
                episode::mark_relapse(&mut data, recurrence, now, policy);
                let saved = episode::persist_on(&txn, &event, status, data).await?;
                txn.commit().await?;
                feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
            }
            Transition::StartFlapping {
                recurrence,
                transitions,
            } => {
                touched += 1;
                episode::mark_relapse(&mut data, recurrence, now, policy);
                data.insert(
                    state_machine::FLAPPING_SINCE.into(),
                    json!(now.to_rfc3339()),
                );
                let saved = episode::persist_on(&txn, &event, AlertStatus::Flapping, data).await?;
                txn.commit().await?;
                feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
                notify_flapping(ctx, &saved, transitions, rule_policies).await;
            }
            Transition::ProblemOngoing => {
                touched += 1;
                data.insert(
                    state_machine::LAST_PROBLEM_AT.into(),
                    json!(now.to_rfc3339()),
                );
                let mut active: alert_events::ActiveModel = event.into();
                active.data = Set(Some(Value::Object(data)));
                active.update(&txn).await?;
                txn.commit().await?;
            }
            // Evento fechado não é tocado; `Resolve`/`EnterRecovering` não
            // aparecem neste caminho (`recovered` é falso aqui).
            _ => txn.commit().await?,
        }
    }

    Ok(touched)
}

/// O aviso único de "alvo oscilando", emitido pela declaração de flapping.
async fn notify_flapping(
    ctx: &AppContext,
    event: &alert_events::Model,
    transitions: u32,
    policies: RulePolicies,
) {
    let fallback = event.message.as_deref().unwrap_or("Alerta do sistema");
    let request = NotificationRequest {
        alert_rule_id: event.alert_rule_id,
        scope_key: event.scope_key.clone(),
        device_id: event.device_id,
        kind: NotificationKind::Flapping,
        policy: policies.notification,
        silenced: false,
        message: formatter::alert_flapping(
            &episode::title_of(event, fallback),
            transitions,
            policies.episode.flap_window_seconds,
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

/// Os eventos abertos do escopo canônico.
async fn open_events_of(ctx: &AppContext, scope_key: &str) -> AppResult<Vec<alert_events::Model>> {
    Ok(alert_events::Entity::find()
        .filter(alert_events_entity::Column::ScopeKey.eq(scope_key))
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .all(&ctx.db)
        .await?)
}

/// Tudo que a regra dita sobre um episódio: como ele fecha e como ele avisa.
///
/// As duas viajam juntas porque saem da mesma linha e são lidas no mesmo laço —
/// separá-las custaria uma segunda consulta às mesmas regras.
#[derive(Debug, Clone, Copy, Default)]
struct RulePolicies {
    episode: EpisodePolicy,
    notification: NotificationPolicy,
}

/// As políticas das regras dos eventos, carregadas de uma vez.
async fn policies_of(
    ctx: &AppContext,
    events: &[alert_events::Model],
) -> AppResult<HashMap<i64, RulePolicies>> {
    let rule_ids: Vec<i64> = events
        .iter()
        .filter_map(|event| event.alert_rule_id)
        .collect();
    if rule_ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(alert_rules::Entity::find()
        .filter(alert_rules::Column::Id.is_in(rule_ids))
        .all(&ctx.db)
        .await?
        .iter()
        .map(|rule| {
            (
                rule.id,
                RulePolicies {
                    episode: EpisodePolicy::from(rule),
                    notification: NotificationPolicy::from(rule),
                },
            )
        })
        .collect())
}

/// A política do evento; regra apagada não tem janela, limiar nem cooldown a
/// respeitar.
fn policy_for(policies: &HashMap<i64, RulePolicies>, event: &alert_events::Model) -> RulePolicies {
    event
        .alert_rule_id
        .and_then(|rule_id| policies.get(&rule_id))
        .copied()
        .unwrap_or_default()
}

/// A janela de estabilidade só vale para quem é observado de novo.
///
/// Um monitor checa o alvo a cada ciclo: a janela espera uma **nova** checagem
/// ok antes de fechar. Um alerta de evento (túnel VPN, status de interface)
/// recebe a volta uma vez só — o túnel reconectou, a interface subiu — e não
/// haverá segunda leitura para confirmar: com a janela, o alerta entrava em
/// `recovering` e ficava preso lá para sempre. Para ele, a própria volta fecha.
fn observed_policy(policy: EpisodePolicy, observed_again: bool) -> EpisodePolicy {
    if observed_again {
        policy
    } else {
        EpisodePolicy {
            recovery_window_seconds: 0,
            ..policy
        }
    }
}

async fn run(
    ctx: &AppContext,
    scope_key: &str,
    reason: &str,
    respect_windows: bool,
) -> AppResult<usize> {
    let open = open_events_of(ctx, scope_key).await?;
    if open.is_empty() {
        return Ok(0);
    }

    // A política de cada evento vem da sua regra, carregada de uma vez. Regra
    // apagada (ou fechamento administrativo) = política zerada: resolve na
    // hora, sem janela e sem limiar de flap a respeitar.
    let policies = if respect_windows {
        policies_of(ctx, &open).await?
    } else {
        HashMap::new()
    };

    let mut resolved = 0;

    for listed in open {
        // Mesma releitura de `note_degraded_scope`: decide e grava sobre a
        // linha atual, dentro da transação.
        let txn = begin_write(&ctx.db).await?;
        let Some(event) = reload_open(&txn, listed.id).await? else {
            txn.commit().await?;
            continue;
        };
        let now = Utc::now();
        let rule_policies = policy_for(&policies, &event);
        let policy = observed_policy(rule_policies.episode, event.monitor_id.is_some());
        // Status fora do vocabulário (linha antiga) é tratado como `active`:
        // o pior caso é entrar em observação quando poderia fechar.
        let status = event.status.parse().unwrap_or(AlertStatus::Active);
        // O silêncio é lido pelo prazo, não pelo status: um alerta silenciado
        // que entrou em `recovering` perdeu o rótulo mas não o pedido do
        // operador — era por aí que o ✅ furava o silêncio (F8 da análise).
        let silenced = silence::silenced_until(&event).is_some_and(|until| until > now);
        let transition = state_machine::decide(&EpisodeInput {
            status,
            data: event.data.as_ref().and_then(Value::as_object),
            started_at: event.started_at.with_timezone(&Utc),
            policy,
            condition_matched: false,
            degraded: false,
            recovered: true,
            silenced_now: silenced,
            now,
        });

        match transition {
            Transition::Resolve { summary } => {
                resolved += 1;
                let problem = event
                    .data
                    .as_ref()
                    .and_then(|data| data.get(problem_kind::PROBLEM_KIND))
                    .and_then(Value::as_str)
                    .and_then(problem_kind::ProblemKind::parse);
                let saved = persist_resolve(&txn, event, now).await?;
                txn.commit().await?;
                let request = NotificationRequest {
                    alert_rule_id: saved.alert_rule_id,
                    scope_key: saved.scope_key.clone(),
                    device_id: saved.device_id,
                    kind: NotificationKind::Resolved,
                    policy: rule_policies.notification,
                    silenced,
                    message: formatter::alert_resolved(
                        saved.id,
                        saved.message.as_deref(),
                        reason,
                        Some(summary),
                        problem,
                        json!({
                            "alertEventId": saved.id,
                            "scopeKey": scope_key,
                            "monitorId": saved.monitor_id,
                        }),
                    ),
                };
                if let Err(error) = outbox::enqueue(ctx, request).await {
                    tracing::warn!(
                        %error,
                        alert_event_id = saved.id,
                        "falha ao enfileirar notificação de resolução"
                    );
                }
                feed::publish(ctx, "alert:resolved", feed::event_payload(&saved)).await;
            }
            Transition::EnterRecovering => {
                // Entrada em recovering não notifica: a notícia boa de verdade
                // é a resolução ao fim da janela.
                let saved = persist_enter_recovering(&txn, event).await?;
                txn.commit().await?;
                feed::publish(ctx, "alert:updated", feed::event_payload(&saved)).await;
            }
            // `None` não aparece neste caminho: `condition_matched` é sempre
            // falso aqui, então recaída e "problema em curso" são impossíveis.
            _ => txn.commit().await?,
        }
    }

    Ok(resolved)
}

/// O evento como está agora, se ainda aberto.
async fn reload_open<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> AppResult<Option<alert_events::Model>> {
    Ok(alert_events::Entity::find_by_id(id)
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .one(db)
        .await?)
}

/// Persiste o fechamento do episódio.
async fn persist_resolve<C: ConnectionTrait>(
    db: &C,
    event: alert_events::Model,
    resolved_at: chrono::DateTime<Utc>,
) -> AppResult<alert_events::Model> {
    let mut active: alert_events::ActiveModel = event.into();
    active.status = Set(AlertStatus::Resolved.as_str().into());
    active.resolved_at = Set(Some(resolved_at.into()));
    Ok(active.update(db).await?)
}

/// Persiste a entrada em observação de estabilidade.
///
/// O `lastProblemAt` registrado é mantido — é ele que ancora a janela no
/// último problema. Na falta (evento aberto antes da Fase 1), ancora na
/// abertura do evento, e o contador de recaídas nasce em zero.
async fn persist_enter_recovering<C: ConnectionTrait>(
    db: &C,
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
    Ok(active.update(db).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decide_recovery(policy: EpisodePolicy, status: AlertStatus) -> Transition {
        let now = Utc::now();
        state_machine::decide(&EpisodeInput {
            status,
            data: None,
            started_at: now - chrono::Duration::days(6),
            policy,
            condition_matched: false,
            degraded: false,
            recovered: true,
            silenced_now: false,
            now,
        })
    }

    #[test]
    fn alerta_de_evento_fecha_na_propria_volta() {
        // O túnel VPN e a interface: janela de 120 s na regra, volta única.
        let rule = EpisodePolicy {
            recovery_window_seconds: 120,
            ..EpisodePolicy::default()
        };
        assert!(matches!(
            decide_recovery(observed_policy(rule, false), AlertStatus::Acknowledged),
            Transition::Resolve { .. }
        ));
    }

    #[test]
    fn monitor_continua_esperando_a_janela() {
        let rule = EpisodePolicy {
            recovery_window_seconds: 120,
            ..EpisodePolicy::default()
        };
        assert_eq!(observed_policy(rule, true), rule);
        assert!(matches!(
            decide_recovery(observed_policy(rule, true), AlertStatus::Active),
            Transition::EnterRecovering
        ));
    }
}
