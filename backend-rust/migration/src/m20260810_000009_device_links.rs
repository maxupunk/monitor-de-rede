//! §6 #09 — `device_links`. Enlaces da topologia (LLDP/CDP, inferência, manual).

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("device_links");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("source_device_id"))
            .col(big_integer("target_device_id"))
            .col(big_integer_null("source_interface_id"))
            .col(big_integer_null("target_interface_id"))
            .col(string("link_type"))
            .col(string("discovery_method"))
            .col(integer("confidence").default(100).take())
            .col(boolean("confirmed").default(false).take())
            .col(timestamp_with_time_zone_null("last_seen_at"))
            .foreign_key(&mut fk(
                "device_links",
                "source_device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "device_links",
                "target_device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ))
            // Interface some, enlace fica: o vínculo entre os dois equipamentos
            // continua verdadeiro mesmo sem saber por qual porta.
            .foreign_key(&mut fk(
                "device_links",
                "source_interface_id",
                "device_interfaces",
                ForeignKeyAction::SetNull,
            ))
            .foreign_key(&mut fk(
                "device_links",
                "target_interface_id",
                "device_interfaces",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // O enlace é procurado nos dois sentidos (`source→target` e o inverso),
        // daí o índice composto mais o índice isolado no destino.
        m.create_index(index(
            "device_links_source_target_index",
            "device_links",
            &["source_device_id", "target_device_id"],
        ))
        .await?;
        m.create_index(index(
            "device_links_target_device_id_index",
            "device_links",
            &["target_device_id"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("device_links")).await?;
        Ok(())
    }
}
