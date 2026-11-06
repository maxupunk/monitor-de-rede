//! §6 #19 — `dns_servers`. Resolvedores comparados pelo benchmark.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("dns_servers");
        stmt.col(big_pk_auto("id"))
            .col(string("name"))
            // IP, `ip:porta` (UDP/TCP) ou endpoint https (DoH)
            .col(string("address"))
            .col(string("protocol").default("udp").take())
            // Participa da comparação de latência exibida no dashboard
            .col(boolean("is_default").default(true).take())
            .col(string_null("description"));

        m.create_table(with_timestamps(stmt.take())).await?;

        // O par é a identidade: 1.1.1.1 por UDP e por DoH são dois servidores
        // distintos. É este UNIQUE que produz o 409 do §7.15.
        m.create_index(unique(
            "dns_servers_address_protocol_unique",
            "dns_servers",
            &["address", "protocol"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("dns_servers")).await?;
        Ok(())
    }
}
