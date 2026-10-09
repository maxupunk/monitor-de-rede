//! "Acessar a partir de" na conexão de banco (ADR 013).
//!
//! `via_probe_id` nulo: a central conecta direto. Preenchido: o backup passa
//! pela ponte do agente remoto, para bancos que só a rede dele alcança — o
//! mesmo padrão de `device_credentials.via_probe_id`.
//!
//! A FK vai no próprio `ADD COLUMN`, com `ON DELETE SET NULL`, como em
//! `probes.device_id`: apagar o agente não apaga a conexão, só desfaz a rota.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        if !m.has_column("database_connections", "via_probe_id").await? {
            db.execute_raw(Statement::from_string(
                db.get_database_backend(),
                "ALTER TABLE database_connections ADD COLUMN via_probe_id BIGINT NULL \
                 REFERENCES probes (id) ON DELETE SET NULL ON UPDATE CASCADE"
                    .to_string(),
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("database_connections", "via_probe_id").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("database_connections"))
                    .drop_column(Alias::new("via_probe_id"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
