//! Perna que atravessa equipamentos sem IP (ADR 015 §5.2).
//!
//! * `edge_ids` — todas as arestas da perna, em ordem, como `,10,20,`: o
//!   filtro "testes deste enlace" vira um `LIKE` que vale igual no SQLite e no
//!   PostgreSQL, como o `link_tests.device_ids`.
//! * `via` — os equipamentos sem IP atravessados (`[{ deviceId, name }]`),
//!   com o nome, para o histórico sobreviver à exclusão.
//!
//! Perna antiga fica com as duas colunas nulas: a leitura deriva a lista da
//! `edge_id`.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "link_test_legs";
const EDGE_IDS: &str = "edge_ids";
const VIA: &str = "via";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for column in [EDGE_IDS, VIA] {
            if m.has_column(TABLE, column).await? {
                continue;
            }
            let definition = if column == EDGE_IDS {
                text_null(column)
            } else {
                json_binary_null(column)
            };
            m.alter_table(
                Table::alter()
                    .table(Alias::new(TABLE))
                    .add_column(definition)
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if m.get_connection().get_database_backend() == sea_orm::DatabaseBackend::Sqlite {
            return Ok(());
        }
        for column in [EDGE_IDS, VIA] {
            if m.has_column(TABLE, column).await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new(TABLE))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }
}
