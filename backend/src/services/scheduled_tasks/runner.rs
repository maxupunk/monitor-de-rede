//! Execução das tarefas agendadas.
//!
//! Um laço no processo do servidor (como o backup automático) confere a
//! cada [`TICK`] o que venceu ([`run_due`]). O processo é o do servidor
//! porque é nele que mora o canal dos agentes: ação em container de um host
//! remoto só sai daqui.
//!
//! - A próxima execução é gravada **antes** de rodar: um restart no meio não
//!   repete a tarefa.
//! - Horário perdido (servidor parado) não roda atrasado — "reiniciar às
//!   03:00" às 09:00 seria pior que não rodar. Fica no histórico como
//!   `skipped`, com o motivo.
//! - Uma execução por tarefa de cada vez ([`RunGuard`]); a seguinte que
//!   vencer no meio é registrada como `skipped`.
//! - A tarefa roda em nome de quem a salvou por último, com o papel que essa
//!   pessoa tem **agora**: perdeu a escrita, a tarefa falha com o motivo.

use std::{str::FromStr, time::Duration};

use chrono::{DateTime, Local, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use super::{
    events,
    model::{ScheduledTaskRun, ScheduledTaskStatus, ScheduledTaskTrigger},
    schedule::Schedule,
    view,
};
use crate::{
    models::{_entities::users, scheduled_task_runs, scheduled_tasks},
    services::{
        ai::harness::tools::{ToolArgs, ToolOutput, UnattendedTools},
        audit::AuditActor,
        shared::{
            errors::{AppError, AppResult},
            run_guard::RunGuard,
        },
        users::Role,
    },
};

/// Escopo do [`RunGuard`] das tarefas.
pub const SCOPE: &str = "scheduled_task";
/// De quanto em quanto tempo o laço confere os vencimentos. Bem abaixo do
/// minuto do cron: a tarefa das 03:00 roda até ~03:00:20.
const TICK: Duration = Duration::from_secs(20);
/// Espera antes da primeira conferência, para não somar ao boot.
const FIRST_TICK_DELAY: Duration = Duration::from_secs(30);
/// Atraso tolerado: passou disso, o horário foi perdido.
const GRACE_MINUTES: i64 = 5;
/// Teto de uma execução.
const RUN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Resultado maior que isso é cortado no histórico.
const RESULT_MAX_BYTES: usize = 32 * 1024;
const MESSAGE_MAX_CHARS: usize = 500;

/// Sobe o laço. Fora do ambiente de teste — lá, quem testa chama
/// [`run_due`] diretamente.
pub fn spawn(ctx: &AppContext) {
    if ctx.environment == loco_rs::environment::Environment::Test {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(FIRST_TICK_DELAY).await;
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Err(error) = run_due(&ctx, Utc::now()).await {
                tracing::warn!(%error, "ciclo das tarefas agendadas falhou");
            }
        }
    });
}

/// O desfecho de uma execução.
struct Outcome {
    status: ScheduledTaskStatus,
    message: String,
    result: Option<Value>,
}

impl Outcome {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            status: ScheduledTaskStatus::Failed,
            message: message.into(),
            result: None,
        }
    }

    /// Erro de negócio da ferramenta (`{"error": ...}`) é falha; o resto,
    /// sucesso com o resumo que ela der.
    fn of(output: ToolOutput) -> Self {
        let data = output.data;
        if let Some(error) = data["error"].as_str() {
            return Self {
                status: ScheduledTaskStatus::Failed,
                message: error.to_string(),
                result: Some(data),
            };
        }
        let message = ["message", "summary"]
            .iter()
            .find_map(|key| data[*key].as_str())
            .map_or_else(|| "Concluída".to_string(), str::to_string);
        Self {
            status: ScheduledTaskStatus::Success,
            message,
            result: Some(data),
        }
    }
}

fn clip(text: &str) -> String {
    if text.chars().count() <= MESSAGE_MAX_CHARS {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(MESSAGE_MAX_CHARS).collect();
    clipped.push('…');
    clipped
}

/// O resultado como fica no histórico: inteiro até [`RESULT_MAX_BYTES`].
fn stored_result(result: Option<Value>) -> Option<Value> {
    let result = result?;
    let text = result.to_string();
    if text.len() <= RESULT_MAX_BYTES {
        return Some(result);
    }
    let preview: String = text.chars().take(RESULT_MAX_BYTES).collect();
    Some(json!({ "truncated": true, "bytes": text.len(), "preview": preview }))
}

fn local(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).format("%d/%m %H:%M").to_string()
}

/// Em nome de quem a tarefa roda, se essa pessoa ainda pode.
async fn owner(ctx: &AppContext, task: &scheduled_tasks::Model) -> Result<AuditActor, String> {
    let orphan = || {
        "Ninguém assina esta tarefa (o usuário foi excluído): edite e salve para assumi-la"
            .to_string()
    };
    let Some(user_id) = task.user_id else {
        return Err(orphan());
    };
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(orphan)?;
    let role = Role::from_str(&user.role).map_err(|error| error.to_string())?;
    if !user.active || !role.can_write() {
        return Err(format!(
            "{} não pode mais rodar ações (usuário inativo ou só de leitura): edite e salve a tarefa para assumi-la",
            user.name
        ));
    }
    let admin_only = UnattendedTools::new()
        .contract(&task.tool)
        .is_ok_and(|contract| contract.admin_only);
    if admin_only && role != Role::Admin {
        return Err(format!(
            "Só o administrador roda esta ação, e {} não é mais administrador",
            user.name
        ));
    }
    Ok(AuditActor {
        user_id: Some(user.id),
        ..AuditActor::default()
    })
}

async fn execute(ctx: &AppContext, task: &scheduled_tasks::Model) -> Outcome {
    let actor = match owner(ctx, task).await {
        Ok(actor) => actor,
        Err(reason) => return Outcome::failed(reason),
    };
    let args = ToolArgs::from_value(task.arguments.clone()).with_actor(actor);
    let tools = UnattendedTools::new();
    match tokio::time::timeout(RUN_TIMEOUT, tools.execute(ctx, &task.tool, &args)).await {
        Err(_) => Outcome::failed(format!(
            "Passou de {} minutos sem terminar e foi interrompida",
            RUN_TIMEOUT.as_secs() / 60
        )),
        Ok(Err(error)) => Outcome::failed(error.to_string()),
        Ok(Ok(output)) => Outcome::of(output),
    }
}

/// Grava o desfecho na execução e na tarefa, e avisa as telas.
async fn finish(
    ctx: &AppContext,
    task: &scheduled_tasks::Model,
    run: scheduled_task_runs::Model,
    outcome: Outcome,
    started: std::time::Instant,
) -> AppResult<ScheduledTaskRun> {
    let message = clip(&outcome.message);
    let mut active: scheduled_task_runs::ActiveModel = run.into();
    active.status = Set(outcome.status.as_str().to_string());
    active.finished_at = Set(Some(Utc::now().into()));
    active.duration_ms = Set(Some(
        i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
    ));
    active.message = Set(Some(message.clone()));
    active.result = Set(stored_result(outcome.result));
    let run = active.update(&ctx.db).await?;

    // A tarefa pode ter sido excluída enquanto rodava.
    if let Some(current) = scheduled_tasks::Entity::find_by_id(task.id)
        .one(&ctx.db)
        .await?
    {
        let mut active: scheduled_tasks::ActiveModel = current.into();
        active.last_status = Set(Some(outcome.status.as_str().to_string()));
        active.last_message = Set(Some(message));
        active.update(&ctx.db).await?;
    }
    if outcome.status == ScheduledTaskStatus::Failed {
        tracing::warn!(task_id = task.id, tool = %task.tool, message = ?run.message, "tarefa agendada falhou");
    }
    events::publish(ctx, task.id, &task.name, Some(outcome.status)).await;
    Ok(view::run(run))
}

/// Começa uma execução agora e devolve o id dela e o fim, que roda à parte.
///
/// # Errors
///
/// `Conflict` quando a tarefa já está rodando, ou erro do banco.
pub async fn start(
    ctx: &AppContext,
    task: scheduled_tasks::Model,
    trigger: ScheduledTaskTrigger,
    requested_by: Option<i64>,
) -> AppResult<(i64, JoinHandle<AppResult<ScheduledTaskRun>>)> {
    let guard = RunGuard::acquire(
        SCOPE,
        task.id,
        "Esta tarefa já está rodando; espere terminar",
    )?;
    let now = Utc::now();
    let run = scheduled_task_runs::ActiveModel {
        task_id: Set(task.id),
        trigger: Set(trigger.as_str().to_string()),
        status: Set(ScheduledTaskStatus::Running.as_str().to_string()),
        started_at: Set(now.into()),
        user_id: Set(requested_by.or(task.user_id)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    let mut active: scheduled_tasks::ActiveModel = task.clone().into();
    active.last_run_at = Set(Some(now.into()));
    active.last_status = Set(Some(ScheduledTaskStatus::Running.as_str().to_string()));
    active.last_message = Set(None);
    active.update(&ctx.db).await?;
    events::publish(ctx, task.id, &task.name, Some(ScheduledTaskStatus::Running)).await;

    let run_id = run.id;
    let ctx = ctx.clone();
    let handle = tokio::spawn(async move {
        let _guard = guard;
        let started = std::time::Instant::now();
        let outcome = execute(&ctx, &task).await;
        finish(&ctx, &task, run, outcome, started).await
    });
    Ok((run_id, handle))
}

/// Registra um horário que não rodou, com o motivo.
async fn skip(
    ctx: &AppContext,
    task: &scheduled_tasks::Model,
    reason: String,
    overwrite_last: bool,
) -> AppResult<()> {
    let now = Utc::now();
    scheduled_task_runs::ActiveModel {
        task_id: Set(task.id),
        trigger: Set(ScheduledTaskTrigger::Schedule.as_str().to_string()),
        status: Set(ScheduledTaskStatus::Skipped.as_str().to_string()),
        started_at: Set(now.into()),
        finished_at: Set(Some(now.into())),
        duration_ms: Set(Some(0)),
        message: Set(Some(reason.clone())),
        user_id: Set(task.user_id),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    if overwrite_last {
        let mut active: scheduled_tasks::ActiveModel = task.clone().into();
        active.last_status = Set(Some(ScheduledTaskStatus::Skipped.as_str().to_string()));
        active.last_message = Set(Some(reason));
        active.update(&ctx.db).await?;
    }
    Ok(())
}

/// Dispara as tarefas vencidas. Devolve as execuções que começaram — o laço
/// as deixa correr; os testes esperam por elas.
///
/// # Errors
///
/// Erro do banco ao listar ou ao gravar a próxima execução.
pub async fn run_due(
    ctx: &AppContext,
    now: DateTime<Utc>,
) -> AppResult<Vec<JoinHandle<AppResult<ScheduledTaskRun>>>> {
    let due = scheduled_tasks::Entity::find()
        .filter(scheduled_tasks::Column::Enabled.eq(true))
        .filter(scheduled_tasks::Column::NextRunAt.lte(now))
        .all(&ctx.db)
        .await?;
    let mut started = Vec::new();
    for task in due {
        let Some(scheduled_for) = task.next_run_at.map(|at| at.with_timezone(&Utc)) else {
            continue;
        };
        // A próxima vai para o banco antes de rodar.
        let next = Schedule::parse(&task.cron)
            .ok()
            .and_then(|schedule| schedule.next_after(now));
        let mut active: scheduled_tasks::ActiveModel = task.clone().into();
        active.next_run_at = Set(next.map(Into::into));
        let task = active.update(&ctx.db).await?;

        if now - scheduled_for > chrono::Duration::minutes(GRACE_MINUTES) {
            skip(
                ctx,
                &task,
                format!(
                    "Não rodou às {}: o servidor estava fora do ar nesse horário",
                    local(scheduled_for)
                ),
                true,
            )
            .await?;
            events::publish(ctx, task.id, &task.name, Some(ScheduledTaskStatus::Skipped)).await;
            continue;
        }
        match start(ctx, task.clone(), ScheduledTaskTrigger::Schedule, None).await {
            Ok((_, handle)) => started.push(handle),
            Err(AppError::Conflict(_)) => {
                skip(
                    ctx,
                    &task,
                    format!(
                        "Não rodou às {}: a execução anterior ainda não tinha terminado",
                        local(scheduled_for)
                    ),
                    false,
                )
                .await?;
                events::publish(ctx, task.id, &task.name, None).await;
            }
            Err(error) => {
                tracing::warn!(%error, task_id = task.id, "falha ao disparar tarefa agendada");
            }
        }
    }
    Ok(started)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erro_da_ferramenta_vira_falha_com_a_mensagem() {
        let falha = Outcome::of(ToolOutput::not_found("Alerta 9 não encontrado"));
        assert_eq!(falha.status, ScheduledTaskStatus::Failed);
        assert_eq!(falha.message, "Alerta 9 não encontrado");

        let ok = Outcome::of(ToolOutput::data(json!({ "done": true })));
        assert_eq!(ok.status, ScheduledTaskStatus::Success);
        assert_eq!(ok.message, "Concluída");
    }

    #[test]
    fn resultado_enorme_e_cortado() {
        let grande = json!({ "rows": "x".repeat(RESULT_MAX_BYTES * 2) });
        let guardado = stored_result(Some(grande)).unwrap();
        assert_eq!(guardado["truncated"], true);
        assert!(guardado["preview"].as_str().unwrap().len() <= RESULT_MAX_BYTES);

        let pequeno = json!({ "ok": 1 });
        assert_eq!(stored_result(Some(pequeno.clone())), Some(pequeno));
    }

    #[test]
    fn mensagem_longa_ganha_reticencias() {
        let longa = "a".repeat(MESSAGE_MAX_CHARS + 10);
        assert_eq!(clip(&longa).chars().count(), MESSAGE_MAX_CHARS + 1);
        assert_eq!(clip("curta"), "curta");
    }
}
