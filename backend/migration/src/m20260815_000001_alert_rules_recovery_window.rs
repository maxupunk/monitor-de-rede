//! Fase 1 do roadmap de alertas inteligentes — histerese de resolução.
//!
//! `recovery_window_seconds` é o tempo que o alvo precisa permanecer estável,
//! contado a partir do **último** problema, antes de o alerta fechar de
//! verdade. `0` preserva o comportamento original: resolve na primeira
//! checagem ok. É parâmetro de regra (§3.4 do roadmap): cada escopo tolera a
//! oscilação que fizer sentido para ele.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m
            .has_column("alert_rules", "recovery_window_seconds")
            .await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("alert_rules"))
                    .add_column(integer("recovery_window_seconds").not_null().default(0))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Mesmo motivo do m20260814_000001: o SQLite de produção suporta ADD
        // COLUMN, mas não oferece um DROP COLUMN compatível com todas as
        // versões que ainda atendemos.
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("alert_rules", "recovery_window_seconds")
                .await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("alert_rules"))
                    .drop_column(Alias::new("recovery_window_seconds"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
