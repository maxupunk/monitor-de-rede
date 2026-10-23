//! Tarefas agendadas: uma ferramenta do catálogo da IA, com os argumentos,
//! rodada numa expressão cron (hora local do servidor).
//!
//! * `scheduled_tasks` — o agendamento: `tool` + `arguments`, `cron`, ativo,
//!   próxima e última execução, quem criou (`user_id`, dono da execução) e
//!   por onde (`origin`: tela ou chat).
//! * `scheduled_task_runs` — cada execução: gatilho (agenda ou "rodar agora"),
//!   desfecho, duração, resumo e o resultado da ferramenta (cortado).

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("scheduled_tasks").await? {
            let mut stmt = table("scheduled_tasks");
            stmt.col(big_pk_auto("id"))
                .col(string_len("name", 120))
                .col(text_null("description"))
                .col(string_len("tool", 64))
                .col(json_binary("arguments"))
                .col(string_len("cron", 120))
                .col(boolean("enabled").default(true))
                .col(string_len("origin", 16))
                .col(timestamp_with_time_zone_null("next_run_at"))
                .col(timestamp_with_time_zone_null("last_run_at"))
                .col(string_len_null("last_status", 16))
                .col(text_null("last_message"))
                .col(big_integer_null("user_id"))
                .foreign_key(&mut fk(
                    "scheduled_tasks",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index(
                "scheduled_tasks_enabled_next_index",
                "scheduled_tasks",
                &["enabled", "next_run_at"],
            ))
            .await?;
        }

        if !m.has_table("scheduled_task_runs").await? {
            let mut stmt = table("scheduled_task_runs");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("task_id"))
                .col(string_len("trigger", 16))
                .col(string_len("status", 16))
                .col(timestamp_with_time_zone("started_at"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .col(big_integer_null("duration_ms"))
                .col(text_null("message"))
                .col(json_binary_null("result"))
                .col(big_integer_null("user_id"))
                .foreign_key(&mut fk(
                    "scheduled_task_runs",
                    "task_id",
                    "scheduled_tasks",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "scheduled_task_runs",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(stmt.take()).await?;
            m.create_index(index(
                "scheduled_task_runs_task_index",
                "scheduled_task_runs",
                &["task_id", "id"],
            ))
            .await?;
            m.create_index(index(
                "scheduled_task_runs_started_index",
                "scheduled_task_runs",
                &["started_at"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("scheduled_task_runs")).await?;
        m.drop_table(drop("scheduled_tasks")).await?;
        Ok(())
    }
}
