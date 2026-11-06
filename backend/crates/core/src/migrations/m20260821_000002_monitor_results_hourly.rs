//! Fase 3 do roadmap — rollup horário de `monitor_results`.
//!
//! Agrega o histórico bruto de checagens em buckets de uma hora, permitindo
//! responder "este link é estável?" em 24h / 7d / 30d sem varrer milhões de
//! linhas a cada consulta.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("monitor_results_hourly").await? {
            let mut stmt = table("monitor_results_hourly");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("monitor_id"))
                .col(big_integer_null("probe_id"))
                .col(timestamp_with_time_zone("bucket"))
                .col(integer("total_checks"))
                .col(integer("up_checks"))
                .col(integer("down_checks"))
                .col(integer("unknown_checks"))
                .col(double_null("avg_latency_ms"))
                .col(double_null("min_latency_ms"))
                .col(double_null("max_latency_ms"))
                .col(timestamp_with_time_zone("first_started_at"))
                .col(timestamp_with_time_zone("last_finished_at"))
                .foreign_key(&mut fk(
                    "monitor_results_hourly",
                    "monitor_id",
                    "monitors",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "monitor_results_hourly",
                    "probe_id",
                    "probes",
                    ForeignKeyAction::SetNull,
                ));

            m.create_table(with_timestamps(stmt.take())).await?;

            m.create_index(unique(
                "monitor_results_hourly_monitor_bucket_unique",
                "monitor_results_hourly",
                &["monitor_id", "bucket"],
            ))
            .await?;
            m.create_index(index(
                "monitor_results_hourly_bucket_index",
                "monitor_results_hourly",
                &["bucket"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("monitor_results_hourly")).await?;
        Ok(())
    }
}
