//! Tarefas agendadas pelo chat: o mesmo cadastro da tela
//! (`services::scheduled_tasks`), com a confirmação do usuário.
//!
//! Criar, alterar, rodar e excluir são [`ToolKind::Action`]. Agendar uma
//! ação é arriscado por natureza — ela vai rodar sozinha, sem perguntar a
//! cada vez —, então o risco acompanha o que a tarefa faz: consulta é baixo,
//! teste ativo é médio e ação ou container é alto (sempre pergunta).

use async_trait::async_trait;
use chrono::{DateTime, Local};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use super::{
    AiToolHandler, RiskAssessment, ToolArgs, ToolGroup, ToolKind, ToolOutput, UnattendedTools,
};
use crate::{
    models::{scheduled_task_runs, scheduled_tasks},
    services::{
        scheduled_tasks::{
            model::{
                ScheduledTaskInput, ScheduledTaskOrigin, ScheduledTaskPreview,
                ScheduledTaskPreviewInput, ScheduledTaskTrigger,
            },
            runner, schedule, service, view, Author,
        },
        shared::errors::{AppError, AppResult},
    },
};

/// Como a IA escreve a agenda: o formato e três exemplos bastam.
const CRON_HELP: &str = "Cron de 5 campos na hora local: minuto hora dia mês dia-da-semana. Ex.: '0 3 * * *' todo dia 03:00; '*/15 * * * *' a cada 15 min; '0 8 * * 1-5' seg–sex 08:00";
const RUNS_SHOWN: u64 = 5;

fn local(at: &str) -> String {
    DateTime::parse_from_rfc3339(at).map_or_else(
        |_| at.to_string(),
        |at| at.with_timezone(&Local).format("%d/%m %H:%M").to_string(),
    )
}

/// O risco de deixar `tool` rodando sozinha.
async fn unattended_risk(ctx: &AppContext, tool: &str, arguments: &Value) -> RiskAssessment {
    let tools = UnattendedTools::new();
    let Ok(contract) = tools.contract(tool) else {
        return RiskAssessment::medium("Ferramenta desconhecida");
    };
    match contract.kind {
        ToolKind::Passive => RiskAssessment::low("Só uma consulta, repetida no horário"),
        ToolKind::Active => {
            RiskAssessment::medium("Um teste ativo na rede, repetido sozinho no horário")
        }
        _ => {
            let args = ToolArgs::from_value(arguments.clone());
            let reason = tools
                .risk(ctx, tool, &args)
                .await
                .map_or_else(|_| String::new(), |risk| format!(" ({})", risk.reason));
            RiskAssessment::high(format!(
                "A ação vai rodar sozinha, sem confirmação a cada vez{reason}"
            ))
        }
    }
}

/// O autor do chat, se ele pode agendar; senão a recusa como dado.
async fn chat_author(ctx: &AppContext, args: &ToolArgs) -> Result<Author, ToolOutput> {
    Author::load(&ctx.db, args.actor().clone(), ScheduledTaskOrigin::Ai)
        .await
        .map_err(|error| ToolOutput::not_found(error.to_string()))
}

/// Erro de validação volta à IA como dado, para ela corrigir.
fn as_output<T>(result: AppResult<T>) -> AppResult<Result<T, ToolOutput>> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(
            error @ (AppError::Validation(_)
            | AppError::NotFound(_)
            | AppError::Forbidden(_)
            | AppError::Conflict(_)
            | AppError::BusinessRule(_)),
        ) => Ok(Err(ToolOutput::not_found(error.to_string()))),
        Err(error) => Err(error),
    }
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

/// A frase da agenda conferida: "todo dia às 03:00 (próxima: 24/10 03:00)".
fn when(preview: &ScheduledTaskPreview) -> String {
    let label = preview.schedule_label.clone().unwrap_or_default();
    match preview.next_runs.first() {
        Some(next) => format!("{label} (próxima: {})", local(next)),
        None => label,
    }
}

async fn load_task(
    ctx: &AppContext,
    args: &ToolArgs,
) -> AppResult<Result<scheduled_tasks::Model, ToolOutput>> {
    let Some(id) = args.integer("task_id") else {
        return Ok(Err(ToolOutput::not_found(
            "Informe task_id (de list_scheduled_tasks)",
        )));
    };
    match service::find(&ctx.db, id).await {
        Ok(task) => Ok(Ok(task)),
        Err(AppError::NotFound(_)) => Ok(Err(ToolOutput::not_found(format!(
            "Tarefa {id} não encontrada; use list_scheduled_tasks"
        )))),
        Err(error) => Err(error),
    }
}

fn task_label(task: &scheduled_tasks::Model) -> String {
    format!(
        "#{} \"{}\" ({}, {})",
        task.id,
        task.name,
        task.tool,
        schedule::describe(&task.cron)
    )
}

pub struct ListScheduledTasks;

#[async_trait]
impl AiToolHandler for ListScheduledTasks {
    fn name(&self) -> &'static str {
        "list_scheduled_tasks"
    }

    fn description(&self) -> &'static str {
        "Tarefas agendadas: o que roda, quando, próxima execução e último resultado. Com task_id, também as últimas execuções dela."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "integer", "description": "Detalha uma tarefa com as últimas execuções" },
                "search": { "type": "string", "description": "Trecho do nome ou da ferramenta" }
            }
        })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Schedules
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let search = args.text("search").map(|text| text.to_lowercase());
        let tasks: Vec<Value> = service::list(&ctx.db)
            .await?
            .into_iter()
            .filter(|task| args.integer("task_id").is_none_or(|id| task.id == id))
            .filter(|task| {
                search.as_ref().is_none_or(|search| {
                    task.name.to_lowercase().contains(search) || task.tool.contains(search)
                })
            })
            .map(|task| {
                json!({
                    "id": task.id,
                    "name": task.name,
                    "tool": task.tool,
                    "arguments": task.arguments,
                    "schedule": task.schedule_label,
                    "cron": task.cron,
                    "enabled": task.enabled,
                    "running": task.running,
                    "next_run": task.next_run_at.as_deref().map(local),
                    "last_run": task.last_run_at.as_deref().map(local),
                    "last_status": task.last_status,
                    "last_message": task.last_message,
                    "owner": task.owner_name,
                })
            })
            .collect();
        let mut data = json!({ "total": tasks.len(), "tasks": tasks });
        if let Some(id) = args.integer("task_id") {
            let runs: Vec<Value> = scheduled_task_runs::Entity::find()
                .filter(scheduled_task_runs::Column::TaskId.eq(id))
                .order_by_desc(scheduled_task_runs::Column::Id)
                .limit(RUNS_SHOWN)
                .all(&ctx.db)
                .await?
                .into_iter()
                .map(|run| {
                    let run = view::run(run);
                    json!({
                        "started": local(&run.started_at),
                        "trigger": run.trigger,
                        "status": run.status,
                        "duration_ms": run.duration_ms,
                        "message": run.message,
                    })
                })
                .collect();
            data["recent_runs"] = json!(runs);
        }
        Ok(ToolOutput::data(data))
    }
}

/// A frase da confirmação da tarefa pedida pela IA, ou o que está errado.
async fn plan_task(
    ctx: &AppContext,
    args: &ToolArgs,
    input: &ScheduledTaskInput,
    verb: &str,
) -> AppResult<Result<String, ToolOutput>> {
    let author = Author::load(&ctx.db, args.actor().clone(), ScheduledTaskOrigin::Ai)
        .await
        .ok();
    let probe = ScheduledTaskPreviewInput {
        tool: input.tool.clone(),
        arguments: input.arguments.clone(),
        cron: input.cron.clone(),
    };
    let preview = service::preview(ctx, &probe, author.as_ref()).await?;
    if !preview.valid {
        return Ok(Err(ToolOutput::not_found(preview.errors.join("; "))));
    }
    let what = preview
        .summary
        .clone()
        .unwrap_or_else(|| format!("rodar {}", input.tool));
    Ok(Ok(format!(
        "{verb} \"{}\": {what} — {}",
        input.name.trim(),
        when(&preview)
    )))
}

/// A tarefa pedida pela IA, ou o que falta nela.
fn input_from_args(args: &ToolArgs) -> Result<ScheduledTaskInput, String> {
    let Some(tool) = args.text("tool") else {
        return Err("Informe tool: o nome da ferramenta que a tarefa roda".to_string());
    };
    let Some(cron) = args.text("cron") else {
        return Err(format!("Informe cron. {CRON_HELP}"));
    };
    Ok(ScheduledTaskInput {
        name: args.text("name").unwrap_or_else(|| tool.clone()),
        description: args.text("description"),
        arguments: args.raw("arguments").cloned().unwrap_or_else(|| json!({})),
        tool,
        cron,
        enabled: true,
    })
}

pub struct CreateScheduledTask;

#[async_trait]
impl AiToolHandler for CreateScheduledTask {
    fn name(&self) -> &'static str {
        "create_scheduled_task"
    }

    fn description(&self) -> &'static str {
        "Propõe agendar uma ferramenta para rodar sozinha num horário (o usuário confirma). Vale consulta, teste ativo, ação ou container; acesso a equipamento não. arguments segue o contrato da ferramenta."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Nome curto da tarefa" },
                "tool": { "type": "string", "description": "Nome exato da ferramenta (ex.: ping_host, get_system_summary)" },
                "arguments": { "type": "object", "description": "Argumentos da ferramenta" },
                "cron": { "type": "string", "description": CRON_HELP },
                "description": { "type": "string" }
            },
            "required": ["name", "tool", "cron"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Schedules
    }

    async fn risk(&self, ctx: &AppContext, args: &ToolArgs) -> RiskAssessment {
        let arguments = args.raw("arguments").cloned().unwrap_or_else(|| json!({}));
        unattended_risk(ctx, &args.text("tool").unwrap_or_default(), &arguments).await
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let plan = match input_from_args(args) {
            Ok(input) => plan_task(ctx, args, &input, "Agendar").await?,
            Err(missing) => Err(ToolOutput::not_found(missing)),
        };
        Ok(preview_or_error(plan))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let author = match chat_author(ctx, args).await {
            Ok(author) => author,
            Err(output) => return Ok(output),
        };
        let input = match input_from_args(args) {
            Ok(input) => input,
            Err(missing) => return Ok(ToolOutput::not_found(missing)),
        };
        let task = match as_output(service::create(ctx, &input, &author).await)? {
            Ok(task) => task,
            Err(output) => return Ok(output),
        };
        Ok(ToolOutput::data(json!({
            "done": true,
            "task_id": task.id,
            "name": task.name,
            "schedule": task.schedule_label,
            "next_run": task.next_run_at.as_deref().map(local),
        })))
    }
}

pub struct UpdateScheduledTask;

impl UpdateScheduledTask {
    /// Só `enabled: false`: pausar, sem mexer em mais nada.
    fn pause_only(args: &ToolArgs) -> bool {
        args.raw("enabled").and_then(Value::as_bool) == Some(false)
            && ["name", "cron", "arguments", "tool"]
                .iter()
                .all(|key| args.raw(key).is_none())
    }

    fn merged(task: &scheduled_tasks::Model, args: &ToolArgs) -> ScheduledTaskInput {
        ScheduledTaskInput {
            name: args.text("name").unwrap_or_else(|| task.name.clone()),
            description: task.description.clone(),
            tool: task.tool.clone(),
            arguments: args
                .raw("arguments")
                .cloned()
                .unwrap_or_else(|| task.arguments.clone()),
            cron: args.text("cron").unwrap_or_else(|| task.cron.clone()),
            enabled: args
                .raw("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(task.enabled),
        }
    }
}

#[async_trait]
impl AiToolHandler for UpdateScheduledTask {
    fn name(&self) -> &'static str {
        "update_scheduled_task"
    }

    fn description(&self) -> &'static str {
        "Propõe mudar uma tarefa agendada: horário (cron), argumentos, nome, ou pausar/retomar com enabled. Só o que vier muda."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "integer", "description": "Id (de list_scheduled_tasks)" },
                "name": { "type": "string" },
                "cron": { "type": "string", "description": CRON_HELP },
                "arguments": { "type": "object", "description": "Novos argumentos (substituem os atuais)" },
                "enabled": { "type": "boolean", "description": "false pausa; true retoma" }
            },
            "required": ["task_id"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Schedules
    }

    async fn risk(&self, ctx: &AppContext, args: &ToolArgs) -> RiskAssessment {
        if Self::pause_only(args) {
            return RiskAssessment::low("Só pausa a tarefa; nada roda");
        }
        match load_task(ctx, args).await {
            Ok(Ok(task)) => {
                let input = Self::merged(&task, args);
                unattended_risk(ctx, &input.tool, &input.arguments).await
            }
            _ => RiskAssessment::for_kind(self.kind()),
        }
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let task = match load_task(ctx, args).await? {
            Ok(task) => task,
            Err(output) => return Ok(preview_or_error(Err(output))),
        };
        if Self::pause_only(args) {
            return Ok(format!("Pausar a tarefa agendada {}", task_label(&task)));
        }
        let verb = if !task.enabled && args.raw("enabled").and_then(Value::as_bool) == Some(true) {
            "Retomar"
        } else {
            "Alterar"
        };
        let plan = plan_task(ctx, args, &Self::merged(&task, args), verb).await?;
        Ok(preview_or_error(
            plan.map(|summary| format!("{summary} (tarefa #{})", task.id)),
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let author = match chat_author(ctx, args).await {
            Ok(author) => author,
            Err(output) => return Ok(output),
        };
        let task = match load_task(ctx, args).await? {
            Ok(task) => task,
            Err(output) => return Ok(output),
        };
        let result = if Self::pause_only(args) {
            service::set_enabled(ctx, task.id, false, &author).await
        } else {
            service::update(ctx, task.id, &Self::merged(&task, args), &author).await
        };
        let task = match as_output(result)? {
            Ok(task) => task,
            Err(output) => return Ok(output),
        };
        Ok(ToolOutput::data(json!({
            "done": true,
            "task_id": task.id,
            "name": task.name,
            "enabled": task.enabled,
            "schedule": task.schedule_label,
            "next_run": task.next_run_at.as_deref().map(local),
        })))
    }
}

pub struct RunScheduledTask;

#[async_trait]
impl AiToolHandler for RunScheduledTask {
    fn name(&self) -> &'static str {
        "run_scheduled_task"
    }

    fn description(&self) -> &'static str {
        "Propõe rodar uma tarefa agendada agora, fora do horário, e devolve o resultado."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "integer", "description": "Id (de list_scheduled_tasks)" }
            },
            "required": ["task_id"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Schedules
    }

    async fn risk(&self, ctx: &AppContext, args: &ToolArgs) -> RiskAssessment {
        let Ok(Ok(task)) = load_task(ctx, args).await else {
            return RiskAssessment::for_kind(self.kind());
        };
        let tools = UnattendedTools::new();
        tools
            .risk(
                ctx,
                &task.tool,
                &ToolArgs::from_value(task.arguments.clone()),
            )
            .await
            .unwrap_or_else(|_| RiskAssessment::for_kind(self.kind()))
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let loaded = load_task(ctx, args).await?;
        Ok(preview_or_error(loaded.map(|task| {
            format!("Rodar agora a tarefa agendada {}", task_label(&task))
        })))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let author = match chat_author(ctx, args).await {
            Ok(author) => author,
            Err(output) => return Ok(output),
        };
        let task = match load_task(ctx, args).await? {
            Ok(task) => task,
            Err(output) => return Ok(output),
        };
        let started = runner::start(
            ctx,
            task,
            ScheduledTaskTrigger::Manual,
            Some(author.user_id),
        )
        .await;
        let (_, handle) = match as_output(started)? {
            Ok(started) => started,
            Err(output) => return Ok(output),
        };
        let run = handle
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))??;
        Ok(ToolOutput::data(json!({
            "done": true,
            "status": run.status,
            "message": run.message,
            "duration_ms": run.duration_ms,
            "result": run.result,
        })))
    }
}

pub struct DeleteScheduledTask;

#[async_trait]
impl AiToolHandler for DeleteScheduledTask {
    fn name(&self) -> &'static str {
        "delete_scheduled_task"
    }

    fn description(&self) -> &'static str {
        "Propõe excluir uma tarefa agendada e o histórico dela. Para só parar, prefira update_scheduled_task com enabled=false."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "integer", "description": "Id (de list_scheduled_tasks)" }
            },
            "required": ["task_id"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Schedules
    }

    async fn risk(&self, _ctx: &AppContext, _args: &ToolArgs) -> RiskAssessment {
        RiskAssessment::medium("Apaga a tarefa e o histórico de execuções dela")
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let task = match load_task(ctx, args).await? {
            Ok(task) => task,
            Err(output) => return Ok(preview_or_error(Err(output))),
        };
        let runs = scheduled_task_runs::Entity::find()
            .filter(scheduled_task_runs::Column::TaskId.eq(task.id))
            .count(&ctx.db)
            .await?;
        let history = match runs {
            0 => "Ela ainda não rodou.".to_string(),
            1 => "A execução registrada também será apagada.".to_string(),
            n => format!("As {n} execuções registradas também serão apagadas."),
        };
        Ok(format!(
            "Excluir a tarefa agendada {}. {history}",
            task_label(&task)
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let author = match chat_author(ctx, args).await {
            Ok(author) => author,
            Err(output) => return Ok(output),
        };
        let task = match load_task(ctx, args).await? {
            Ok(task) => task,
            Err(output) => return Ok(output),
        };
        let deleted = match as_output(service::delete(ctx, task.id, &author).await)? {
            Ok(deleted) => deleted,
            Err(output) => return Ok(output),
        };
        Ok(ToolOutput::data(json!({
            "done": true,
            "deleted_task": deleted.name,
        })))
    }
}
