//! Cadastro das tarefas agendadas: catálogo, conferência, criar, editar,
//! pausar, excluir e o histórico de execuções.
//!
//! A tela e a IA passam pelas mesmas funções, com o mesmo [`Author`]: quem
//! salva assina a tarefa e ela roda com o papel dessa pessoa
//! ([`super::runner`]).

use std::{collections::HashMap, str::FromStr};

use chrono::Utc;
use loco_rs::prelude::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};

use super::{
    arguments, events,
    model::{
        SchedulableTool, SchedulableToolKind, ScheduledTask, ScheduledTaskInput,
        ScheduledTaskOrigin, ScheduledTaskPreview, ScheduledTaskPreviewInput, ScheduledTaskRun,
    },
    schedule::Schedule,
    view,
};
use crate::{
    models::{_entities::users, scheduled_task_runs, scheduled_tasks},
    services::{
        ai::harness::tools::{ToolArgs, ToolContract, UnattendedTools},
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        shared::{
            errors::{AppError, AppResult},
            pagination::{paginate, PaginatedResponse},
        },
        users::Role,
    },
};

/// Quantas próximas execuções a conferência mostra.
const UPCOMING: usize = 3;
/// Começo da frase que a ferramenta devolve quando o argumento não serve
/// (`preview_or_error` das ações).
const INVALID_ACTION: &str = "Ação inválida: ";

/// Quem está salvando a tarefa, e por onde.
#[derive(Debug, Clone)]
pub struct Author {
    pub actor: AuditActor,
    pub user_id: i64,
    pub role: Role,
    pub origin: ScheduledTaskOrigin,
}

impl Author {
    /// O autor a partir do ator da requisição (ou do chat).
    ///
    /// # Errors
    ///
    /// `Forbidden` sem usuário identificado, desativado ou só de leitura.
    pub async fn load(
        db: &DatabaseConnection,
        actor: AuditActor,
        origin: ScheduledTaskOrigin,
    ) -> AppResult<Self> {
        let user = match actor.user_id {
            Some(id) => users::Entity::find_by_id(id).one(db).await?,
            None => None,
        };
        let Some(user) = user.filter(|user| user.active) else {
            return Err(AppError::forbidden("Usuário não identificado."));
        };
        let role = Role::from_str(&user.role)?;
        if !role.can_write() {
            return Err(AppError::forbidden(
                "Seu perfil é só de leitura: peça a um operador para agendar tarefas.",
            ));
        }
        Ok(Self {
            actor,
            user_id: user.id,
            role,
            origin,
        })
    }
}

/// O catálogo do que pode ser agendado.
#[must_use]
pub fn catalog() -> Vec<SchedulableTool> {
    UnattendedTools::new()
        .contracts()
        .into_iter()
        .map(|contract| SchedulableTool {
            name: contract.name.to_string(),
            description: contract.description.to_string(),
            group: contract.group.id().to_string(),
            group_label: contract.group.purpose().to_string(),
            kind: SchedulableToolKind::of(contract.kind),
            admin_only: contract.admin_only,
            parameters: contract.parameters,
        })
        .collect()
}

/// Ferramenta, argumentos e agenda já conferidos.
struct Checked {
    contract: ToolContract,
    arguments: Value,
    schedule: Schedule,
}

/// Confere ferramenta, papel, argumentos e agenda, juntando todos os
/// problemas — a tela mostra a lista inteira, não um erro por vez.
async fn check(
    ctx: &AppContext,
    tools: &UnattendedTools,
    input: &ScheduledTaskPreviewInput,
    author: Option<&Author>,
) -> (Result<Checked, Vec<String>>, ScheduledTaskPreview) {
    let mut errors = Vec::new();
    let mut preview = ScheduledTaskPreview {
        valid: false,
        errors: Vec::new(),
        schedule_label: None,
        next_runs: Vec::new(),
        summary: None,
        risk: None,
        risk_reason: None,
    };

    let schedule = match Schedule::parse(&input.cron) {
        Ok(schedule) => {
            preview.schedule_label = Some(schedule.label());
            preview.next_runs = schedule
                .upcoming(Utc::now(), UPCOMING)
                .into_iter()
                .map(view::iso_utc)
                .collect();
            Some(schedule)
        }
        Err(error) => {
            errors.push(error.to_string());
            None
        }
    };

    let tool = input.tool.trim();
    let arguments = arguments::clean(&input.arguments);
    let contract = match tools.contract(tool) {
        Ok(contract) => Some(contract),
        Err(_) if tool.is_empty() => {
            errors.push("Escolha o que a tarefa vai fazer".to_string());
            None
        }
        Err(_) => {
            errors.push(format!(
                "A ferramenta \"{tool}\" não existe ou não pode ser agendada. Agendáveis: {}",
                tools.names().join(", ")
            ));
            None
        }
    };

    if let Some(contract) = &contract {
        if contract.admin_only && author.is_some_and(|author| author.role != Role::Admin) {
            errors.push("Só o administrador pode agendar esta ação".to_string());
        }
        let problems = arguments::problems(&contract.parameters, &arguments);
        let arguments_ok = problems.is_empty();
        errors.extend(problems);
        if arguments_ok {
            let args = ToolArgs::from_value(arguments.clone())
                .with_actor(author.map_or_else(AuditActor::default, |author| author.actor.clone()));
            match tools.preview(ctx, tool, &args).await {
                Ok(Some(text)) => match text.strip_prefix(INVALID_ACTION) {
                    Some(problem) => errors.push(problem.to_string()),
                    None => preview.summary = Some(text),
                },
                Ok(None) => {}
                Err(error) => errors.push(error.to_string()),
            }
            if let Ok(risk) = tools.risk(ctx, tool, &args).await {
                preview.risk = Some(risk.level);
                preview.risk_reason = Some(risk.reason);
            }
        }
    }

    preview.valid = errors.is_empty();
    preview.errors.clone_from(&errors);
    let checked = match (contract, schedule) {
        (Some(contract), Some(schedule)) if errors.is_empty() => Ok(Checked {
            contract,
            arguments,
            schedule,
        }),
        _ => Err(errors),
    };
    (checked, preview)
}

/// A conferência que a tela mostra enquanto o usuário monta a tarefa.
///
/// # Errors
///
/// Erro do banco.
pub async fn preview(
    ctx: &AppContext,
    input: &ScheduledTaskPreviewInput,
    author: Option<&Author>,
) -> AppResult<ScheduledTaskPreview> {
    Ok(check(ctx, &UnattendedTools::new(), input, author).await.1)
}

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::validation(
            "Dê um nome à tarefa (até 120 caracteres)",
        ));
    }
    Ok(name.to_string())
}

fn description_of(input: &ScheduledTaskInput) -> Option<String> {
    input
        .description
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

async fn validate(
    ctx: &AppContext,
    input: &ScheduledTaskInput,
    author: &Author,
) -> AppResult<(String, Checked)> {
    let name = validate_name(&input.name)?;
    let probe = ScheduledTaskPreviewInput {
        tool: input.tool.clone(),
        arguments: input.arguments.clone(),
        cron: input.cron.clone(),
    };
    let (checked, _) = check(ctx, &UnattendedTools::new(), &probe, Some(author)).await;
    let checked = checked.map_err(|errors| AppError::validation(errors.join("; ")))?;
    Ok((name, checked))
}

/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn find(db: &DatabaseConnection, id: i64) -> AppResult<scheduled_tasks::Model> {
    scheduled_tasks::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Tarefa agendada não encontrada"))
}

async fn owner_names(
    db: &DatabaseConnection,
    rows: &[scheduled_tasks::Model],
) -> AppResult<HashMap<i64, String>> {
    let ids: Vec<i64> = rows.iter().filter_map(|row| row.user_id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(users::Entity::find()
        .filter(users::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|user| (user.id, user.name))
        .collect())
}

/// Todas as tarefas, por nome.
///
/// # Errors
///
/// Erro do banco.
pub async fn list(db: &DatabaseConnection) -> AppResult<Vec<ScheduledTask>> {
    let rows = scheduled_tasks::Entity::find()
        .order_by_asc(scheduled_tasks::Column::Name)
        .all(db)
        .await?;
    let names = owner_names(db, &rows).await?;
    Ok(rows
        .iter()
        .map(|row| {
            let owner = row.user_id.and_then(|id| names.get(&id).cloned());
            view::task(row, owner)
        })
        .collect())
}

/// Uma tarefa no contrato da tela.
///
/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn get(db: &DatabaseConnection, id: i64) -> AppResult<ScheduledTask> {
    present(db, &find(db, id).await?).await
}

async fn present(
    db: &DatabaseConnection,
    row: &scheduled_tasks::Model,
) -> AppResult<ScheduledTask> {
    let names = owner_names(db, std::slice::from_ref(row)).await?;
    let owner = row.user_id.and_then(|id| names.get(&id).cloned());
    Ok(view::task(row, owner))
}

fn snapshot(row: &scheduled_tasks::Model) -> Value {
    json!({
        "name": row.name,
        "tool": row.tool,
        "arguments": row.arguments,
        "cron": row.cron,
        "enabled": row.enabled,
    })
}

async fn audit(ctx: &AppContext, author: &Author, entry: AuditEntryInput) {
    if let Err(error) = AuditService::new(&ctx.db)
        .log(author.actor.clone(), entry)
        .await
    {
        tracing::warn!(%error, "falha ao auditar tarefa agendada");
    }
}

fn via(author: &Author) -> &'static str {
    match author.origin {
        ScheduledTaskOrigin::Ai => " (via Assistente IA)",
        ScheduledTaskOrigin::User => "",
    }
}

/// Cria a tarefa, assinada por `author`.
///
/// # Errors
///
/// `Validation` com todos os problemas, `Forbidden` ou erro do banco.
pub async fn create(
    ctx: &AppContext,
    input: &ScheduledTaskInput,
    author: &Author,
) -> AppResult<ScheduledTask> {
    let (name, checked) = validate(ctx, input, author).await?;
    let now = Utc::now();
    let row = scheduled_tasks::ActiveModel {
        name: Set(name),
        description: Set(description_of(input)),
        tool: Set(checked.contract.name.to_string()),
        arguments: Set(checked.arguments),
        cron: Set(checked.schedule.expression().to_string()),
        enabled: Set(input.enabled),
        origin: Set(author.origin.as_str().to_string()),
        next_run_at: Set(input
            .enabled
            .then(|| checked.schedule.next_after(now))
            .flatten()
            .map(Into::into)),
        user_id: Set(Some(author.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    audit(
        ctx,
        author,
        AuditEntryInput {
            action: AuditAction::Create,
            resource_type: ResourceType::ScheduledTask,
            resource_id: Some(row.id),
            resource_label: Some(row.name.clone()),
            description: Some(format!(
                "Agendou \"{}\" ({}) — {}{}",
                row.name,
                row.tool,
                checked.schedule.label(),
                via(author)
            )),
            changes: Some(AuditChanges {
                old: None,
                new: Some(snapshot(&row)),
            }),
        },
    )
    .await;
    events::publish(ctx, row.id, &row.name, None).await;
    present(&ctx.db, &row).await
}

/// Substitui a tarefa. Quem salva passa a assiná-la.
///
/// # Errors
///
/// `NotFound`, `Validation`, `Forbidden` ou erro do banco.
pub async fn update(
    ctx: &AppContext,
    id: i64,
    input: &ScheduledTaskInput,
    author: &Author,
) -> AppResult<ScheduledTask> {
    let current = find(&ctx.db, id).await?;
    let (name, checked) = validate(ctx, input, author).await?;
    let old = snapshot(&current);
    let mut row: scheduled_tasks::ActiveModel = current.into();
    row.name = Set(name);
    row.description = Set(description_of(input));
    row.tool = Set(checked.contract.name.to_string());
    row.arguments = Set(checked.arguments);
    row.cron = Set(checked.schedule.expression().to_string());
    row.enabled = Set(input.enabled);
    row.next_run_at = Set(input
        .enabled
        .then(|| checked.schedule.next_after(Utc::now()))
        .flatten()
        .map(Into::into));
    row.user_id = Set(Some(author.user_id));
    let row = row.update(&ctx.db).await?;
    audit(
        ctx,
        author,
        AuditEntryInput {
            action: AuditAction::Update,
            resource_type: ResourceType::ScheduledTask,
            resource_id: Some(row.id),
            resource_label: Some(row.name.clone()),
            description: Some(format!(
                "Alterou a tarefa agendada \"{}\" — {}{}",
                row.name,
                checked.schedule.label(),
                via(author)
            )),
            changes: Some(AuditChanges {
                old: Some(old),
                new: Some(snapshot(&row)),
            }),
        },
    )
    .await;
    events::publish(ctx, row.id, &row.name, None).await;
    present(&ctx.db, &row).await
}

/// Pausa ou retoma. Retomar recalcula a próxima execução a partir de agora:
/// o que venceu durante a pausa não roda.
///
/// # Errors
///
/// `NotFound`, `Forbidden` ou erro do banco.
pub async fn set_enabled(
    ctx: &AppContext,
    id: i64,
    enabled: bool,
    author: &Author,
) -> AppResult<ScheduledTask> {
    let current = find(&ctx.db, id).await?;
    if enabled {
        // Quem retoma assina: confere de novo se pode rodar a ferramenta.
        let input = ScheduledTaskInput {
            name: current.name.clone(),
            description: current.description.clone(),
            tool: current.tool.clone(),
            arguments: current.arguments.clone(),
            cron: current.cron.clone(),
            enabled,
        };
        return update(ctx, id, &input, author).await;
    }
    let mut row: scheduled_tasks::ActiveModel = current.into();
    row.enabled = Set(false);
    row.next_run_at = Set(None);
    let row = row.update(&ctx.db).await?;
    audit(
        ctx,
        author,
        AuditEntryInput {
            action: AuditAction::Update,
            resource_type: ResourceType::ScheduledTask,
            resource_id: Some(row.id),
            resource_label: Some(row.name.clone()),
            description: Some(format!(
                "Pausou a tarefa agendada \"{}\"{}",
                row.name,
                via(author)
            )),
            changes: None,
        },
    )
    .await;
    events::publish(ctx, row.id, &row.name, None).await;
    present(&ctx.db, &row).await
}

/// Exclui a tarefa e o histórico dela.
///
/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn delete(
    ctx: &AppContext,
    id: i64,
    author: &Author,
) -> AppResult<scheduled_tasks::Model> {
    let row = find(&ctx.db, id).await?;
    // O SQLite só aplica o `CASCADE` com `foreign_keys=ON`: o histórico sai à
    // mão, nos dois bancos.
    scheduled_task_runs::Entity::delete_many()
        .filter(scheduled_task_runs::Column::TaskId.eq(id))
        .exec(&ctx.db)
        .await?;
    scheduled_tasks::Entity::delete_by_id(id)
        .exec(&ctx.db)
        .await?;
    audit(
        ctx,
        author,
        AuditEntryInput {
            action: AuditAction::Delete,
            resource_type: ResourceType::ScheduledTask,
            resource_id: Some(row.id),
            resource_label: Some(row.name.clone()),
            description: Some(format!(
                "Excluiu a tarefa agendada \"{}\"{}",
                row.name,
                via(author)
            )),
            changes: Some(AuditChanges {
                old: Some(snapshot(&row)),
                new: None,
            }),
        },
    )
    .await;
    events::publish(ctx, id, &row.name, None).await;
    Ok(row)
}

/// Histórico de execuções, da mais nova para a mais antiga.
///
/// # Errors
///
/// `NotFound` ou erro do banco.
pub async fn runs(
    db: &DatabaseConnection,
    task_id: i64,
    page: u64,
    limit: u64,
) -> AppResult<PaginatedResponse<ScheduledTaskRun>> {
    find(db, task_id).await?;
    paginate(
        db,
        scheduled_task_runs::Entity::find()
            .filter(scheduled_task_runs::Column::TaskId.eq(task_id))
            .order_by_desc(scheduled_task_runs::Column::Id),
        page,
        limit,
        view::run,
    )
    .await
}
