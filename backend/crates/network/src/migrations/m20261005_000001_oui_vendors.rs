//! Registro de fabricantes do IEEE (`oui_vendors`).
//!
//! Dado público e derivado: o servidor baixa e substitui inteiro, e o backup
//! não o leva — restaurar não precisa de 50 mil linhas que se baixam de novo.
//! A chave é o próprio prefixo hexadecimal (6, 7 ou 9 dígitos).

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("oui_vendors").await? {
            let mut stmt = table("oui_vendors");
            stmt.col(
                ColumnDef::new(Alias::new("prefix"))
                    .string_len(9)
                    .not_null()
                    .primary_key()
                    .take(),
            )
            .col(string_len("organization", 255))
            .col(string_len("registry", 8))
            .col(timestamp_with_time_zone("updated_at"));
            m.create_table(stmt.take()).await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("oui_vendors")).await
    }
}
