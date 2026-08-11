//! §6 #04 — `networks`. Sub-redes varríveis.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("networks");
        stmt.col(big_pk_auto("id"))
            // Opcional: uma sub-rede pode ser cadastrada antes de existir um Site.
            // Exigir o vínculo obrigava a inventar um Site só para poder varrer uma
            // faixa — ver `VpnServerService.resolveNetwork`.
            .col(big_integer_null("site_id"))
            .col(big_integer_null("probe_id"))
            .col(string("name"))
            .col(string("cidr"))
            .col(string_null("gateway"))
            .col(integer_null("vlan"))
            .col(json_binary_null("dns_servers"))
            .col(boolean("scan_enabled").default(true).take())
            .col(integer("scan_interval").default(3600).take())
            .col(boolean("active").default(true).take())
            // Rastreamento das varreduras periódicas: sem saber quando a rede foi
            // varrida pela última vez o scheduler não tem como decidir quais estão
            // vencidas — mesmo par `last_run_at` / `next_run_at` dos monitores.
            .col(timestamp_with_time_zone_null("last_scan_at"))
            .col(timestamp_with_time_zone_null("next_scan_at"))
            .foreign_key(&mut fk(
                "networks",
                "site_id",
                "sites",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "networks",
                "probe_id",
                "probes",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("networks")).await?;
        Ok(())
    }
}
