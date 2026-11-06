//! Fase 3 do roadmap — janelas de manutenção.
//!
//! Permite agendar intervalos em que alertas e notificações de um site ou
//! dispositivo são suprimidos. A tabela é consultada pelo despachante de
//! notificações: o alerta ainda é criado, mas a mensagem não sai enquanto a
//! janela vigorar.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("maintenance_windows").await? {
            let mut stmt = table("maintenance_windows");
            stmt.col(big_pk_auto("id"))
                .col(big_integer_null("site_id"))
                .col(big_integer_null("device_id"))
                .col(string("name"))
                .col(text_null("description"))
                .col(timestamp_with_time_zone("starts_at"))
                .col(timestamp_with_time_zone("ends_at"))
                .col(big_integer_null("created_by"))
                .foreign_key(&mut fk(
                    "maintenance_windows",
                    "site_id",
                    "sites",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "maintenance_windows",
                    "device_id",
                    "devices",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "maintenance_windows",
                    "created_by",
                    "users",
                    ForeignKeyAction::SetNull,
                ));

            m.create_table(with_timestamps(stmt.take())).await?;

            m.create_index(index(
                "maintenance_windows_active_index",
                "maintenance_windows",
                &["starts_at", "ends_at"],
            ))
            .await?;
            m.create_index(index(
                "maintenance_windows_site_index",
                "maintenance_windows",
                &["site_id"],
            ))
            .await?;
            m.create_index(index(
                "maintenance_windows_device_index",
                "maintenance_windows",
                &["device_id"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("maintenance_windows")).await?;
        Ok(())
    }
}
