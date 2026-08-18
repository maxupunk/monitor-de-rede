//! Adiciona o perfil de acesso ao usuário.
//!
//! Usuários existentes viram administradores para preservar exatamente o
//! acesso que tinham antes desta migration. Novas contas recebem um papel
//! explícito pelo serviço de usuários.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("users", "role").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("users"))
                    .add_column(string("role").not_null().default("admin"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("users", "role").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("users"))
                    .drop_column(Alias::new("role"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
