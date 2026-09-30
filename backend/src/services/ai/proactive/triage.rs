//! Triagem do resumo automático pelo Laya.
//!
//! - **Desligada** (padrão): nada muda — vale severidade e teto por hora.
//! - **Só registrar**: o Laya opina, a opinião fica no alerta, e o resumo sai
//!   sempre. Serve para calibrar antes de deixar o Laya decidir.
//! - **Decidir**: alerta que o Laya não acha acionável fica sem resumo — e
//!   sem gastar o teto por hora. A tela mostra o porquê e oferece "Gerar
//!   resumo agora".
//!
//! Nunca acrescenta chamada ao LLM. Laya fora do ar = comportamento de hoje.

use chrono::{Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::{
    models::{
        _entities::{alert_events as alert_events_entity, alert_rules, devices},
        alert_events,
    },
    services::{
        ai::laya::{
            config::{LayaFeature, LayaTriageMode},
            decisions::incident_triage::{IncidentTriage, TriageVerdict},
            runtime::{Lane, LayaRuntime},
            suggestion::percent,
        },
        alerts::{
            contracts::AlertStatus, correlation::role_weight, feed, inhibition,
            problem_kind::PROBLEM_KIND,
        },
        shared::errors::AppResult,
    },
};

/// Campo de `alert_events.data` com a opinião do Laya.
pub const TRIAGE_FIELD: &str = "layaTriage";
pub const TRIAGE_EVENT: &str = "alert:laya_triage";

/// A opinião gravada no alerta. Percentuais 0–100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LayaTriage {
    pub actionable: f64,
    pub symptom: f64,
    pub transient: f64,
    pub model: String,
    pub mode: LayaTriageMode,
    /// O resumo automático não saiu por causa desta opinião.
    pub suppressed: bool,
    pub assessed_at: String,
}

/// O resumo sai? Desligada, só registrando ou sem opinião: sai. Decidindo:
/// só se o alerta for acionável no limiar configurado.
#[must_use]
pub fn should_summarize(
    mode: LayaTriageMode,
    verdict: Option<&TriageVerdict>,
    threshold: f64,
) -> bool {
    match (mode, verdict) {
        (LayaTriageMode::Enforce, Some(verdict)) => verdict.actionable >= threshold,
        _ => true,
    }
}

/// O que o Laya lê sobre o alerta — só consultas baratas: numa tempestade de
/// alertas, a correlação completa (que carrega o parque inteiro) não cabe
/// aqui.
async fn build_state(ctx: &AppContext, alert: &alert_events::Model) -> AppResult<String> {
    let rule = match alert.alert_rule_id {
        Some(id) => alert_rules::Entity::find_by_id(id).one(&ctx.db).await?,
        None => None,
    };
    let device = match alert.device_id {
        Some(id) => devices::Entity::find_by_id(id).one(&ctx.db).await?,
        None => None,
    };
    let parent_alerting = match alert.device_id {
        Some(id) => inhibition::explaining_ancestor(&ctx.db, id)
            .await?
            .is_some(),
        None => false,
    };
    let hour_ago = Utc::now() - Duration::hours(1);
    let mut same_target = alert_events::Entity::find()
        .filter(alert_events_entity::Column::StartedAt.gte(hour_ago))
        .filter(alert_events_entity::Column::Id.ne(alert.id));
    if let Some(rule_id) = alert.alert_rule_id {
        same_target = same_target.filter(alert_events_entity::Column::AlertRuleId.eq(rule_id));
    }
    if let Some(device_id) = alert.device_id {
        same_target = same_target.filter(alert_events_entity::Column::DeviceId.eq(device_id));
    }
    let recent_repeats = same_target.count(&ctx.db).await?;
    let others_open_now = alert_events::Entity::find()
        .filter(alert_events_entity::Column::Status.ne(AlertStatus::Resolved.as_str()))
        .filter(alert_events_entity::Column::StartedAt.gte(Utc::now() - Duration::minutes(5)))
        .filter(alert_events_entity::Column::Id.ne(alert.id))
        .count(&ctx.db)
        .await?;

    Ok(json!({
        "alert": {
            "severity": alert.severity,
            "message": alert.message,
            "problemKind": alert.data.as_ref().and_then(|data| data.get(PROBLEM_KIND)),
        },
        "rule": rule.as_ref().map(|rule| json!({ "name": rule.name, "type": rule.r#type })),
        "device": device.as_ref().map(|device| json!({
            "name": device.name,
            "type": device.r#type,
            "roleWeight": role_weight(&device.r#type),
        })),
        "parentDeviceAlerting": parent_alerting,
        "sameAlertLastHour": recent_repeats,
        "otherAlertsOpenedLast5Minutes": others_open_now,
    })
    .to_string())
}

/// Pergunta ao Laya e grava a opinião no alerta. `None` com a triagem
/// desligada, o Laya fora do ar ou o alerta sumido — e aí o resumo segue
/// como sempre.
///
/// # Errors
///
/// Erro do banco.
pub async fn assess(ctx: &AppContext, alert_id: i64) -> AppResult<Option<(LayaTriage, f64)>> {
    let runtime = LayaRuntime::from_context(ctx);
    let Some(settings) = runtime
        .settings_for(&ctx.db, LayaFeature::IncidentTriage)
        .await
    else {
        return Ok(None);
    };
    let Some(alert) = alert_events::Entity::find_by_id(alert_id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(None);
    };
    let state = build_state(ctx, &alert).await?;
    let Some(outcome) = runtime
        .decide_with(&settings, Lane::Background, &state, &IncidentTriage)
        .await
    else {
        return Ok(None);
    };
    let mode = settings.features.incident_triage;
    let threshold = settings.threshold();
    let triage = LayaTriage {
        actionable: percent(outcome.value.actionable),
        symptom: percent(outcome.value.symptom),
        transient: percent(outcome.value.transient),
        model: outcome.model,
        mode,
        suppressed: !should_summarize(mode, Some(&outcome.value), threshold),
        assessed_at: Utc::now().to_rfc3339(),
    };

    // Relê antes de gravar: o motor pode ter mexido no `data` enquanto isso.
    let Some(current) = alert_events::Entity::find_by_id(alert_id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(None);
    };
    let mut data = match current.data.clone() {
        Some(Value::Object(map)) => Value::Object(map),
        _ => json!({}),
    };
    data[TRIAGE_FIELD] = json!(triage);
    let mut active: alert_events::ActiveModel = current.into();
    active.data = Set(Some(data));
    let updated = active.update(&ctx.db).await?;
    feed::publish(
        ctx,
        TRIAGE_EVENT,
        json!({
            "id": updated.id,
            "alertEventId": updated.id,
            "deviceId": updated.device_id,
            "layaTriage": triage,
        }),
    )
    .await;
    Ok(Some((triage, threshold)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(actionable: f64) -> TriageVerdict {
        TriageVerdict {
            actionable,
            symptom: 0.0,
            transient: 0.0,
        }
    }

    #[test]
    fn so_o_modo_decidir_suprime() {
        let low = verdict(0.2);
        assert!(should_summarize(LayaTriageMode::Off, Some(&low), 0.6));
        assert!(should_summarize(LayaTriageMode::Shadow, Some(&low), 0.6));
        assert!(!should_summarize(LayaTriageMode::Enforce, Some(&low), 0.6));
        assert!(should_summarize(
            LayaTriageMode::Enforce,
            Some(&verdict(0.6)),
            0.6
        ));
        assert!(
            should_summarize(LayaTriageMode::Enforce, None, 0.6),
            "sem opinião, resume como sempre"
        );
    }
}
