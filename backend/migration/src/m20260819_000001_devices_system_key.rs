//! A chave técnica que identifica um dispositivo **do próprio sistema**.
//!
//! O Servidor NetMonitor precisa ser encontrado sem depender de ID, nome, IP,
//! site ou rede: o ID varia por instalação, o nome é editável e os demais
//! campos podem ser nulos. A coluna guarda um identificador estável
//! (`netmonitor`) e é anulável — todo dispositivo comum a deixa em `NULL`.
//!
//! # Por que coluna anulável + índice único à parte
//!
//! `ALTER TABLE ... ADD COLUMN ... UNIQUE` não existe no SQLite, então a
//! unicidade vem de um `CREATE UNIQUE INDEX` posterior. E `NULL`s são
//! **distintos** tanto no SQLite quanto no PostgreSQL, de modo que o índice
//! restringe apenas as linhas que de fato declaram uma chave — exatamente o
//! que `devices_network_ip_unique` já explora neste mesmo esquema.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("devices", "system_key").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(string_null("system_key"))
                    .to_owned(),
            )
            .await?;
        }
        m.create_index(unique(
            "devices_system_key_unique",
            "devices",
            &["system_key"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_index(
            Index::drop()
                .name("devices_system_key_unique")
                .table(Alias::new("devices"))
                .to_owned(),
        )
        .await?;
        // Mesmo motivo das demais migrations de coluna: nem toda versão de
        // SQLite ainda atendida suporta `DROP COLUMN`.
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("devices", "system_key").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .drop_column(Alias::new("system_key"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
