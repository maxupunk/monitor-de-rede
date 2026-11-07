//! Linhas do banco → contrato HTTP.

use chrono::{DateTime, Utc};
use sea_orm::prelude::DateTimeWithTimeZone;

use super::{
    model::{
        ScheduledTask, ScheduledTaskOrigin, ScheduledTaskRun, ScheduledTaskStatus,
        ScheduledTaskTrigger,
    },
    runner::SCOPE,
    schedule,
};
use crate::{
    models::{scheduled_task_runs, scheduled_tasks},
    services::shared::run_guard::RunGuard,
};

/// Instante no formato do contrato (RFC 3339).
#[must_use]
pub fn iso(at: &DateTimeWithTimeZone) -> String {
    at.to_rfc3339()
}

fn iso_opt(at: Option<&DateTimeWithTimeZone>) -> Option<String> {
    at.map(iso)
}

#[must_use]
pub fn iso_utc(at: DateTime<Utc>) -> String {
    at.to_rfc3339()
}

/// Uma tarefa, com o nome de quem a assina.
#[must_use]
pub fn task(row: &scheduled_tasks::Model, owner_name: Option<String>) -> ScheduledTask {
    ScheduledTask {
        id: row.id,
        name: row.name.clone(),
        description: row.description.clone(),
        tool: row.tool.clone(),
        arguments: row.arguments.clone(),
        cron: row.cron.clone(),
        schedule_label: schedule::describe(&row.cron),
        enabled: row.enabled,
        origin: ScheduledTaskOrigin::from_db(&row.origin),
        next_run_at: iso_opt(row.next_run_at.as_ref()),
        last_run_at: iso_opt(row.last_run_at.as_ref()),
        last_status: row
            .last_status
            .as_deref()
            .and_then(ScheduledTaskStatus::from_db),
        last_message: row.last_message.clone(),
        owner_id: row.user_id,
        owner_name,
        running: RunGuard::is_running(SCOPE, row.id),
        created_at: iso(&row.created_at),
        updated_at: iso(&row.updated_at),
    }
}

/// Uma execução.
#[must_use]
pub fn run(row: scheduled_task_runs::Model) -> ScheduledTaskRun {
    ScheduledTaskRun {
        id: row.id,
        task_id: row.task_id,
        trigger: ScheduledTaskTrigger::from_db(&row.trigger),
        status: ScheduledTaskStatus::from_db(&row.status).unwrap_or(ScheduledTaskStatus::Failed),
        started_at: iso(&row.started_at),
        finished_at: iso_opt(row.finished_at.as_ref()),
        duration_ms: row.duration_ms,
        message: row.message,
        result: row.result,
    }
}
