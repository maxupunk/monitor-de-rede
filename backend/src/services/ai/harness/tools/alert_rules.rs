//! Regras de alerta pela IA: entender de onde um alerta vem, conhecer as
//! regras cadastradas e — com a confirmação do usuário — criar ou excluir.
//!
//! As escritas passam por [`crate::services::alerts::rules`], o mesmo serviço
//! da tela: mesmas recusas, mesmo evento SSE, mesma auditoria.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use super::{lookup::find_device, AiToolHandler, ToolArgs, ToolGroup, ToolKind, ToolOutput};
use crate::{
    dtos::resources::AlertRuleInput,
    models::_entities::{
        alert_events, alert_rules, device_interfaces, devices, monitors, sites, vpn_peers,
    },
    services::{
        ai::knowledge::ALERT_RULES_GUIDE,
        alerts::{
            contracts::AlertStatus,
            fields::ALERT_FIELDS,
            rules::{self, SEVERITIES},
        },
        shared::errors::{AppError, AppResult},
    },
};

const OPERATORS: [&str; 7] = ["eq", "neq", "gt", "gte", "lt", "lte", "contains"];
/// Disparos nas últimas 24 h a partir dos quais a regra é chamada de ruidosa.
const NOISY_FIRINGS_24H: u64 = 10;
const MAX_RULES: i64 = 50;
/// Teto das janelas aceitas pela IA: um dia.
const MAX_WINDOW_SECONDS: i64 = 86_400;

/// `latencyMs gt 150`: a condição como a IA e o usuário leem.
fn condition_text(condition: &Value) -> String {
    let value = match &condition["value"] {
        Value::String(text) => format!("\"{text}\""),
        other => other.to_string(),
    };
    format!(
        "{} {} {value}",
        condition["field"].as_str().unwrap_or("?"),
        condition["operator"].as_str().unwrap_or("?"),
    )
}

/// Onde a regra vale, com nomes no lugar dos ids.
async fn scope_label(ctx: &AppContext, rule: &alert_rules::Model) -> AppResult<String> {
    let mut parts = Vec::new();
    if let Some(id) = rule.monitor_id {
        let name = monitors::Entity::find_by_id(id)
            .one(&ctx.db)
            .await?
            .map_or_else(|| format!("#{id}"), |monitor| monitor.name);
        parts.push(format!("monitor {name}"));
    }
    if let Some(id) = rule.device_id {
        let name = devices::Entity::find_by_id(id)
            .one(&ctx.db)
            .await?
            .map_or_else(|| format!("#{id}"), |device| device.name);
        parts.push(format!("dispositivo {name}"));
    }
    if let Some(id) = rule.site_id {
        let name = sites::Entity::find_by_id(id)
            .one(&ctx.db)
            .await?
            .map_or_else(|| format!("#{id}"), |site| site.name);
        parts.push(format!("site {name}"));
    }
    Ok(if parts.is_empty() {
        "global (todos os dispositivos)".to_string()
    } else {
        parts.join(", ")
    })
}

/// Disparos de cada regra desde `since`.
async fn firings_since(
    ctx: &AppContext,
    rule_ids: Vec<i64>,
    since: chrono::DateTime<Utc>,
) -> AppResult<HashMap<i64, u64>> {
    if rule_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<Option<i64>> = alert_events::Entity::find()
        .select_only()
        .column(alert_events::Column::AlertRuleId)
        .filter(alert_events::Column::AlertRuleId.is_in(rule_ids))
        .filter(alert_events::Column::StartedAt.gte(since))
        .into_tuple()
        .all(&ctx.db)
        .await?;
    let mut counts = HashMap::new();
    for id in rows.into_iter().flatten() {
        *counts.entry(id).or_insert(0) += 1;
    }
    Ok(counts)
}

pub struct AlertRulesGuide;

#[async_trait]
impl AiToolHandler for AlertRulesGuide {
    fn name(&self) -> &'static str {
        "get_alert_rules_guide"
    }

    fn description(&self) -> &'static str {
        "Guia das regras de alerta: como um alerta nasce, ciclo de vida, todos os campos (condition.field), operadores, receitas prontas e como diagnosticar a origem de um alerta. Leia antes de criar uma regra."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::AlertRules
    }

    async fn execute(&self, _ctx: &AppContext, _args: &ToolArgs) -> AppResult<ToolOutput> {
        Ok(ToolOutput::data(json!({ "guide": ALERT_RULES_GUIDE })))
    }
}

pub struct ListAlertRules;

#[async_trait]
impl AiToolHandler for ListAlertRules {
    fn name(&self) -> &'static str {
        "list_alert_rules"
    }

    fn description(&self) -> &'static str {
        "Regras de alerta cadastradas: condição, severidade, escopo, se está ativa, alertas abertos e disparos nas últimas 24 h (ruidosa = muitos disparos)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Só as regras deste dispositivo (nome, IP ou id) e as globais" },
                "search": { "type": "string", "description": "Trecho do nome ou do campo da condição" },
                "enabled_only": { "type": "boolean" },
                "limit": { "type": "integer", "description": "Máximo de regras (1 a 50, padrão 20)" }
            }
        })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::AlertRules
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let mut query = alert_rules::Entity::find().order_by_asc(alert_rules::Column::Id);
        if args.flag("enabled_only") {
            query = query.filter(alert_rules::Column::Enabled.eq(true));
        }
        if let Some(identifier) = args.text("device") {
            let Some(device) = find_device(&ctx.db, &identifier).await? else {
                return Ok(ToolOutput::not_found(format!(
                    "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
                )));
            };
            let monitor_ids: Vec<i64> = monitors::Entity::find()
                .filter(monitors::Column::DeviceId.eq(device.id))
                .select_only()
                .column(monitors::Column::Id)
                .into_tuple()
                .all(&ctx.db)
                .await?;
            query = query.filter(
                sea_orm::Condition::any()
                    .add(alert_rules::Column::DeviceId.eq(device.id))
                    .add(alert_rules::Column::MonitorId.is_in(monitor_ids))
                    .add(
                        sea_orm::Condition::all()
                            .add(alert_rules::Column::DeviceId.is_null())
                            .add(alert_rules::Column::MonitorId.is_null())
                            .add(alert_rules::Column::SiteId.is_null()),
                    ),
            );
        }
        let search = args.text("search").map(|term| term.to_lowercase());
        let limit = args.integer_in("limit", 20, 1, MAX_RULES);
        let rules: Vec<alert_rules::Model> = query
            .all(&ctx.db)
            .await?
            .into_iter()
            .filter(|rule| {
                search.as_ref().is_none_or(|term| {
                    rule.name.to_lowercase().contains(term)
                        || condition_text(&rule.condition)
                            .to_lowercase()
                            .contains(term)
                })
            })
            .collect();
        let total = rules.len();
        let shown: Vec<alert_rules::Model> = rules.into_iter().take(limit as usize).collect();

        let ids: Vec<i64> = shown.iter().map(|rule| rule.id).collect();
        let fired = firings_since(ctx, ids.clone(), Utc::now() - Duration::hours(24)).await?;
        let open_rows: Vec<Option<i64>> = alert_events::Entity::find()
            .select_only()
            .column(alert_events::Column::AlertRuleId)
            .filter(alert_events::Column::AlertRuleId.is_in(ids))
            .filter(alert_events::Column::Status.is_in(AlertStatus::OPEN))
            .into_tuple()
            .all(&ctx.db)
            .await?;
        let mut open: HashMap<i64, u64> = HashMap::new();
        for id in open_rows.into_iter().flatten() {
            *open.entry(id).or_insert(0) += 1;
        }

        let mut items = Vec::with_capacity(shown.len());
        for rule in &shown {
            let fired_24h = fired.get(&rule.id).copied().unwrap_or(0);
            items.push(json!({
                "id": rule.id,
                "name": rule.name,
                "condition": condition_text(&rule.condition),
                "severity": rule.severity,
                "scope": scope_label(ctx, rule).await?,
                "enabled": rule.enabled,
                "duration_s": rule.duration_seconds,
                "recovery_window_s": rule.recovery_window_seconds,
                "open_alerts": open.get(&rule.id).copied().unwrap_or(0),
                "fired_24h": fired_24h,
                "noisy": fired_24h >= NOISY_FIRINGS_24H,
            }));
        }
        Ok(ToolOutput::data(json!({ "total": total, "rules": items })))
    }
}

/// O alvo de um alerta, pela chave de escopo (`monitor:12`, `interface:34`...).
async fn describe_target(ctx: &AppContext, scope_key: Option<&str>) -> AppResult<Value> {
    let Some((kind, id)) = scope_key.and_then(|key| key.split_once(':')) else {
        return Ok(Value::Null);
    };
    let Ok(id) = id.parse::<i64>() else {
        return Ok(json!({ "kind": kind }));
    };
    Ok(match kind {
        "monitor" => match monitors::Entity::find_by_id(id).one(&ctx.db).await? {
            Some(monitor) => json!({
                "kind": "monitor",
                "id": monitor.id,
                "name": monitor.name,
                "type": monitor.r#type,
                "status": monitor.status,
                "interval_s": monitor.interval_seconds,
                "enabled": monitor.enabled,
            }),
            None => json!({ "kind": "monitor", "id": id, "missing": true }),
        },
        "interface" => match device_interfaces::Entity::find_by_id(id)
            .one(&ctx.db)
            .await?
        {
            Some(iface) => json!({ "kind": "interface", "id": iface.id, "name": iface.name }),
            None => json!({ "kind": "interface", "id": id, "missing": true }),
        },
        "vpn_peer" => match vpn_peers::Entity::find_by_id(id).one(&ctx.db).await? {
            Some(peer) => {
                let device = devices::Entity::find_by_id(peer.device_id)
                    .one(&ctx.db)
                    .await?
                    .map(|device| device.name);
                json!({ "kind": "vpn_peer", "id": peer.id, "device": device })
            }
            None => json!({ "kind": "vpn_peer", "id": id, "missing": true }),
        },
        other => json!({ "kind": other, "id": id }),
    })
}

pub struct ExplainAlert;

#[async_trait]
impl AiToolHandler for ExplainAlert {
    fn name(&self) -> &'static str {
        "explain_alert"
    }

    fn description(&self) -> &'static str {
        "De onde vem um alerta: a regra que disparou (condição, janelas, escopo), o alvo (monitor, interface, túnel VPN), os fatos que casaram, disparos da regra nas últimas 24 h e outros alertas abertos no mesmo dispositivo."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "alert_id": { "type": "integer", "description": "Id do alerta (de get_alerts)" }
            },
            "required": ["alert_id"]
        })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::AlertRules
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Some(id) = args.integer("alert_id") else {
            return Ok(ToolOutput::not_found("Informe alert_id (de get_alerts)"));
        };
        let Some(event) = alert_events::Entity::find_by_id(id).one(&ctx.db).await? else {
            return Ok(ToolOutput::not_found(format!(
                "Alerta {id} não encontrado; use get_alerts"
            )));
        };

        let rule = match event.alert_rule_id {
            Some(rule_id) => {
                alert_rules::Entity::find_by_id(rule_id)
                    .one(&ctx.db)
                    .await?
            }
            None => None,
        };
        let rule_json = match &rule {
            Some(rule) => {
                let fired = firings_since(ctx, vec![rule.id], Utc::now() - Duration::hours(24))
                    .await?
                    .get(&rule.id)
                    .copied()
                    .unwrap_or(0);
                json!({
                    "id": rule.id,
                    "name": rule.name,
                    "condition": condition_text(&rule.condition),
                    "severity": rule.severity,
                    "scope": scope_label(ctx, rule).await?,
                    "enabled": rule.enabled,
                    "template": rule.template_key,
                    "duration_s": rule.duration_seconds,
                    "recovery_window_s": rule.recovery_window_seconds,
                    "flap_threshold": rule.flap_threshold,
                    "notification_cooldown_s": rule.notification_cooldown_seconds,
                    "inhibit_when_parent_down": rule.inhibit_when_parent_down,
                    "fired_24h": fired,
                    "noisy": fired >= NOISY_FIRINGS_24H,
                })
            }
            None => json!(null),
        };

        let device = match event.device_id {
            Some(device_id) => devices::Entity::find_by_id(device_id).one(&ctx.db).await?,
            None => None,
        };
        let other_open = match event.device_id {
            Some(device_id) => {
                alert_events::Entity::find()
                    .filter(alert_events::Column::DeviceId.eq(device_id))
                    .filter(alert_events::Column::Id.ne(event.id))
                    .filter(alert_events::Column::Status.is_in(AlertStatus::OPEN))
                    .count(&ctx.db)
                    .await?
            }
            None => 0,
        };
        let open_minutes = (event.resolved_at.unwrap_or_else(|| Utc::now().into())
            - event.started_at)
            .num_minutes();

        Ok(ToolOutput::data(json!({
            "alert": {
                "id": event.id,
                "status": event.status,
                "severity": event.severity,
                "message": event.message,
                "started_at": event.started_at,
                "resolved_at": event.resolved_at,
                "duration_min": open_minutes,
            },
            "rule": rule_json,
            "device": device.map(|device| json!({
                "id": device.id,
                "name": device.name,
                "ip": device.ip_address,
            })),
            "target": describe_target(ctx, event.scope_key.as_deref()).await?,
            "facts": event.data,
            "other_open_alerts_on_device": other_open,
        })))
    }
}

/// Regra pedida pela IA, já resolvida e validada.
struct RulePlan {
    input: AlertRuleInput,
    summary: String,
}

fn window_seconds(args: &ToolArgs, key: &str) -> Option<i32> {
    args.integer(key)
        .map(|value| value.clamp(0, MAX_WINDOW_SECONDS))
        .and_then(|value| i32::try_from(value).ok())
}

async fn plan_rule(ctx: &AppContext, args: &ToolArgs) -> AppResult<Result<RulePlan, ToolOutput>> {
    let Some(name) = args.text("name") else {
        return Ok(Err(ToolOutput::not_found("Informe o nome da regra (name)")));
    };
    let condition = json!({
        "field": args.text("field").unwrap_or_default(),
        "operator": args.text("operator").unwrap_or_default(),
        "value": args.raw("value").cloned().unwrap_or(Value::Null),
    });
    if let Err(error) = rules::check_vocabulary(&condition) {
        return Ok(Err(ToolOutput::not_found(error.to_string())));
    }
    let severity = args.text("severity").unwrap_or_else(|| "warning".into());
    if !SEVERITIES.contains(&severity.as_str()) {
        return Ok(Err(ToolOutput::not_found(format!(
            "Severidade '{severity}' inválida; use {}",
            SEVERITIES.join(", ")
        ))));
    }

    let mut scope = Vec::new();
    let device_id = match args.text("device") {
        Some(identifier) => {
            let Some(device) = find_device(&ctx.db, &identifier).await? else {
                return Ok(Err(ToolOutput::not_found(format!(
                    "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
                ))));
            };
            scope.push(format!("no dispositivo {}", device.name));
            Some(device.id)
        }
        None => None,
    };
    let monitor_id = match args.integer("monitor_id") {
        Some(id) => {
            let Some(monitor) = monitors::Entity::find_by_id(id).one(&ctx.db).await? else {
                return Ok(Err(ToolOutput::not_found(format!(
                    "Monitor {id} não encontrado; use list_monitors"
                ))));
            };
            scope.push(format!("no monitor {}", monitor.name));
            Some(monitor.id)
        }
        None => None,
    };
    if scope.is_empty() {
        scope.push("em todos os dispositivos (regra global)".to_string());
    }

    let duration = window_seconds(args, "duration_seconds");
    let recovery = window_seconds(args, "recovery_window_seconds");
    let cooldown = window_seconds(args, "notification_cooldown_seconds");
    let hold = duration
        .filter(|seconds| *seconds > 0)
        .map(|seconds| format!(" por {seconds} s"))
        .unwrap_or_default();
    let summary = format!(
        "Criar a regra de alerta '{name}' [{severity}]: dispara quando {}{hold}, {}",
        condition_text(&condition),
        scope.join(" e ")
    );

    Ok(Ok(RulePlan {
        input: AlertRuleInput {
            device_id: Some(device_id),
            monitor_id: Some(monitor_id),
            name: Some(name),
            condition: Some(condition),
            severity: Some(severity),
            duration_seconds: duration,
            recovery_window_seconds: recovery,
            notification_cooldown_seconds: cooldown,
            enabled: Some(true),
            ..AlertRuleInput::default()
        },
        summary,
    }))
}

fn preview_or_error(result: Result<String, ToolOutput>) -> String {
    match result {
        Ok(text) => text,
        Err(output) => format!(
            "Ação inválida: {}",
            output.data["error"]
                .as_str()
                .unwrap_or("argumentos incompletos")
        ),
    }
}

pub struct CreateAlertRule;

#[async_trait]
impl AiToolHandler for CreateAlertRule {
    fn name(&self) -> &'static str {
        "create_alert_rule"
    }

    fn description(&self) -> &'static str {
        "Propõe criar uma regra de alerta. Consulte get_alert_rules_guide para os campos e list_alert_rules para não duplicar. O usuário confirma no chat antes de executar."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Nome claro da regra" },
                "field": { "type": "string", "enum": &ALERT_FIELDS[..], "description": "Fato avaliado (condition.field)" },
                "operator": { "type": "string", "enum": OPERATORS },
                "value": { "description": "Valor de referência (número ou texto, ex.: 150 ou \"down\")" },
                "severity": { "type": "string", "enum": SEVERITIES },
                "device": { "type": "string", "description": "Restringe a um dispositivo (nome, IP ou id); ausente = global" },
                "monitor_id": { "type": "integer", "description": "Restringe a um monitor" },
                "duration_seconds": { "type": "integer", "description": "Tempo que a condição precisa durar (0 a 86400)" },
                "recovery_window_seconds": { "type": "integer", "description": "Estabilidade exigida para resolver" },
                "notification_cooldown_seconds": { "type": "integer", "description": "Intervalo mínimo entre notificações" }
            },
            "required": ["name", "field", "operator", "value"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::AlertRules
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let plan = plan_rule(ctx, args).await?;
        Ok(preview_or_error(plan.map(|plan| plan.summary)))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let plan = match plan_rule(ctx, args).await? {
            Ok(plan) => plan,
            Err(output) => return Ok(output),
        };
        let rule = rules::create(ctx, plan.input, args.actor().clone()).await?;
        Ok(ToolOutput::data(json!({
            "done": true,
            "rule_id": rule.id,
            "name": rule.name,
            "condition": condition_text(&rule.condition),
            "severity": rule.severity,
        })))
    }
}

pub struct DeleteAlertRule;

impl DeleteAlertRule {
    async fn load(
        ctx: &AppContext,
        args: &ToolArgs,
    ) -> AppResult<Result<(alert_rules::Model, u64), ToolOutput>> {
        let Some(id) = args.integer("rule_id") else {
            return Ok(Err(ToolOutput::not_found(
                "Informe rule_id (de list_alert_rules ou explain_alert)",
            )));
        };
        match rules::find(ctx, id).await {
            Ok(rule) => {
                let events = rules::event_count(ctx, rule.id).await?;
                Ok(Ok((rule, events)))
            }
            Err(AppError::NotFound(_)) => Ok(Err(ToolOutput::not_found(format!(
                "Regra {id} não encontrada; use list_alert_rules"
            )))),
            Err(error) => Err(error),
        }
    }
}

#[async_trait]
impl AiToolHandler for DeleteAlertRule {
    fn name(&self) -> &'static str {
        "delete_alert_rule"
    }

    fn description(&self) -> &'static str {
        "Propõe excluir uma regra de alerta — o histórico de alertas dela é apagado junto. Para só parar os avisos, sugira desativar a regra em /alerts. O usuário confirma no chat antes de executar."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "rule_id": { "type": "integer", "description": "Id da regra (de list_alert_rules ou explain_alert)" }
            },
            "required": ["rule_id"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::AlertRules
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let loaded = Self::load(ctx, args).await?;
        Ok(preview_or_error(loaded.map(|(rule, events)| {
            let history = match events {
                0 => "Ela ainda não registrou alertas.".to_string(),
                1 => "O alerta registrado por ela também será apagado.".to_string(),
                n => format!("Os {n} alertas registrados por ela também serão apagados."),
            };
            format!(
                "Excluir a regra de alerta #{} '{}' ({}). {history}",
                rule.id,
                rule.name,
                condition_text(&rule.condition)
            )
        })))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let (rule, events) = match Self::load(ctx, args).await? {
            Ok(loaded) => loaded,
            Err(output) => return Ok(output),
        };
        rules::delete(ctx, rule.id, args.actor().clone()).await?;
        Ok(ToolOutput::data(json!({
            "done": true,
            "deleted_rule": rule.name,
            "deleted_alerts": events,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condicao_legivel_distingue_texto_de_numero() {
        assert_eq!(
            condition_text(&json!({ "field": "latencyMs", "operator": "gt", "value": 150 })),
            "latencyMs gt 150"
        );
        assert_eq!(
            condition_text(&json!({ "field": "status", "operator": "eq", "value": "down" })),
            "status eq \"down\""
        );
    }

    #[test]
    fn guia_cobre_o_vocabulario_inteiro_que_a_tela_oferece() {
        // "success" e "type" são fatos internos, fora da tela e do guia.
        for field in ALERT_FIELDS
            .iter()
            .filter(|field| !matches!(**field, "success" | "type"))
        {
            let documented = ALERT_RULES_GUIDE.contains(&format!("`{field}`"));
            let statistical_family = field.contains("Baseline")
                || field.contains("Stddev")
                || field.ends_with("UpperBandPercent");
            assert!(
                documented || statistical_family,
                "campo {field} fora do guia de regras de alerta"
            );
        }
    }

    #[test]
    fn janelas_sao_limitadas_a_um_dia() {
        let args = ToolArgs::from_value(
            json!({ "duration_seconds": 999_999, "recovery_window_seconds": 30 }),
        );
        assert_eq!(window_seconds(&args, "duration_seconds"), Some(86_400));
        assert_eq!(window_seconds(&args, "recovery_window_seconds"), Some(30));
        assert_eq!(window_seconds(&args, "notification_cooldown_seconds"), None);
    }
}
