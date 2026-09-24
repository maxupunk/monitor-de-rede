//! Ações que mudam o sistema. Todas são [`ToolKind::Action`]: a IA só propõe,
//! o usuário confirma no chat, e a execução roda em nome dele — pelos mesmos
//! serviços que a tela usa, com a mesma auditoria.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::EntityTrait;
use serde_json::{json, Value};

use super::{lookup::find_device, AiToolHandler, ToolArgs, ToolKind, ToolOutput};
use crate::{
    dtos::resources::MonitorInput,
    models::{alert_events, devices},
    services::{
        alerts::{actions as alert_actions, contracts::AlertStatus},
        audit::{AuditAction, AuditEntryInput, AuditService, ResourceType},
        maintenance_windows::{self, MaintenanceWindowInput},
        monitoring::creation,
        shared::errors::AppResult,
    },
};

const SILENCE_MIN: i64 = 5;
const SILENCE_MAX: i64 = 24 * 60;
const WINDOW_MIN: i64 = 15;
const WINDOW_MAX: i64 = 7 * 24 * 60;
/// Tipos que a IA pode criar: os que só precisam de um alvo.
const CREATABLE_MONITORS: &[&str] = &["ping", "http", "https", "tcp", "dns"];

/// Marca no registro de auditoria que a ação veio do chat.
pub(super) const VIA_AI: &str = "via Assistente IA";

fn local_time(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).format("%d/%m %H:%M").to_string()
}

pub(super) async fn audit(ctx: &AppContext, args: &ToolArgs, entry: AuditEntryInput) {
    let _ = AuditService::new(&ctx.db)
        .log(args.actor().clone(), entry)
        .await;
}

/// Alerta pedido e uma descrição curta dele para a confirmação.
async fn load_alert(
    ctx: &AppContext,
    args: &ToolArgs,
) -> AppResult<Result<(alert_events::Model, String), ToolOutput>> {
    let Some(id) = args.integer("alert_id") else {
        return Ok(Err(ToolOutput::not_found(
            "Informe alert_id (de get_alerts)",
        )));
    };
    let Some(event) = alert_events::Entity::find_by_id(id).one(&ctx.db).await? else {
        return Ok(Err(ToolOutput::not_found(format!(
            "Alerta {id} não encontrado; use get_alerts"
        ))));
    };
    if event.status == AlertStatus::Resolved.as_str() {
        return Ok(Err(ToolOutput::not_found(format!(
            "Alerta {id} já está resolvido"
        ))));
    }
    let device = match event.device_id {
        Some(device_id) => devices::Entity::find_by_id(device_id)
            .one(&ctx.db)
            .await?
            .map(|device| format!(" — {}", device.name)),
        None => None,
    };
    let label = format!(
        "#{id} [{}] {}{}",
        event.severity,
        event.message.as_deref().unwrap_or("sem mensagem"),
        device.unwrap_or_default()
    );
    Ok(Ok((event, label)))
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

fn alert_schema(extra: Value) -> Value {
    let mut properties = json!({
        "alert_id": { "type": "integer", "description": "Id do alerta (de get_alerts)" }
    });
    if let (Some(base), Some(more)) = (properties.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    json!({ "type": "object", "properties": properties, "required": ["alert_id"] })
}

pub struct AcknowledgeAlert;

#[async_trait]
impl AiToolHandler for AcknowledgeAlert {
    fn name(&self) -> &'static str {
        "acknowledge_alert"
    }

    fn description(&self) -> &'static str {
        "Propõe reconhecer um alerta aberto (o operador está ciente). O usuário confirma no chat antes de executar."
    }

    fn parameters(&self) -> Value {
        alert_schema(json!({}))
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let loaded = load_alert(ctx, args).await?;
        Ok(preview_or_error(
            loaded.map(|(_, label)| format!("Reconhecer o alerta {label}")),
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let (event, label) = match load_alert(ctx, args).await? {
            Ok(loaded) => loaded,
            Err(output) => return Ok(output),
        };
        let event = alert_actions::acknowledge(ctx, event).await?;
        Ok(ToolOutput::data(json!({
            "done": true,
            "alert": label,
            "status": event.status,
        })))
    }
}

pub struct SilenceAlert;

#[async_trait]
impl AiToolHandler for SilenceAlert {
    fn name(&self) -> &'static str {
        "silence_alert"
    }

    fn description(&self) -> &'static str {
        "Propõe silenciar as notificações de um alerta por alguns minutos. O usuário confirma no chat antes de executar."
    }

    fn parameters(&self) -> Value {
        alert_schema(json!({
            "minutes": { "type": "integer", "description": "Duração em minutos (5 a 1440, padrão 60)" }
        }))
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let minutes = args.integer_in("minutes", 60, SILENCE_MIN, SILENCE_MAX);
        let loaded = load_alert(ctx, args).await?;
        Ok(preview_or_error(loaded.map(|(_, label)| {
            format!("Silenciar o alerta {label} por {minutes} min")
        })))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let minutes = args.integer_in("minutes", 60, SILENCE_MIN, SILENCE_MAX);
        let (event, label) = match load_alert(ctx, args).await? {
            Ok(loaded) => loaded,
            Err(output) => return Ok(output),
        };
        let event = alert_actions::silence(ctx, event, minutes).await?;
        Ok(ToolOutput::data(json!({
            "done": true,
            "alert": label,
            "status": event.status,
            "minutes": minutes,
        })))
    }
}

/// Janela pedida, calculada a partir de agora.
struct WindowPlan {
    device: devices::Model,
    name: String,
    description: Option<String>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

async fn plan_window(
    ctx: &AppContext,
    args: &ToolArgs,
    now: DateTime<Utc>,
) -> AppResult<Result<WindowPlan, ToolOutput>> {
    let Some(identifier) = args.text("device") else {
        return Ok(Err(ToolOutput::not_found("Informe o dispositivo")));
    };
    let Some(device) = find_device(&ctx.db, &identifier).await? else {
        return Ok(Err(ToolOutput::not_found(format!(
            "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
        ))));
    };
    let duration = args.integer_in("duration_minutes", 60, WINDOW_MIN, WINDOW_MAX);
    let starts_in = args.integer_in("starts_in_minutes", 0, 0, WINDOW_MAX);
    let starts_at = now + Duration::minutes(starts_in);
    let name = args
        .text("name")
        .unwrap_or_else(|| format!("Manutenção {}", device.name));
    Ok(Ok(WindowPlan {
        name,
        description: args.text("reason"),
        starts_at,
        ends_at: starts_at + Duration::minutes(duration),
        device,
    }))
}

pub struct CreateMaintenanceWindow;

#[async_trait]
impl AiToolHandler for CreateMaintenanceWindow {
    fn name(&self) -> &'static str {
        "create_maintenance_window"
    }

    fn description(&self) -> &'static str {
        "Propõe uma janela de manutenção para um dispositivo (alertas dele não notificam no período). O usuário confirma no chat antes de executar."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Nome, IP ou id do dispositivo" },
                "duration_minutes": { "type": "integer", "description": "Duração (15 a 10080, padrão 60)" },
                "starts_in_minutes": { "type": "integer", "description": "Começa daqui a N minutos (padrão 0 = agora)" },
                "name": { "type": "string", "description": "Nome da janela (opcional)" },
                "reason": { "type": "string", "description": "Motivo (opcional)" }
            },
            "required": ["device"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let plan = plan_window(ctx, args, Utc::now()).await?;
        Ok(preview_or_error(plan.map(|plan| {
            format!(
                "Criar a janela de manutenção '{}' para {} de {} até {}",
                plan.name,
                plan.device.name,
                local_time(plan.starts_at),
                local_time(plan.ends_at)
            )
        })))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let plan = match plan_window(ctx, args, Utc::now()).await? {
            Ok(plan) => plan,
            Err(output) => return Ok(output),
        };
        let row = maintenance_windows::create(
            &ctx.db,
            MaintenanceWindowInput {
                site_id: None,
                device_id: Some(plan.device.id),
                name: plan.name,
                description: plan.description,
                starts_at: plan.starts_at,
                ends_at: plan.ends_at,
            },
            args.actor().user_id,
        )
        .await?;
        maintenance_windows::publish_updated(ctx).await;
        audit(
            ctx,
            args,
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::MaintenanceWindow,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!(
                    "Janela de manutenção '{}' criada ({VIA_AI})",
                    row.name
                )),
                changes: None,
            },
        )
        .await;
        Ok(ToolOutput::data(json!({
            "done": true,
            "window_id": row.id,
            "name": row.name,
            "device": plan.device.name,
            "starts_at": row.starts_at,
            "ends_at": row.ends_at,
        })))
    }
}

/// Monitor pedido, já com tipo, alvo e nome resolvidos.
struct MonitorPlan {
    device: devices::Model,
    kind: String,
    target: String,
    port: Option<i64>,
    name: String,
}

async fn plan_monitor(
    ctx: &AppContext,
    args: &ToolArgs,
) -> AppResult<Result<MonitorPlan, ToolOutput>> {
    let Some(identifier) = args.text("device") else {
        return Ok(Err(ToolOutput::not_found("Informe o dispositivo")));
    };
    let Some(device) = find_device(&ctx.db, &identifier).await? else {
        return Ok(Err(ToolOutput::not_found(format!(
            "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
        ))));
    };
    let kind = args
        .text("type")
        .map_or_else(|| "ping".to_string(), |kind| kind.to_lowercase());
    if !CREATABLE_MONITORS.contains(&kind.as_str()) {
        return Ok(Err(ToolOutput::not_found(format!(
            "Tipo '{kind}' não pode ser criado pelo chat; use {}",
            CREATABLE_MONITORS.join(", ")
        ))));
    }
    let Some(target) = args.text("target").or_else(|| device.ip_address.clone()) else {
        return Ok(Err(ToolOutput::not_found(format!(
            "'{}' não tem IP; informe target",
            device.name
        ))));
    };
    let name = args
        .text("name")
        .unwrap_or_else(|| format!("{} {}", kind.to_uppercase(), device.name));
    Ok(Ok(MonitorPlan {
        port: args.integer("port"),
        device,
        kind,
        target,
        name,
    }))
}

pub struct CreateMonitor;

#[async_trait]
impl AiToolHandler for CreateMonitor {
    fn name(&self) -> &'static str {
        "create_monitor"
    }

    fn description(&self) -> &'static str {
        "Propõe criar um monitor (ping, http, https, tcp ou dns) para um dispositivo. O usuário confirma no chat antes de executar."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Nome, IP ou id do dispositivo" },
                "type": { "type": "string", "enum": CREATABLE_MONITORS },
                "target": { "type": "string", "description": "Alvo (padrão: IP do dispositivo; URL para http/https; domínio para dns)" },
                "port": { "type": "integer", "description": "Porta (para tcp)" },
                "name": { "type": "string", "description": "Nome do monitor (opcional)" }
            },
            "required": ["device"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let plan = plan_monitor(ctx, args).await?;
        Ok(preview_or_error(plan.map(|plan| {
            let port = plan.port.map(|port| format!(":{port}")).unwrap_or_default();
            format!(
                "Criar o monitor {} '{}' em {} apontando para {}{port}",
                plan.kind, plan.name, plan.device.name, plan.target
            )
        })))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let plan = match plan_monitor(ctx, args).await? {
            Ok(plan) => plan,
            Err(output) => return Ok(output),
        };
        let row = creation::create(
            &ctx.db,
            MonitorInput {
                device_id: Some(plan.device.id),
                monitor_type: Some(plan.kind),
                name: Some(plan.name),
                target: Some(plan.target),
                port: plan.port,
                ..MonitorInput::default()
            },
        )
        .await?;
        audit(
            ctx,
            args,
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::Monitor,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!(
                    "Monitor '{}' ({}) criado ({VIA_AI})",
                    row.name, row.r#type
                )),
                changes: None,
            },
        )
        .await;
        Ok(ToolOutput::data(json!({
            "done": true,
            "monitor_id": row.id,
            "name": row.name,
            "type": row.r#type,
            "interval_s": row.interval_seconds,
        })))
    }
}
