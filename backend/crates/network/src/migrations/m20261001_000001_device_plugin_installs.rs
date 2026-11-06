//! Plugins instalados em cada equipamento.
//!
//! O catálogo diz o que **serve** ao equipamento (compatibilidade); a
//! instalação diz o que o operador **escolheu usar** nele — e é ela que dá ao
//! plugin a sua aba em `/devices/{id}`. Instalar não toca o equipamento: é uma
//! decisão de cadastro. Um plugin aparece uma vez por equipamento.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("device_plugin_installs").await? {
            let mut stmt = table("device_plugin_installs");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("device_id"))
                .col(big_integer("plugin_id"))
                .col(big_integer_null("user_id"))
                .foreign_key(&mut fk(
                    "device_plugin_installs",
                    "device_id",
                    "devices",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "device_plugin_installs",
                    "plugin_id",
                    "plugins",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "device_plugin_installs",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "device_plugin_installs_device_plugin_unique",
                "device_plugin_installs",
                &["device_id", "plugin_id"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("device_plugin_installs")).await?;
        Ok(())
    }
}
