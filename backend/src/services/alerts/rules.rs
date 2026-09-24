//! Cadastro de regras de alerta: validação, gravação, evento e auditoria.
//!
//! É o caminho único para criar, alterar e excluir regras — a tela (via
//! controller) e o Assistente IA (via ferramenta confirmada pelo usuário)
//! passam por aqui, com as mesmas recusas e o mesmo registro de auditoria.

use loco_rs::prelude::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde_json::{json, Value};

use super::{evaluator::Operator, fields::ALERT_FIELDS};
use crate::{
    dtos::resources::AlertRuleInput,
    models::_entities::{alert_events, alert_rules},
    services::{
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        events::EventBus,
        shared::errors::{AppError, AppResult},
    },
    views::alerts::{rule_event_payload, AlertRuleResponse},
};

/// Severidades aceitas numa regra.
pub const SEVERITIES: [&str; 3] = ["critical", "warning", "info"];

/// O front envia a regra em linguagem simples (métrica/operador/valor); aqui
/// garantimos o formato `{field, operator, value}` esperado pelo avaliador.
#[must_use]
pub fn normalize_condition(condition: &Value) -> Option<Value> {
    let object = condition.as_object()?;
    let field = object.get("field")?.as_str()?;
    let operator = object.get("operator")?.as_str()?;
    Some(json!({
        "field": field,
        "operator": operator,
        "value": object.get("value").cloned().unwrap_or(Value::Null),
    }))
}

/// Confere a condição contra o vocabulário do motor.
///
/// A tela só oferece campos e operadores válidos; quem monta a condição à mão
/// (a IA) pode inventar um `cpuLoad` que regra nenhuma jamais avaliaria — a
/// regra nasceria muda. Por isso a recusa diz quais valores existem.
///
/// # Errors
///
/// Campo fora de [`ALERT_FIELDS`], operador desconhecido ou valor ausente.
pub fn check_vocabulary(condition: &Value) -> AppResult<()> {
    let field = condition["field"].as_str().unwrap_or_default();
    if !ALERT_FIELDS.contains(&field) {
        return Err(AppError::validation(format!(
            "Campo '{field}' não existe no vocabulário de alertas. Campos válidos: {}",
            ALERT_FIELDS.join(", ")
        )));
    }
    let operator = condition["operator"].as_str().unwrap_or_default();
    if Operator::parse(operator).is_none() {
        return Err(AppError::validation(format!(
            "Operador '{operator}' inválido. Use eq, neq, gt, gte, lt, lte ou contains."
        )));
    }
    if condition["value"].is_null() {
        return Err(AppError::validation(
            "Informe o valor de referência da condição.",
        ));
    }
    Ok(())
}

fn invalid_condition() -> AppError {
    AppError::validation(
        "Condição inválida. Informe a métrica alvo, a comparação e o valor de referência da regra.",
    )
}

/// A janela de recuperação é um tempo de estabilidade: negativo não faz
/// sentido e seria tratado como zero pela máquina de estados — melhor recusar
/// do que gravar um valor que não faz o que diz.
fn invalid_recovery_window() -> AppError {
    AppError::validation(
        "Janela de recuperação inválida. Informe zero ou mais segundos de estabilidade exigida.",
    )
}

/// Mesma lógica da janela de recuperação: os limiares de flapping são
/// contagens e tempos: negativos seriam tratados como "desligado" pela máquina
/// de estados, e gravar um valor que não faz o que diz confunde quem configura.
fn invalid_flap_settings() -> AppError {
    AppError::validation(
        "Limiar de oscilação inválido. Informe zero ou mais recaídas e uma janela não negativa.",
    )
}

/// O cooldown é um intervalo de silêncio: negativo seria lido como "desligado"
/// pela política de notificação — mesma recusa das outras janelas.
fn invalid_cooldown() -> AppError {
    AppError::validation(
        "Intervalo entre notificações inválido. Informe zero ou mais segundos de silêncio.",
    )
}

/// Aplica um campo opcional não negativo: ausente mantém o atual.
fn non_negative(
    input: Option<i32>,
    current: i32,
    invalid: fn() -> AppError,
) -> Result<i32, AppError> {
    match input {
        Some(value) if value < 0 => Err(invalid()),
        Some(value) => Ok(value),
        None => Ok(current),
    }
}

/// Publicação de evento é best-effort: a gravação já concluiu quando chegamos aqui.
pub async fn publish(ctx: &AppContext, kind: &str, payload: Value) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus.publish(&ctx.db, kind, payload).await {
            tracing::warn!(%error, event = kind, "falha ao publicar evento de regra");
        }
    }
}

/// Auditoria também é best-effort: não desfaz a gravação.
async fn audit(ctx: &AppContext, actor: AuditActor, entry: AuditEntryInput) {
    let _ = AuditService::new(&ctx.db).log(actor, entry).await;
}

/// # Errors
///
/// Regra inexistente ou erro do banco.
pub async fn find(ctx: &AppContext, id: i64) -> AppResult<alert_rules::Model> {
    alert_rules::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Regra de alerta não encontrada"))
}

/// Quantos alertas a regra já registrou — somem junto com ela
/// (`alert_events.alert_rule_id` apaga em cascata).
///
/// # Errors
///
/// Erro do banco.
pub async fn event_count(ctx: &AppContext, id: i64) -> AppResult<u64> {
    Ok(alert_events::Entity::find()
        .filter(alert_events::Column::AlertRuleId.eq(id))
        .count(&ctx.db)
        .await?)
}

/// Cria a regra, avisa as telas e registra quem criou.
///
/// # Errors
///
/// Condição, nome ou janela inválidos, ou erro do banco.
pub async fn create(
    ctx: &AppContext,
    input: AlertRuleInput,
    actor: AuditActor,
) -> AppResult<alert_rules::Model> {
    let condition = input
        .condition
        .as_ref()
        .and_then(normalize_condition)
        .ok_or_else(invalid_condition)?;
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Nome da regra é obrigatório"))?;
    let recovery_window_seconds = input.recovery_window_seconds.unwrap_or(0);
    if recovery_window_seconds < 0 {
        return Err(invalid_recovery_window());
    }
    // Default da coluna: detecção desligada, mas com uma janela sensata já
    // pronta para quem só ligar o limiar depois.
    let flap_threshold = non_negative(input.flap_threshold, 0, invalid_flap_settings)?;
    let flap_window_seconds = non_negative(input.flap_window_seconds, 900, invalid_flap_settings)?;
    let notification_cooldown_seconds =
        non_negative(input.notification_cooldown_seconds, 0, invalid_cooldown)?;

    let rule = alert_rules::ActiveModel {
        // Na criação, campo ausente e `null` significam a mesma coisa — a
        // regra nasce sem aquela dimensão de escopo.
        site_id: Set(input.site_id.flatten()),
        device_id: Set(input.device_id.flatten()),
        monitor_id: Set(input.monitor_id.flatten()),
        name: Set(name.to_string()),
        r#type: Set(input.rule_type.unwrap_or_else(|| "custom".into())),
        condition: Set(condition),
        severity: Set(input.severity.unwrap_or_else(|| "warning".into())),
        duration_seconds: Set(input.duration_seconds.unwrap_or(0)),
        recovery_window_seconds: Set(recovery_window_seconds),
        flap_threshold: Set(flap_threshold),
        flap_window_seconds: Set(flap_window_seconds),
        notification_cooldown_seconds: Set(notification_cooldown_seconds),
        inhibit_when_parent_down: Set(input.inhibit_when_parent_down.unwrap_or(false)),
        enabled: Set(input.enabled.unwrap_or(true)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    publish(ctx, "alert_rule:created", rule_event_payload(&rule)).await;
    audit(
        ctx,
        actor,
        AuditEntryInput {
            action: AuditAction::Create,
            resource_type: ResourceType::AlertRule,
            resource_id: Some(rule.id),
            resource_label: Some(rule.name.clone()),
            description: Some(format!("Regra de alerta '{}' criada", rule.name)),
            changes: None,
        },
    )
    .await;
    Ok(rule)
}

/// Altera a regra: campo ausente mantém o valor atual (o toggle da lista
/// manda só `enabled`).
///
/// # Errors
///
/// Regra inexistente, condição ou janela inválidas, ou erro do banco.
pub async fn update(
    ctx: &AppContext,
    id: i64,
    input: AlertRuleInput,
    actor: AuditActor,
) -> AppResult<alert_rules::Model> {
    let current = find(ctx, id).await?;
    let old_response = AlertRuleResponse::from(current.clone());

    let condition = match input.condition.as_ref() {
        Some(raw) => normalize_condition(raw).ok_or_else(invalid_condition)?,
        None => current.condition.clone(),
    };
    let recovery_window_seconds = match input.recovery_window_seconds {
        Some(value) if value < 0 => return Err(invalid_recovery_window()),
        Some(value) => value,
        None => current.recovery_window_seconds,
    };
    let flap_threshold = non_negative(
        input.flap_threshold,
        current.flap_threshold,
        invalid_flap_settings,
    )?;
    let flap_window_seconds = non_negative(
        input.flap_window_seconds,
        current.flap_window_seconds,
        invalid_flap_settings,
    )?;
    let notification_cooldown_seconds = non_negative(
        input.notification_cooldown_seconds,
        current.notification_cooldown_seconds,
        invalid_cooldown,
    )?;

    let rule = alert_rules::ActiveModel {
        id: Set(id),
        // `unwrap_or` sobre a dupla opção: campo ausente mantém o atual,
        // `null` explícito limpa. Ver a nota do `AlertRuleInput`.
        site_id: Set(input.site_id.unwrap_or(current.site_id)),
        device_id: Set(input.device_id.unwrap_or(current.device_id)),
        monitor_id: Set(input.monitor_id.unwrap_or(current.monitor_id)),
        name: Set(input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&current.name)
            .to_string()),
        r#type: Set(input.rule_type.unwrap_or(current.r#type)),
        condition: Set(condition),
        severity: Set(input.severity.unwrap_or(current.severity)),
        duration_seconds: Set(input.duration_seconds.unwrap_or(current.duration_seconds)),
        recovery_window_seconds: Set(recovery_window_seconds),
        flap_threshold: Set(flap_threshold),
        flap_window_seconds: Set(flap_window_seconds),
        notification_cooldown_seconds: Set(notification_cooldown_seconds),
        inhibit_when_parent_down: Set(input
            .inhibit_when_parent_down
            .unwrap_or(current.inhibit_when_parent_down)),
        enabled: Set(input.enabled.unwrap_or(current.enabled)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;

    publish(ctx, "alert_rule:updated", rule_event_payload(&rule)).await;
    audit(
        ctx,
        actor,
        AuditEntryInput {
            action: AuditAction::Update,
            resource_type: ResourceType::AlertRule,
            resource_id: Some(rule.id),
            resource_label: Some(rule.name.clone()),
            description: Some(format!("Regra de alerta '{}' atualizada", rule.name)),
            changes: Some(AuditChanges {
                old: serde_json::to_value(old_response).ok(),
                new: serde_json::to_value(AlertRuleResponse::from(rule.clone())).ok(),
            }),
        },
    )
    .await;
    Ok(rule)
}

/// Exclui a regra — e, pela cascata do banco, os alertas que ela registrou.
/// Devolve a regra como era.
///
/// # Errors
///
/// Regra inexistente ou erro do banco.
pub async fn delete(ctx: &AppContext, id: i64, actor: AuditActor) -> AppResult<alert_rules::Model> {
    let rule = find(ctx, id).await?;
    // O payload é montado antes do DELETE: depois dele a linha não existe mais.
    let payload = rule_event_payload(&rule);
    let old_response = AlertRuleResponse::from(rule.clone());
    alert_rules::Entity::delete_by_id(id).exec(&ctx.db).await?;
    publish(ctx, "alert_rule:deleted", payload).await;
    audit(
        ctx,
        actor,
        AuditEntryInput {
            action: AuditAction::Delete,
            resource_type: ResourceType::AlertRule,
            resource_id: Some(id),
            resource_label: Some(rule.name.clone()),
            description: Some(format!("Regra de alerta '{}' excluída", rule.name)),
            changes: Some(AuditChanges {
                old: serde_json::to_value(old_response).ok(),
                new: None,
            }),
        },
    )
    .await;
    Ok(rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condicao_e_normalizada_para_os_tres_campos() {
        let normalized = normalize_condition(&json!({
            "field": "latencyMs", "operator": "gt", "value": 200, "extra": "ignorado"
        }))
        .expect("condição válida");
        assert_eq!(
            normalized,
            json!({ "field": "latencyMs", "operator": "gt", "value": 200 })
        );
    }

    #[test]
    fn condicao_sem_valor_vira_null_e_nao_erro() {
        let normalized = normalize_condition(&json!({ "field": "status", "operator": "eq" }))
            .expect("field e operator bastam");
        assert_eq!(normalized["value"], Value::Null);
    }

    #[test]
    fn condicao_invalida_e_recusada() {
        assert!(normalize_condition(&json!({ "operator": "eq", "value": 1 })).is_none());
        assert!(normalize_condition(&json!({ "field": "status", "operator": 5 })).is_none());
        assert!(normalize_condition(&json!(["status", "eq", 1])).is_none());
        assert!(normalize_condition(&Value::Null).is_none());
    }

    #[test]
    fn condicao_invalida_devolve_422_com_mensagem_estavel() {
        assert_eq!(
            invalid_condition().status(),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY
        );
        assert!(invalid_condition()
            .to_string()
            .starts_with("Condição inválida."));
    }

    #[test]
    fn vocabulario_recusa_campo_inventado_e_operador_desconhecido() {
        let ok = json!({ "field": "cpuUsagePercent", "operator": "gte", "value": 90 });
        assert!(check_vocabulary(&ok).is_ok());

        let campo = json!({ "field": "cpuLoad", "operator": "gt", "value": 90 });
        let erro = check_vocabulary(&campo).unwrap_err().to_string();
        assert!(erro.contains("cpuLoad") && erro.contains("cpuUsagePercent"));

        let operador = json!({ "field": "latencyMs", "operator": ">", "value": 90 });
        assert!(check_vocabulary(&operador).is_err());

        let sem_valor = json!({ "field": "status", "operator": "eq", "value": null });
        assert!(check_vocabulary(&sem_valor).is_err());
    }

    #[test]
    fn campos_nao_negativos_mantem_o_atual_quando_ausentes() {
        assert_eq!(non_negative(None, 7, invalid_cooldown).unwrap(), 7);
        assert_eq!(non_negative(Some(0), 7, invalid_cooldown).unwrap(), 0);
        assert!(non_negative(Some(-1), 7, invalid_cooldown).is_err());
    }
}
