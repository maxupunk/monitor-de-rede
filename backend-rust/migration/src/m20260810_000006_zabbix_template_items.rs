//! §6 #06 — `zabbix_template_items`. Itens SNMP extraídos do export.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("zabbix_template_items");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("template_id"))
            .col(string_null("zabbix_uuid"))
            .col(string("name"))
            .col(string("key"))
            .col(string("snmp_oid"))
            .col(string("value_type"))
            .col(string_null("units"))
            .col(float_null("multiplier"))
            .foreign_key(&mut fk(
                "zabbix_template_items",
                "template_id",
                "zabbix_templates",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(append_only(stmt.take())).await?;

        m.create_index(index(
            "zabbix_template_items_template_id_index",
            "zabbix_template_items",
            &["template_id"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("zabbix_template_items")).await?;
        Ok(())
    }
}
