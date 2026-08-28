//! Interface de entrada de link (WAN/Uplink) de cada dispositivo.
//!
//! Permite vincular qual interface do equipamento representa a entrada principal
//! do link de Internet / tráfego WAN para correlação de métricas de consumo de banda
//! com tempo de resposta nos monitores.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("devices", "link_interface_id").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(big_integer_null("link_interface_id"))
                    .to_owned(),
            )
            .await?;
        }
        if !m.has_column("devices", "link_interface_name").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(string_null("link_interface_name"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
            if m.has_column("devices", "link_interface_name").await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("devices"))
                        .drop_column(Alias::new("link_interface_name"))
                        .to_owned(),
                )
                .await?;
            }
            if m.has_column("devices", "link_interface_id").await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("devices"))
                        .drop_column(Alias::new("link_interface_id"))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }
}
