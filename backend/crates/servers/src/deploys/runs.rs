//! O registro de cada deploy: criar, mudar de estado sem corrida e guardar o
//! log.
//!
//! Mudança de estado é **condicional** (`UPDATE … WHERE status IN (…)`): o
//! despachante, a aprovação e o cancelamento podem disputar o mesmo deploy,
//! e só um vence. Quem perde recebe `false` e não faz nada.

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use netmonitor_core::{
    models::deploy_runs,
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

use super::{DeployStage, RunStatus, RunTrigger};

/// Teto do log gravado: guarda o fim, que é onde está o erro.
pub const MAX_LOG_BYTES: usize = 256 * 1024;
const TRUNCATED_MARK: &str = "… (início do log omitido)\n";

/// Consulta `deploy_runs` sem trafegar a coluna `log` (até 256 KiB por linha).
fn select_without_log() -> sea_orm::Select<deploy_runs::Entity> {
    use deploy_runs::Column as C;
    deploy_runs::Entity::find()
        .select_only()
        .columns([
            C::Id,
            C::ApplicationId,
            C::TargetId,
            C::Trigger,
            C::TriggeredBy,
            C::Reference,
            C::CommitSha,
            C::CommitMessage,
            C::ImageTag,
            C::EnvVersionId,
            C::Status,
            C::Stage,
            C::RollbackOfRunId,
            C::AfterRunId,
            C::BackupId,
            C::ApprovedBy,
            C::ApprovedAt,
            C::ScheduledFor,
            C::Error,
            C::StartedAt,
            C::FinishedAt,
            C::DurationMs,
            C::BuildId,
            C::CreatedAt,
            C::UpdatedAt,
        ])
        .column_as(Expr::cust("CAST(NULL AS TEXT)"), "log")
}

/// Um deploy a criar.
#[derive(Debug, Clone)]
pub struct NewRun {
    pub application_id: i64,
    pub target_id: i64,
    pub trigger: RunTrigger,
    pub triggered_by: Option<String>,
    pub reference: Option<String>,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub image_tag: String,
    pub build_id: Option<i64>,
    pub env_version_id: Option<i64>,
    pub status: RunStatus,
    pub rollback_of_run_id: Option<i64>,
    pub after_run_id: Option<i64>,
    pub approved_by: Option<String>,
    pub scheduled_for: Option<DateTime<Utc>>,
}

/// # Errors
///
/// Banco.
pub async fn insert<C: ConnectionTrait>(db: &C, run: NewRun) -> AppResult<deploy_runs::Model> {
    let approved_at = run.approved_by.as_ref().map(|_| Utc::now().into());
    Ok(deploy_runs::ActiveModel {
        application_id: Set(run.application_id),
        target_id: Set(run.target_id),
        trigger: Set(run.trigger.as_str().to_string()),
        triggered_by: Set(run
            .triggered_by
            .map(|value| value.chars().take(120).collect())),
        reference: Set(run.reference.map(|value| value.chars().take(120).collect())),
        commit_sha: Set(run.commit_sha),
        commit_message: Set(run
            .commit_message
            .map(|message| message.chars().take(2000).collect())),
        image_tag: Set(run.image_tag),
        build_id: Set(run.build_id),
        env_version_id: Set(run.env_version_id),
        status: Set(run.status.as_str().to_string()),
        rollback_of_run_id: Set(run.rollback_of_run_id),
        after_run_id: Set(run.after_run_id),
        approved_by: Set(run.approved_by),
        approved_at: Set(approved_at),
        scheduled_for: Set(run.scheduled_for.map(Into::into)),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// # Errors
///
/// `404`.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_runs::Model> {
    deploy_runs::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Deploy não encontrado"))
}

/// O deploy sem a coluna `log` (para quem só olha o estado).
///
/// # Errors
///
/// `404`.
pub async fn find_without_log<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> AppResult<deploy_runs::Model> {
    select_without_log()
        .filter(deploy_runs::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Deploy não encontrado"))
}

/// Os deploys destes ids, sem a coluna `log`, numa consulta por lote.
///
/// # Errors
///
/// Banco.
pub async fn find_many_without_log<C: ConnectionTrait>(
    db: &C,
    ids: &[i64],
) -> AppResult<HashMap<i64, deploy_runs::Model>> {
    let mut found = HashMap::with_capacity(ids.len());
    for chunk in ids.chunks(500) {
        for run in select_without_log()
            .filter(deploy_runs::Column::Id.is_in(chunk.iter().copied()))
            .all(db)
            .await?
        {
            found.insert(run.id, run);
        }
    }
    Ok(found)
}

/// O estado gravado.
#[must_use]
pub fn status_of(run: &deploy_runs::Model) -> RunStatus {
    RunStatus::parse(&run.status).unwrap_or(RunStatus::Failed)
}

/// Os últimos deploys, mais recentes primeiro; de um alvo ou de todos.
///
/// # Errors
///
/// Banco.
pub async fn recent<C: ConnectionTrait>(
    db: &C,
    target_id: Option<i64>,
    limit: u64,
) -> AppResult<Vec<deploy_runs::Model>> {
    let mut query = select_without_log()
        .order_by_desc(deploy_runs::Column::Id)
        .limit(limit);
    if let Some(target_id) = target_id {
        query = query.filter(deploy_runs::Column::TargetId.eq(target_id));
    }
    Ok(query.all(db).await?)
}

/// Os deploys nestes estados, do mais antigo ao mais novo (a ordem da fila).
///
/// # Errors
///
/// Banco.
pub async fn with_status<C: ConnectionTrait>(
    db: &C,
    statuses: &[RunStatus],
) -> AppResult<Vec<deploy_runs::Model>> {
    Ok(select_without_log()
        .filter(deploy_runs::Column::Status.is_in(statuses.iter().map(|status| status.as_str())))
        .order_by_asc(deploy_runs::Column::Id)
        .all(db)
        .await?)
}

/// Os deploys pendentes de um alvo específico.
///
/// # Errors
///
/// Banco.
pub async fn pending_for_target<C: ConnectionTrait>(
    db: &C,
    target_id: i64,
) -> AppResult<Vec<deploy_runs::Model>> {
    Ok(select_without_log()
        .filter(deploy_runs::Column::TargetId.eq(target_id))
        .filter(
            deploy_runs::Column::Status
                .is_in(RunStatus::PENDING.iter().map(|status| status.as_str())),
        )
        .order_by_asc(deploy_runs::Column::Id)
        .all(db)
        .await?)
}

/// A versão no ar do alvo: o último deploy que terminou bem.
///
/// # Errors
///
/// Banco.
pub async fn live<C: ConnectionTrait>(
    db: &C,
    target_id: i64,
) -> AppResult<Option<deploy_runs::Model>> {
    Ok(select_without_log()
        .filter(deploy_runs::Column::TargetId.eq(target_id))
        .filter(deploy_runs::Column::Status.eq(RunStatus::Succeeded.as_str()))
        .order_by_desc(deploy_runs::Column::Id)
        .one(db)
        .await?)
}

/// A versão no ar de cada alvo em `target_ids`, em lote (2 consultas).
///
/// # Errors
///
/// Banco.
pub async fn live_by_targets<C: ConnectionTrait>(
    db: &C,
    target_ids: &[i64],
) -> AppResult<HashMap<i64, deploy_runs::Model>> {
    if target_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let pairs: Vec<(i64, Option<i64>)> = deploy_runs::Entity::find()
        .select_only()
        .column(deploy_runs::Column::TargetId)
        .column_as(deploy_runs::Column::Id.max(), "max_id")
        .filter(deploy_runs::Column::TargetId.is_in(target_ids.iter().copied()))
        .filter(deploy_runs::Column::Status.eq(RunStatus::Succeeded.as_str()))
        .group_by(deploy_runs::Column::TargetId)
        .into_tuple::<(i64, Option<i64>)>()
        .all(db)
        .await?;
    let ids: Vec<i64> = pairs.into_iter().filter_map(|(_, id)| id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(select_without_log()
        .filter(deploy_runs::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|run| (run.target_id, run))
        .collect())
}

/// O alvo tem um deploy rodando?
///
/// # Errors
///
/// Banco.
pub async fn running_on<C: ConnectionTrait>(db: &C, target_id: i64) -> AppResult<bool> {
    Ok(deploy_runs::Entity::find()
        .select_only()
        .column(deploy_runs::Column::Id)
        .filter(deploy_runs::Column::TargetId.eq(target_id))
        .filter(deploy_runs::Column::Status.eq(RunStatus::Running.as_str()))
        .into_tuple::<i64>()
        .one(db)
        .await?
        .is_some())
}

/// O que muda junto com o estado.
#[derive(Debug, Default)]
pub struct Change {
    pub stage: Option<Option<DeployStage>>,
    pub error: Option<Option<String>>,
    pub approved_by: Option<String>,
    pub scheduled_for: Option<Option<DateTime<Utc>>>,
    pub started: bool,
    pub finished: bool,
}

/// Leva o deploy a `to` se ele ainda está num dos estados `from`.
///
/// Quando `change.finished` está marcado, executa a atualização de estado e a
/// gravação de `finished_at`/`duration_ms` na mesma transação de escrita.
///
/// # Errors
///
/// Banco. `Ok(false)` quando outro caminho mudou o estado antes.
pub async fn transition(
    db: &DatabaseConnection,
    id: i64,
    from: &[RunStatus],
    to: RunStatus,
    change: Change,
) -> AppResult<bool> {
    if !change.finished {
        return transition_in(db, id, from, to, change).await;
    }
    let txn = begin_write(db).await?;
    let changed = transition_in(&txn, id, from, to, change).await?;
    txn.commit().await?;
    Ok(changed)
}

/// Variante de [`transition`] para uso dentro de uma transação já aberta.
///
/// # Errors
///
/// Banco. `Ok(false)` quando outro caminho mudou o estado antes.
pub async fn transition_in<C: ConnectionTrait>(
    db: &C,
    id: i64,
    from: &[RunStatus],
    to: RunStatus,
    change: Change,
) -> AppResult<bool> {
    let now = Utc::now();
    let mut update = deploy_runs::Entity::update_many()
        .col_expr(deploy_runs::Column::Status, Expr::value(to.as_str()))
        .col_expr(deploy_runs::Column::UpdatedAt, Expr::value(now))
        .filter(deploy_runs::Column::Id.eq(id))
        .filter(deploy_runs::Column::Status.is_in(from.iter().map(|status| status.as_str())));
    if let Some(stage) = change.stage {
        update = update.col_expr(
            deploy_runs::Column::Stage,
            Expr::value(stage.map(|stage| stage.as_str().to_string())),
        );
    }
    if let Some(error) = change.error {
        update = update.col_expr(deploy_runs::Column::Error, Expr::value(error));
    }
    if let Some(approved_by) = change.approved_by {
        update = update
            .col_expr(
                deploy_runs::Column::ApprovedBy,
                Expr::value(Some(approved_by)),
            )
            .col_expr(deploy_runs::Column::ApprovedAt, Expr::value(Some(now)));
    }
    if let Some(scheduled_for) = change.scheduled_for {
        update = update.col_expr(
            deploy_runs::Column::ScheduledFor,
            Expr::value(scheduled_for),
        );
    }
    if change.started {
        update = update.col_expr(deploy_runs::Column::StartedAt, Expr::value(Some(now)));
    }
    let result = update.exec(db).await?;
    if result.rows_affected == 1 && change.finished {
        finish_times(db, id).await?;
    }
    Ok(result.rows_affected == 1)
}

async fn finish_times<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<()> {
    let Some(run) = select_without_log()
        .filter(deploy_runs::Column::Id.eq(id))
        .one(db)
        .await?
    else {
        return Ok(());
    };
    let now = Utc::now();
    let duration = run
        .started_at
        .map(|started| (now - started.with_timezone(&Utc)).num_milliseconds());
    deploy_runs::Entity::update_many()
        .col_expr(deploy_runs::Column::FinishedAt, Expr::value(Some(now)))
        .col_expr(deploy_runs::Column::DurationMs, Expr::value(duration))
        .filter(deploy_runs::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

/// Grava a etapa em curso.
///
/// # Errors
///
/// Banco.
pub async fn set_stage<C: ConnectionTrait>(db: &C, id: i64, stage: DeployStage) -> AppResult<()> {
    deploy_runs::Entity::update_many()
        .col_expr(
            deploy_runs::Column::Stage,
            Expr::value(Some(stage.as_str())),
        )
        .col_expr(deploy_runs::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(deploy_runs::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

/// Grava o log inteiro (já limitado por [`LogBuffer`]).
///
/// # Errors
///
/// Banco.
pub async fn save_log<C: ConnectionTrait>(db: &C, id: i64, log: &str) -> AppResult<()> {
    deploy_runs::Entity::update_many()
        .col_expr(deploy_runs::Column::Log, Expr::value(Some(log.to_string())))
        .filter(deploy_runs::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

/// Grava o id do primeiro backup feito antes do deploy.
///
/// # Errors
///
/// Banco.
pub async fn set_backup<C: ConnectionTrait>(db: &C, id: i64, backup_id: i64) -> AppResult<()> {
    deploy_runs::Entity::update_many()
        .col_expr(deploy_runs::Column::BackupId, Expr::value(Some(backup_id)))
        .filter(deploy_runs::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

/// O log de um deploy em memória, com o teto de [`MAX_LOG_BYTES`].
#[derive(Debug, Default, Clone)]
pub struct LogBuffer {
    lines: VecDeque<String>,
    bytes: usize,
    truncated: bool,
}

impl LogBuffer {
    #[must_use]
    pub fn from_saved(saved: Option<&str>) -> Self {
        let mut buffer = Self::default();
        if let Some(saved) = saved {
            let saved = match saved.strip_prefix(TRUNCATED_MARK) {
                Some(rest) => {
                    buffer.truncated = true;
                    rest
                }
                None => saved,
            };
            for segment in saved.split_inclusive('\n') {
                buffer.bytes += segment.len();
                buffer.lines.push_back(segment.to_string());
            }
            buffer.trim();
        }
        buffer
    }

    pub fn push(&mut self, line: &str) {
        let mut entry = String::with_capacity(line.len() + 1);
        entry.push_str(line);
        entry.push('\n');
        self.bytes += entry.len();
        self.lines.push_back(entry);
        self.trim();
    }

    fn trim(&mut self) {
        while self.bytes > MAX_LOG_BYTES {
            self.truncated = true;
            if self.lines.len() > 1 {
                if let Some(removed) = self.lines.pop_front() {
                    self.bytes -= removed.len();
                }
            } else if let Some(only) = self.lines.front_mut() {
                let mut cut = only.len() - MAX_LOG_BYTES;
                while !only.is_char_boundary(cut) {
                    cut += 1;
                }
                only.drain(..cut);
                self.bytes = only.len();
            }
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        let extra = if self.truncated {
            TRUNCATED_MARK.len()
        } else {
            0
        };
        let mut out = String::with_capacity(self.bytes + extra);
        if self.truncated {
            out.push_str(TRUNCATED_MARK);
        }
        for line in &self.lines {
            out.push_str(line);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_guarda_o_fim() {
        let mut buffer = LogBuffer::default();
        for index in 0..20_000 {
            buffer.push(&format!("linha {index} ççç"));
        }
        let text = buffer.text();
        assert!(text.starts_with(TRUNCATED_MARK));
        assert!(text.len() <= MAX_LOG_BYTES + TRUNCATED_MARK.len());
        assert!(text.ends_with("linha 19999 ççç\n"));
        // Corta em fim de linha: nenhuma linha pela metade no começo.
        assert!(text[TRUNCATED_MARK.len()..].starts_with("linha "));
    }

    #[test]
    fn log_curto_fica_inteiro() {
        let mut buffer = LogBuffer::from_saved(Some("antes\n"));
        buffer.push("depois");
        assert_eq!(buffer.text(), "antes\ndepois\n");
    }
}
