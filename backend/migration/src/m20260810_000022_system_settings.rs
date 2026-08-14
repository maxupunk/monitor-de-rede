//! §6 #22 — `system_settings`. Chave/valor do sistema.
//!
//! Hoje guarda um registro só: `dashboard_layout` (§7.1). O formato genérico
//! existe para o próximo ajuste global não exigir migration.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("system_settings");
        stmt.col(big_pk_auto("id"))
            .col(string_len("key", 100))
            // `text`: o layout do dashboard é um JSON que cresce com o número de
            // cartões.
            .col(text_null("value"));

        m.create_table(with_timestamps(stmt.take())).await?;

        m.create_index(unique(
            "system_settings_key_unique",
            "system_settings",
            &["key"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("system_settings")).await?;
        Ok(())
    }
}
