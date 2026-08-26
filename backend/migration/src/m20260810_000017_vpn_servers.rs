//! §6 #17 — `vpn_servers`. Instância do WireGuard gerida pelo sistema.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("vpn_servers");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("network_id"))
            .col(string("interface_name").default("wg0").take())
            .col(integer("listen_port").default(51820).take())
            .col(string_null("public_endpoint"))
            .col(string("public_key"))
            // Cifrada em repouso com ENCRYPTION_KEY — ver `services::shared::crypto`.
            // `text` e não `string`: o base64 de nonce+criptograma passa dos 255
            // caracteres com folga.
            .col(text("private_key_encrypted"))
            .col(boolean("allow_peer_to_peer").default(false).take())
            .col(integer("mtu").default(1420).take())
            .col(string_null("dns_servers"))
            .col(boolean("active").default(true).take())
            .col(timestamp_with_time_zone_null("last_synced_at"))
            .foreign_key(&mut fk(
                "vpn_servers",
                "network_id",
                "networks",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("vpn_servers")).await?;
        Ok(())
    }
}
