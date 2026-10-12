//! O Laya virou Decisor: o palpite de sistema operacional gravado como
//! `laya` em `devices.observed_os_source` passa a `decisor`, no mesmo padrão
//! em português das outras origens (`declarado`, `sonda`, `cadastro`).
//!
//! Um `UPDATE` simples, igual em SQLite e PostgreSQL. Desfazer volta o valor.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "devices";
const COLUMN: &str = "observed_os_source";

async fn rename(m: &SchemaManager<'_>, from: &str, to: &str) -> Result<(), DbErr> {
    let update = Query::update()
        .table(Alias::new(TABLE))
        .value(Alias::new(COLUMN), to)
        .and_where(Expr::col(Alias::new(COLUMN)).eq(from))
        .to_owned();
    m.exec_stmt(update).await
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        rename(m, "laya", "decisor").await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        rename(m, "decisor", "laya").await
    }
}
