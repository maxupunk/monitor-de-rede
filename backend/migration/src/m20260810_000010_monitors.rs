//! §6 #10 — `monitors`. A unidade de checagem.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("monitors");
        stmt.col(big_pk_auto("id"))
            // Nem toda checagem pertence a um equipamento: medir a latência de um
            // servidor DNS público ou a disponibilidade de um site externo não
            // depende de um dispositivo cadastrado.
            .col(big_integer_null("device_id"))
            .col(big_integer_null("probe_id"))
            .col(string("type"))
            .col(string("name"))
            .col(json_binary("configuration"))
            .col(integer("interval_seconds").default(15).take())
            .col(integer("timeout_seconds").default(10).take())
            .col(integer("retry_count").default(3).take())
            .col(boolean("enabled").default(true).take())
            .col(timestamp_with_time_zone_null("next_run_at"))
            .col(timestamp_with_time_zone_null("last_run_at"))
            .col(string("status").default("unknown").take())
            .foreign_key(&mut fk(
                "monitors",
                "device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ))
            // Probe removido não pode levar o monitor junto: ele volta a rodar
            // localmente (o fallback do §9.2).
            .foreign_key(&mut fk(
                "monitors",
                "probe_id",
                "probes",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // Seleção dos monitores vencidos: o laço central do `scheduler_run`.
        m.create_index(index(
            "monitors_enabled_next_run_at_index",
            "monitors",
            &["enabled", "next_run_at"],
        ))
        .await?;
        m.create_index(index(
            "monitors_device_id_enabled_index",
            "monitors",
            &["device_id", "enabled"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("monitors")).await?;
        Ok(())
    }
}
