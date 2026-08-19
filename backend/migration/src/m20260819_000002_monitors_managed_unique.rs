//! Um único monitor gerenciado por dispositivo.
//!
//! O provisionamento da coleta de saúde é idempotente em Rust, mas dois boots
//! simultâneos podem passar juntos pelo `SELECT` e inserir os dois. Quem
//! decide a corrida é o banco.
//!
//! # Por que o índice é **parcial**
//!
//! Um `UNIQUE (device_id, type)` global quebraria o esquema atual: a coleta
//! SNMP cria dois monitores `type = 'snmp'` no mesmo dispositivo — um para
//! `cpu_usage` e outro para `memory_usage` (ver `snmp::service`). O que precisa
//! ser único é o monitor **gerenciado**, então a condição entra no índice.
//!
//! `CREATE UNIQUE INDEX ... WHERE` existe tanto no SQLite (desde 3.8) quanto
//! no PostgreSQL, com a mesma sintaxe — é por isso que este é o único ponto do
//! esquema que usa SQL cru em vez do construtor do SeaORM: o `IndexCreate`
//! não expressa índice parcial.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const NOME: &str = "monitors_managed_unique";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        // Uma instalação que já tenha duplicatas não pode travar o upgrade.
        // Mantemos a mais antiga — é a que tem histórico.
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "DELETE FROM monitors WHERE type = 'system_health' AND id NOT IN (\
                 SELECT MIN(id) FROM monitors WHERE type = 'system_health' GROUP BY device_id\
             )"
            .to_string(),
        ))
        .await?;
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!(
                "CREATE UNIQUE INDEX IF NOT EXISTS {NOME} ON monitors (device_id, type) \
                 WHERE type = 'system_health'"
            ),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            format!("DROP INDEX IF EXISTS {NOME}"),
        ))
        .await?;
        Ok(())
    }
}
