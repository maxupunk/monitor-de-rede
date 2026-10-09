//! Os contadores SNMP da interface moram na linha dela, não no histórico.
//!
//! `ifHCInOctets`, `ifHCOutOctets`, `ifInErrors` e `ifOutErrors` são contadores
//! acumulados desde o boot do equipamento: o que importa deles é a **última**
//! leitura, que serve de base para a taxa seguinte. Gravá-los em `metrics` a
//! cada coleta fazia quatro das seis séries de cada interface — dois terços da
//! tabela — sem nenhuma tela que precisasse do histórico. A coleta já atualiza
//! a linha da interface (`last_seen_at`), então guardar a leitura ali não
//! acrescenta escrita alguma.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "device_interfaces";
const COUNTERS: [&str; 4] = ["in_octets", "out_octets", "in_errors", "out_errors"];
const READ_AT: &str = "counters_at";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for column in COUNTERS.into_iter().chain([READ_AT]) {
            if m.has_column(TABLE, column).await? {
                continue;
            }
            let definition = if column == READ_AT {
                timestamp_with_time_zone_null(column)
            } else {
                big_integer_null(column)
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
        for column in COUNTERS.into_iter().chain([READ_AT]) {
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
