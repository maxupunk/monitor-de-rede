//! §6 #13 — `discovery_runs`. Uma varredura de rede, enfileirada ou concluída.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("discovery_runs");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("network_id"))
            .col(big_integer_null("probe_id"))
            .col(string("status"))
            .col(timestamp_with_time_zone("started_at"))
            .col(timestamp_with_time_zone_null("finished_at"))
            .col(json_binary_null("configuration"))
            .col(text_null("error"))
            .foreign_key(&mut fk(
                "discovery_runs",
                "network_id",
                "networks",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "discovery_runs",
                "probe_id",
                "probes",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // Próxima varredura pendente — consultado a cada ciclo do scheduler.
        m.create_index(index(
            "discovery_runs_status_id_index",
            "discovery_runs",
            &["status", "id"],
        ))
        .await?;
        m.create_index(index(
            "discovery_runs_network_status_index",
            "discovery_runs",
            &["network_id", "status"],
        ))
        .await?;
        m.create_index(index(
            "discovery_runs_created_at_index",
            "discovery_runs",
            &["created_at"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("discovery_runs")).await?;
        Ok(())
    }
}
