//! CPU e memória das pontas durante a medição (ADR 015 §12).
//!
//! * `health` — `[{ deviceId, atMs, cpu, memory }]`, lido por SNMP (ou do
//!   próprio servidor) a cada segundo enquanto a perna mede, no mesmo relógio
//!   das amostras de vazão (`samples[].atMs`): a tela desenha os dois juntos.
//!
//! Perna antiga fica com a coluna nula.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "link_test_legs";
const HEALTH: &str = "health";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if m.has_column(TABLE, HEALTH).await? {
            return Ok(());
        }
        m.alter_table(
            Table::alter()
                .table(Alias::new(TABLE))
                .add_column(json_binary_null(HEALTH))
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if m.get_connection().get_database_backend() == sea_orm::DatabaseBackend::Sqlite {
            return Ok(());
        }
        if m.has_column(TABLE, HEALTH).await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new(TABLE))
                    .drop_column(Alias::new(HEALTH))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
