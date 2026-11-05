//! Armazenamentos: os destinos para onde vão os backups.
//!
//! A credencial de cada destino (chave do S3, senha do SFTP, connection string
//! do Azure) muda de forma conforme o provider, então ela vive num JSON só,
//! cifrado com a `ENCRYPTION_KEY` (`config_encrypted`). O que a tela lista —
//! nome e provider — fica em coluna própria, para não decifrar nada à toa.
//!
//! O armazenamento é só o **lugar**: a agenda e a retenção do backup do
//! sistema moram em `system_backup_plan`, e as de cada banco em
//! `database_connections`.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("storage_destinations").await? {
            let mut stmt = table("storage_destinations");
            stmt.col(big_pk_auto("id"))
                .col(string_len("name", 120))
                .col(string_len("provider", 32))
                .col(text("config_encrypted"));

            m.create_table(with_timestamps(stmt.take())).await?;

            // Dois destinos com o mesmo nome seriam indistinguíveis na lista e
            // no seletor da restauração.
            m.create_index(unique(
                "storage_destinations_name_unique",
                "storage_destinations",
                &["name"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("storage_destinations")).await
    }
}
