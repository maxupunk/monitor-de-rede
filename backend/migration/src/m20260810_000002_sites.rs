//! §6 #02 — `sites`. Raiz da hierarquia: rede, dispositivo e probe pendem dela.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("sites");
        stmt.col(big_pk_auto("id"))
            .col(string("name"))
            .col(string_null("description"))
            .col(string_null("location"))
            .col(boolean("active").default(true).take());

        m.create_table(with_timestamps(stmt.take())).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("sites")).await?;
        Ok(())
    }
}
