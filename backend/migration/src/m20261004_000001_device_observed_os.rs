//! O sistema que o equipamento **mostrou** ser.
//!
//! `devices.operating_system` é a declaração do operador; o que o fabricante e
//! o modelo sugerem é hardware (uma RouterBOARD pode rodar OpenWrt). Faltava
//! guardar a evidência de software — o servidor SSH (`dropbear`), o SNMP, um
//! plugin que leu `/etc/openwrt_release` — ou, sem ela, o palpite do Laya. É
//! isso que decide a compatibilidade de um plugin quando ninguém declarou.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const COLUMNS: [&str; 3] = ["observed_os", "observed_os_source", "observed_os_reason"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for column in COLUMNS {
            if !m.has_column("devices", column).await? {
                let definition = if column == "observed_os_reason" {
                    text_null(column)
                } else {
                    string_null(column)
                };
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("devices"))
                        .add_column(definition)
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if m.get_connection().get_database_backend() == sea_orm::DatabaseBackend::Sqlite {
            return Ok(());
        }
        for column in COLUMNS {
            if m.has_column("devices", column).await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("devices"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }
}
