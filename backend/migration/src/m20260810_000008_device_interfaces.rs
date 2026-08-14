//! §6 #08 — `device_interfaces`. Portas/interfaces descobertas por SNMP.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("device_interfaces");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("device_id"))
            .col(integer_null("snmp_index"))
            .col(string("name"))
            .col(string_null("description"))
            .col(string_null("alias"))
            .col(string_null("mac_address"))
            .col(string_null("type"))
            // `bigint`: 100 Gbps em bits por segundo já estoura o i32.
            .col(big_integer_null("speed"))
            .col(string_null("admin_status"))
            .col(string_null("oper_status"))
            .col(timestamp_with_time_zone_null("last_seen_at"))
            .foreign_key(&mut fk(
                "device_interfaces",
                "device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // Uma consulta por interface a cada coleta SNMP — o laço mais quente do
        // módulo. Não é UNIQUE porque `snmp_index` é opcional em interfaces
        // criadas manualmente.
        m.create_index(index(
            "device_interfaces_device_id_snmp_index_index",
            "device_interfaces",
            &["device_id", "snmp_index"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("device_interfaces")).await?;
        Ok(())
    }
}
