//! Plugins de frota: configuração guardada e execução em lote.
//!
//! * `plugin_settings` — o estado desejado de um plugin. `device_id` nulo é a
//!   configuração da frota (ex.: os SSIDs da rede Wi-Fi); preenchido, o ajuste
//!   daquele equipamento (ex.: o canal do rádio). Campos secretos (senha do
//!   Wi-Fi) ficam cifrados dentro do JSON. Uma linha por (plugin, escopo) —
//!   regra de serviço: `NULL` em índice único não vale igual nos dois bancos.
//! * `plugin_batches` — uma ação de frota: a mesma ação em vários
//!   equipamentos, o andamento de cada um e o consolidado (`result`). A
//!   execução de cada equipamento aponta o lote por `plugin_runs.batch_id`.

use sea_orm_migration::prelude::*;

use netmonitor_core::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("plugin_settings").await? {
            let mut stmt = table("plugin_settings");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("plugin_id"))
                .col(big_integer_null("device_id"))
                .col(json_binary("value"))
                .col(integer("version").default(1))
                .col(big_integer_null("updated_by"))
                .foreign_key(&mut fk(
                    "plugin_settings",
                    "plugin_id",
                    "plugins",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "plugin_settings",
                    "device_id",
                    "devices",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "plugin_settings",
                    "updated_by",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index(
                "plugin_settings_plugin_device_index",
                "plugin_settings",
                &["plugin_id", "device_id"],
            ))
            .await?;
        }

        if !m.has_table("plugin_batches").await? {
            let mut stmt = table("plugin_batches");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("plugin_id"))
                .col(string_len("action", 64))
                .col(string_len("status", 24))
                .col(json_binary_null("params"))
                .col(json_binary("devices"))
                .col(json_binary_null("result"))
                .col(text_null("error"))
                .col(big_integer_null("user_id"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .foreign_key(&mut fk(
                    "plugin_batches",
                    "plugin_id",
                    "plugins",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "plugin_batches",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index(
                "plugin_batches_plugin_created_index",
                "plugin_batches",
                &["plugin_id", "created_at"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("plugin_batches")).await?;
        m.drop_table(drop("plugin_settings")).await?;
        Ok(())
    }
}
