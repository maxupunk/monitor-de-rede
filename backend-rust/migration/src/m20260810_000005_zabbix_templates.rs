//! §6 #05 — `zabbix_templates`. Export bruto do Zabbix guardado como veio.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("zabbix_templates");
        stmt.col(big_pk_auto("id"))
            // `zabbix_uuid` é a chave de reimport: um template reenviado com o
            // mesmo uuid substitui os itens **preservando o id** (§7.14), para
            // os devices que já apontam para ele não perderem o vínculo.
            .col(string_null("zabbix_uuid"))
            .col(string("name"))
            .col(text_null("description"))
            .col(string_null("zabbix_version"))
            .col(json_binary("raw_export"))
            .col(timestamp_with_time_zone("imported_at"));

        m.create_table(with_timestamps(stmt.take())).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("zabbix_templates")).await?;
        Ok(())
    }
}
