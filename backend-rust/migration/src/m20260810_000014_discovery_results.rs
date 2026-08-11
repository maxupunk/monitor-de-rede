//! §6 #14 — `discovery_results`.
//!
//! Cache do último scan de cada rede, sem histórico persistente de status: um
//! resultado existe enquanto não foi transformado em device, e a verificação de
//! "já adicionado" é feita comparando o IP com a tabela `devices`.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("discovery_results");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("discovery_run_id"))
            .col(string("ip_address"))
            .col(string_null("mac_address"))
            .col(string_null("hostname"))
            .col(string_null("mdns_name"))
            .col(string_null("vendor"))
            .col(string_null("device_type"))
            .col(integer("confidence").default(0).take())
            .col(json_binary_null("data"))
            .col(timestamp_with_time_zone("first_seen_at"))
            .col(timestamp_with_time_zone("last_seen_at"))
            .foreign_key(&mut fk(
                "discovery_results",
                "discovery_run_id",
                "discovery_runs",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // Sustenta o `withCount('results')` da tela de Descoberta e o CASCADE.
        m.create_index(index(
            "discovery_results_discovery_run_id_index",
            "discovery_results",
            &["discovery_run_id"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("discovery_results")).await?;
        Ok(())
    }
}
