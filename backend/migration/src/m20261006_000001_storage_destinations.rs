//! Armazenamentos: os destinos para onde vai o backup das configurações.
//!
//! A credencial de cada destino (chave do S3, senha do SFTP, connection string
//! do Azure) muda de forma conforme o provider, então ela vive num JSON só,
//! cifrado com a `ENCRYPTION_KEY` (`config_encrypted`). O que a tela lista e o
//! agendador consulta — nome, provider, política de backup e o resultado da
//! última execução — fica em coluna própria, para não decifrar nada à toa.

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
                .col(text("config_encrypted"))
                .col(boolean("backup_enabled").default(false))
                .col(integer("backup_interval_hours").default(24))
                .col(integer("backup_retention").default(14))
                .col(timestamp_with_time_zone_null("last_backup_at"))
                .col(string_len_null("last_backup_status", 16))
                .col(text_null("last_backup_error"));

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
