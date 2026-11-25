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
//! - Ferramenta de uma área desligada (ADR 017): o horário vira `skipped`,
//!   com o motivo, e a tarefa volta a rodar quando a área volta.
//! - A tarefa roda em nome de quem a salvou por último, com o papel que essa
//!   pessoa tem **agora**: perdeu a escrita, a tarefa falha com o motivo.

use std::{
    str::FromStr,
    sync::{Arc, LazyLock},
    time::Duration,
};

use chrono::{DateTime, Local, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::{json, Value};
use tokio::{sync::Semaphore, task::JoinHandle};

use super::{
    events,
    model::{ScheduledTaskRun, ScheduledTaskStatus, ScheduledTaskTrigger},
    schedule::Schedule,
    service::Author,
    view,
};
use crate::{
    models::{_entities::users, scheduled_task_runs, scheduled_tasks},
    services::{
        audit::AuditActor,
        shared::{
            errors::{AppError, AppResult},
            run_guard::RunGuard,
            transaction::begin_write,
        },
        users::Role,
    },
};
use netmonitor_core::ai::{
    args::ToolArgs,
    tools::{ToolOutput, UnattendedTools},
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
/// Teto de tarefas agendadas executando em paralelo.
const MAX_CONCURRENT_RUNS: usize = 8;
/// Resultado maior que isso é cortado no histórico.
const RESULT_MAX_BYTES: usize = 32 * 1024;
const MESSAGE_MAX_CHARS: usize = 500;

static RUN_SLOTS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_RUNS)));

/// Sobe o laço. Fora do ambiente de teste — lá, quem testa chama
/// [`run_due`] diretamente.
pub fn spawn(ctx: &AppContext) {
    if ctx.environment == loco_rs::environment::Environment::Test {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(error) = recover_stale_runs(&ctx.db, true).await {
            tracing::warn!(%error, "falha ao reconciliar execuções órfãs de tarefas agendadas");
        }
        tokio::time::sleep(FIRST_TICK_DELAY).await;
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Err(error) = recover_stale_runs(&ctx.db, false).await {
                tracing::warn!(%error, "falha ao reconciliar execuções travadas de tarefas agendadas");
            }
            if let Err(error) = run_due(&ctx, Utc::now()).await {
                tracing::warn!(%error, "ciclo das tarefas agendadas falhou");
            }
        }
    });
}

/// Encerra execuções que ficaram presas em `running` após queda do processo ou
/// falha de gravação no `finish`.
///
/// No boot (`on_startup = true`), qualquer linha `running` sem `RunGuard` ativo
/// neste processo é marcada como falha. No laço periódico, só linhas acima de
/// `RUN_TIMEOUT + 1 min` são reconciliadas.
///
/// **Um processo só.** O agendador mora no processo do servidor (ADR 007) e a
/// execução não guarda a instância dona — não há como distinguir, no boot, a
/// linha de um processo que caiu da de um segundo servidor vivo. A regravação
/// é condicional (`WHERE status = 'running'`) e na mesma transação de escrita:
/// se o dono terminar antes, o `success` dele fica; se terminar depois, o
/// `finish` regrava por cima do `failed` com o desfecho real.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn recover_stale_runs(db: &DatabaseConnection, on_startup: bool) -> AppResult<u64> {
    let running = ScheduledTaskStatus::Running.as_str();
    let failed = ScheduledTaskStatus::Failed.as_str();
    let now = Utc::now();
    let reason = if on_startup {
        "Execução interrompida: o servidor foi reiniciado antes do término"
    } else {
        "Execução interrompida: excedeu o tempo máximo sem finalizar"
    };

    let txn = begin_write(db).await?;
    let mut query =
        scheduled_task_runs::Entity::find().filter(scheduled_task_runs::Column::Status.eq(running));
    if !on_startup {
        let cutoff = now
            - chrono::Duration::seconds(i64::try_from(RUN_TIMEOUT.as_secs()).unwrap_or(600) + 60);
        query = query.filter(scheduled_task_runs::Column::StartedAt.lt(cutoff));
    }
    let stale = query.all(&txn).await?;
    let mut recovered = 0;
    for run in stale {
        if RunGuard::is_running(SCOPE, run.task_id) {
            continue;
        }
        let closed = scheduled_task_runs::Entity::update_many()
            .col_expr(scheduled_task_runs::Column::Status, Expr::value(failed))
            .col_expr(
                scheduled_task_runs::Column::FinishedAt,
                Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(now)),
            )
            .col_expr(scheduled_task_runs::Column::Message, Expr::value(reason))
            .filter(scheduled_task_runs::Column::Id.eq(run.id))
            .filter(scheduled_task_runs::Column::Status.eq(running))
            .exec(&txn)
            .await?
            .rows_affected;
        if closed == 0 {
            continue;
        }
        scheduled_tasks::Entity::update_many()
            .col_expr(scheduled_tasks::Column::LastStatus, Expr::value(failed))
            .col_expr(scheduled_tasks::Column::LastMessage, Expr::value(reason))
            .filter(scheduled_tasks::Column::Id.eq(run.task_id))
            .filter(scheduled_tasks::Column::LastStatus.eq(running))
            .exec(&txn)
            .await?;
        recovered += 1;
    }
    txn.commit().await?;
    if recovered > 0 {
        tracing::warn!(
            recovered,
            on_startup,
            "execuções órfãs de tarefas agendadas encerradas"
        );
    }
    Ok(recovered)
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
    let mut end = RESULT_MAX_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let preview = &text[..end];
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

async fn execute(
    ctx: &AppContext,
    task: &scheduled_tasks::Model,
    manual_actor: Option<AuditActor>,
) -> Outcome {
    let actor = match manual_actor {
        Some(actor) => actor,
        None => match owner(ctx, task).await {
            Ok(actor) => actor,
            Err(reason) => return Outcome::failed(reason),
        },
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
    let txn = begin_write(&ctx.db).await?;
    let mut active: scheduled_task_runs::ActiveModel = run.into();
    active.status = Set(outcome.status.as_str().to_string());
    active.finished_at = Set(Some(Utc::now().into()));
    active.duration_ms = Set(Some(
        i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
    ));
    active.message = Set(Some(message.clone()));
    active.result = Set(stored_result(outcome.result));
    let run = active.update(&txn).await?;

    // A tarefa pode ter sido excluída enquanto rodava.
    if let Some(current) = scheduled_tasks::Entity::find_by_id(task.id)
        .one(&txn)
        .await?
    {
        let mut active: scheduled_tasks::ActiveModel = current.into();
        active.last_status = Set(Some(outcome.status.as_str().to_string()));
        active.last_message = Set(Some(message));
        active.update(&txn).await?;
    }
    txn.commit().await?;
    if outcome.status == ScheduledTaskStatus::Failed {
        tracing::warn!(task_id = task.id, tool = %task.tool, message = ?run.message, "tarefa agendada falhou");
    }
    events::publish(ctx, task.id, &task.name, Some(outcome.status)).await;
    Ok(view::run(run))
}

/// Execução manual disparada pelo usuário: exige papel de administrador para
/// ferramentas `admin_only`, recusa se a área estiver desligada e propaga o
/// ator de quem disparou para a auditoria da ferramenta.
///
/// # Errors
///
/// `Forbidden`, `BusinessRule` (área desligada), `Conflict` ou erro do banco.
pub async fn start_manual(
    ctx: &AppContext,
    task: scheduled_tasks::Model,
    author: &Author,
) -> AppResult<(i64, JoinHandle<AppResult<ScheduledTaskRun>>)> {
    super::service::check_tool_permission(&task.tool, author)?;
    let enabled = netmonitor_core::areas::load(&ctx.db).await?;
    if let Some(area) = super::areas::disabled_area(&enabled, &task.tool) {
        return Err(AppError::business_rule(format!(
            "Não é possível rodar agora: a área {} está desligada (Configurações › Áreas)",
            area.title()
        )));
    }
    start_inner(
        ctx,
        task,
        ScheduledTaskTrigger::Manual,
        Some(author.user_id),
        Some(author.actor.clone()),
    )
    .await
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
    start_inner(ctx, task, trigger, requested_by, None).await
}

async fn start_inner(
    ctx: &AppContext,
    task: scheduled_tasks::Model,
    trigger: ScheduledTaskTrigger,
    requested_by: Option<i64>,
    manual_actor: Option<AuditActor>,
) -> AppResult<(i64, JoinHandle<AppResult<ScheduledTaskRun>>)> {
    let guard = RunGuard::acquire(
        SCOPE,
        task.id,
        "Esta tarefa já está rodando; espere terminar",
    )?;
    let now = Utc::now();
    let txn = begin_write(&ctx.db).await?;
    let run = scheduled_task_runs::ActiveModel {
        task_id: Set(task.id),
        trigger: Set(trigger.as_str().to_string()),
        status: Set(ScheduledTaskStatus::Running.as_str().to_string()),
        started_at: Set(now.into()),
        user_id: Set(requested_by.or(task.user_id)),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    let mut active: scheduled_tasks::ActiveModel = task.clone().into();
    active.last_run_at = Set(Some(now.into()));
    active.last_status = Set(Some(ScheduledTaskStatus::Running.as_str().to_string()));
    active.last_message = Set(None);
    active.update(&txn).await?;
    txn.commit().await?;
    events::publish(ctx, task.id, &task.name, Some(ScheduledTaskStatus::Running)).await;

    let run_id = run.id;
    let ctx = ctx.clone();
    let handle = tokio::spawn(async move {
        let _guard = guard;
        let _permit = Arc::clone(&RUN_SLOTS)
            .acquire_owned()
            .await
            .map_err(|_| AppError::Internal(anyhow::anyhow!("semáforo de tarefas fechado")))?;
        let started = std::time::Instant::now();
        let outcome = execute(&ctx, &task, manual_actor).await;
        let result = finish(&ctx, &task, run, outcome, started).await;
        if let Err(ref error) = result {
            tracing::error!(
                %error,
                task_id = task.id,
                run_id,
                "falha ao gravar o desfecho da tarefa agendada"
            );
        }
        result
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
    let txn = begin_write(&ctx.db).await?;
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
    .insert(&txn)
    .await?;
    if overwrite_last {
        let mut active: scheduled_tasks::ActiveModel = task.clone().into();
        active.last_status = Set(Some(ScheduledTaskStatus::Skipped.as_str().to_string()));
        active.last_message = Set(Some(reason));
        active.update(&txn).await?;
    }
    txn.commit().await?;
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
    if due.is_empty() {
        return Ok(Vec::new());
    }
    // Lido a cada ciclo, sem cache: o laço pode rodar em outro processo.
    let enabled = netmonitor_core::areas::load(&ctx.db).await?;
    let mut started = Vec::new();
    for task in due {
        let Some(scheduled_for) = task.next_run_at.map(|at| at.with_timezone(&Utc)) else {
            continue;
        };
        // A próxima vai para o banco antes de rodar.
        let next = match Schedule::parse(&task.cron) {
            Ok(schedule) => schedule.next_after(now),
            Err(error) => {
                tracing::warn!(
                    %error,
                    task_id = task.id,
                    cron = %task.cron,
                    "expressão cron inválida em tarefa agendada"
                );
                None
            }
        };
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
        if let Some(area) = super::areas::disabled_area(&enabled, &task.tool) {
            skip(
                ctx,
                &task,
                format!(
                    "Não rodou às {}: a área {} está desligada (Configurações › Áreas)",
                    local(scheduled_for),
                    area.title()
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

    async fn banco() -> DatabaseConnection {
        use crate::test_schema::{MigratorTrait, TestMigrator as Migrator};
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

    async fn execucao(
        db: &DatabaseConnection,
        task_id: i64,
        status: ScheduledTaskStatus,
    ) -> scheduled_task_runs::Model {
        scheduled_task_runs::ActiveModel {
            task_id: Set(task_id),
            trigger: Set(ScheduledTaskTrigger::Schedule.as_str().to_string()),
            status: Set(status.as_str().to_string()),
            started_at: Set((Utc::now() - chrono::Duration::hours(2)).into()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("execução")
    }

    #[tokio::test]
    async fn reconciliacao_so_fecha_o_que_ainda_esta_rodando() {
        let db = banco().await;
        let agora: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
        let tarefa = scheduled_tasks::ActiveModel {
            name: Set("Teste".into()),
            tool: Set("list_alerts".into()),
            arguments: Set(json!({})),
            cron: Set("0 3 * * *".into()),
            enabled: Set(true),
            origin: Set("user".into()),
            last_status: Set(Some(ScheduledTaskStatus::Running.as_str().to_string())),
            created_at: Set(agora),
            updated_at: Set(agora),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("tarefa");
        let presa = execucao(&db, tarefa.id, ScheduledTaskStatus::Running).await;
        let concluida = execucao(&db, tarefa.id, ScheduledTaskStatus::Success).await;

        assert_eq!(recover_stale_runs(&db, false).await.unwrap(), 1);

        let reler = |id| scheduled_task_runs::Entity::find_by_id(id).one(&db);
        let presa = reler(presa.id).await.unwrap().unwrap();
        assert_eq!(presa.status, ScheduledTaskStatus::Failed.as_str());
        assert!(presa.finished_at.is_some());
        let concluida = reler(concluida.id).await.unwrap().unwrap();
        assert_eq!(concluida.status, ScheduledTaskStatus::Success.as_str());
        let tarefa = scheduled_tasks::Entity::find_by_id(tarefa.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            tarefa.last_status.as_deref(),
            Some(ScheduledTaskStatus::Failed.as_str())
        );

        // Segunda passada: nada mais a fechar.
        assert_eq!(recover_stale_runs(&db, false).await.unwrap(), 0);
    }

    #[test]
    fn mensagem_longa_ganha_reticencias() {
        let longa = "a".repeat(MESSAGE_MAX_CHARS + 10);
        assert_eq!(clip(&longa).chars().count(), MESSAGE_MAX_CHARS + 1);
        assert_eq!(clip("curta"), "curta");
    }
}
