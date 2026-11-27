//! O contêiner de onde a conexão de banco veio.
//!
//! Um contêiner recriado ou reiniciado pode voltar com outro IP, e o backup
//! que guardava só o endereço parava de alcançar o banco. Com o vínculo, a
//! central reencontra o contêiner no host dele antes de cada backup e atualiza
//! o endereço sozinha (`databases::follow`).
//!
//! Guarda o que sobrevive à recriação — o host (`agent-<id>`), o nome do
//! contêiner e o projeto/serviço do compose —, nunca o id, que muda a cada
//! `docker compose up`. Tudo anulável: conexão digitada à mão, ou cadastrada
//! antes desta migration, segue sem vínculo até alguém vincular.

use sea_orm_migration::prelude::*;

use netmonitor_core::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "database_connections";

/// Uma coluna por `ALTER`: o SQLite não aceita várias no mesmo comando.
const COLUMNS: [(&str, u32); 4] = [
    ("container_host_key", 64),
    ("container_name", 255),
    ("container_project", 255),
    ("container_service", 255),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for (column, len) in COLUMNS {
            if !m.has_column(TABLE, column).await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new(TABLE))
                        .add_column(string_len_null(column, len))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for (column, _) in COLUMNS.iter().rev() {
            if m.has_column(TABLE, column).await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new(TABLE))
                        .drop_column(Alias::new(*column))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }
}
