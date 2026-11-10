//! O registro de cada deploy: criar, mudar de estado sem corrida e guardar o
//! log.
//!
//! Mudança de estado é **condicional** (`UPDATE … WHERE status IN (…)`): o
//! despachante, a aprovação e o cancelamento podem disputar o mesmo deploy,
//! e só um vence. Quem perde recebe `false` e não faz nada.

use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use netmonitor_core::{
    models::deploy_runs,
    shared::errors::{AppError, AppResult},
};

use super::{DeployStage, RunStatus, RunTrigger};

/// Teto do log gravado: guarda o fim, que é onde está o erro.
pub const MAX_LOG_BYTES: usize = 256 * 1024;
const TRUNCATED_MARK: &str = "… (início do log omitido)\n";

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
    let mut query = deploy_runs::Entity::find()
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
    Ok(deploy_runs::Entity::find()
        .filter(deploy_runs::Column::Status.is_in(statuses.iter().map(|status| status.as_str())))
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
    Ok(deploy_runs::Entity::find()
        .filter(deploy_runs::Column::TargetId.eq(target_id))
        .filter(deploy_runs::Column::Status.eq(RunStatus::Succeeded.as_str()))
        .order_by_desc(deploy_runs::Column::Id)
        .one(db)
        .await?)
}

/// O alvo tem um deploy rodando?
///
/// # Errors
///
/// Banco.
pub async fn running_on<C: ConnectionTrait>(db: &C, target_id: i64) -> AppResult<bool> {
    Ok(deploy_runs::Entity::find()
        .filter(deploy_runs::Column::TargetId.eq(target_id))
        .filter(deploy_runs::Column::Status.eq(RunStatus::Running.as_str()))
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
/// # Errors
///
/// Banco. `Ok(false)` quando outro caminho mudou o estado antes.
pub async fn transition<C: ConnectionTrait>(
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
    let run = find(db, id).await?;
    let now = Utc::now();
    let duration = run
        .started_at
        .map(|started| (now - started.with_timezone(&Utc)).num_milliseconds());
    let mut active: deploy_runs::ActiveModel = run.into();
    active.finished_at = Set(Some(now.into()));
    active.duration_ms = Set(duration);
    active.update(db).await?;
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
    text: String,
    truncated: bool,
}

impl LogBuffer {
    #[must_use]
    pub fn from_saved(saved: Option<&str>) -> Self {
        let mut buffer = Self::default();
        if let Some(saved) = saved {
            buffer.text.push_str(saved);
        }
        buffer
    }

    pub fn push(&mut self, line: &str) {
        self.text.push_str(line);
        self.text.push('\n');
        if self.text.len() > MAX_LOG_BYTES {
            let mut cut = self.text.len() - MAX_LOG_BYTES;
            while !self.text.is_char_boundary(cut) {
                cut += 1;
            }
            let cut = self.text[cut..]
                .find('\n')
                .map_or(cut, |line| cut + line + 1);
            self.text.drain(..cut);
            self.truncated = true;
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        if self.truncated {
            format!("{TRUNCATED_MARK}{}", self.text)
        } else {
            self.text.clone()
        }
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
