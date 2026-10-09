//! Backup automático dos bancos: o ciclo que dispara cada conexão vencida.
//!
//! Mesma agenda dos armazenamentos ([`BackupPolicy`]): o cadastro guarda a
//! última execução e o resultado, e é isso que diz quem vence.

use std::time::Duration;

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::{
    backups::{self, Trigger, BACKUP_SCOPE},
    jobs::{DatabaseJobKind, Job},
    service,
};
use crate::{
    models::database_connections,
    services::{
        events::EventBus,
        shared::{backup_schedule::BackupPolicy, errors::AppResult, run_guard::RunGuard},
    },
};

const TICK: Duration = Duration::from_secs(60);

/// Depois do boot e do primeiro ciclo dos armazenamentos: não somar as duas
/// rajadas.
const FIRST_TICK_DELAY: Duration = Duration::from_secs(180);

/// O que a agenda compartilhada precisa saber desta conexão.
#[must_use]
pub fn policy(row: &database_connections::Model) -> BackupPolicy {
    BackupPolicy {
        enabled: row.backup_enabled && row.storage_destination_id.is_some(),
        interval_hours: row.backup_interval_hours,
        last_run_at: row.last_backup_at.map(|at| at.with_timezone(&Utc)),
        last_run_failed: row.last_backup_status.as_deref() == Some(backups::STATUS_FAILED),
    }
}

/// Sobe o ciclo. Fora do ambiente de teste.
pub fn spawn(ctx: &AppContext) {
    if ctx.environment == loco_rs::environment::Environment::Test {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(FIRST_TICK_DELAY).await;
        loop {
            if let Err(error) = run_due(&ctx, Utc::now()).await {
                tracing::warn!(%error, "ciclo de backup de bancos falhou");
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// Faz o backup de cada conexão vencida, uma de cada vez.
///
/// Conexão com backup manual em andamento fica para o próximo ciclo.
///
/// # Errors
///
/// Só a falha da consulta inicial.
pub async fn run_due(ctx: &AppContext, now: DateTime<Utc>) -> AppResult<usize> {
    let rows = database_connections::Entity::find()
        .filter(database_connections::Column::BackupEnabled.eq(true))
        .all(&ctx.db)
        .await?;
    let mut ran = 0;
    for row in rows.into_iter().filter(|row| policy(row).is_due(now)) {
        let id = row.id;
        let Ok(_guard) = RunGuard::acquire(BACKUP_SCOPE, id, "ocupado") else {
            continue;
        };
        let prepared = async {
            let connection = service::open(row)?;
            let destination = backups::destination_of(&ctx.db, &connection).await?;
            AppResult::Ok((connection, destination))
        }
        .await;
        match prepared {
            Ok((connection, destination)) => {
                let job = Job::start(
                    EventBus::from_context(ctx).ok(),
                    DatabaseJobKind::Backup,
                    id,
                    u32::try_from(connection.databases.len()).unwrap_or(0),
                );
                backups::run_backup(ctx, &connection, &destination, &job, Trigger::Scheduled).await;
            }
            Err(error) => {
                tracing::warn!(connection = id, %error, "backup automático de banco não começou")
            }
        }
        ran += 1;
        service::publish_updated(ctx).await;
    }
    Ok(ran)
}
