//! Liga o probe/agente ao dispositivo que o hospeda (ADR 011).
//!
//! Um agente instalado num servidor da VPN é o mesmo equipamento que o peer
//! já criou em `devices`. Com o vínculo, a central sabe de que túnel a conexão
//! do agente precisa vir e a tela mostra servidor e agente juntos.
//!
//! A FK vai no próprio `ADD COLUMN`, com `ON DELETE SET NULL`: apagar o
//! dispositivo não pode apagar o agente (ele continua sendo um probe válido),
//! só desfaz o vínculo. SQLite e PostgreSQL aceitam a mesma sintaxe — o
//! SQLite exige apenas que o default seja nulo, e é.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDICE: &str = "probes_device_id_index";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let backend = db.get_database_backend();
        if !m.has_column("probes", "device_id").await? {
            db.execute_raw(Statement::from_string(
                backend,
                "ALTER TABLE probes ADD COLUMN device_id BIGINT NULL \
                 REFERENCES devices (id) ON DELETE SET NULL ON UPDATE CASCADE"
                    .to_string(),
            ))
            .await?;
        }
        db.execute_raw(Statement::from_string(
            backend,
            format!("CREATE INDEX IF NOT EXISTS {INDICE} ON probes (device_id)"),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!("DROP INDEX IF EXISTS {INDICE}"),
        ))
        .await?;
        if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("probes", "device_id").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("probes"))
                    .drop_column(Alias::new("device_id"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
