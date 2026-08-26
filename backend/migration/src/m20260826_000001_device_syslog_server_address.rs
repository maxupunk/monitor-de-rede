//! Último endereço de Syslog aplicado em cada dispositivo.
//!
//! O catálogo global diz quais endereços este servidor oferece; esta coluna diz
//! qual deles foi realmente gravado em um equipamento específico. Separar os
//! dois evita que a escolha de um dispositivo apareça como preferência de
//! outro e permite reabrir o assistente mostrando o estado aplicado.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("devices", "syslog_server_address").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(string_null("syslog_server_address"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // SQLite antigo não oferece um DROP COLUMN compatível com todas as
        // instalações suportadas; segue o padrão das demais colunas opcionais.
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("devices", "syslog_server_address").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .drop_column(Alias::new("syslog_server_address"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
