//! §6 #18 — `vpn_peers`. Um túnel por dispositivo.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("vpn_peers");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("vpn_server_id"))
            .col(big_integer("device_id"))
            // `string("public_key")` + índice único nomeado abaixo, e não
            // `string_uniq`: o UNIQUE inline deixa o Postgres batizar a
            // constraint (`vpn_peers_public_key_key`), e a §6 exige o nome do
            // Adonis. Achado pelo `cargo run --example schema_parity`.
            .col(string("public_key"))
            // Cifrada em repouso com a APP_KEY — ver `services::shared::crypto`.
            .col(text_null("preshared_key_encrypted"))
            .col(string("device_profile").default("linux").take())
            .col(integer("persistent_keepalive").default(25).take())
            .col(timestamp_with_time_zone_null("last_handshake_at"))
            // Último ciclo em que o servidor contabilizou bytes novos vindos do peer
            // — na prática, o último keepalive recebido. O handshake sozinho não
            // serve como sinal de vida: o WireGuard só renegocia chaves quando há o
            // que enviar, então um túnel ocioso mas saudável passa vários minutos sem
            // handshake novo.
            .col(timestamp_with_time_zone_null("last_seen_at"))
            // `bigint`: contador de octetos de um túnel estoura i32 em horas.
            // A §5.3 exige que saiam como número JSON, não string.
            .col(big_integer("bytes_rx").default(0).take())
            .col(big_integer("bytes_tx").default(0).take())
            .col(boolean("enabled").default(true).take())
            // Memória do ciclo anterior do túnel: alerta nasce de uma *transição*
            // (`connected ➔ disconnected`), e transição exige saber onde o túnel
            // estava antes.
            .col(string_null("last_connection_status"))
            .foreign_key(&mut fk(
                "vpn_peers",
                "vpn_server_id",
                "vpn_servers",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "vpn_peers",
                "device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // A chave pública identifica o peer no protocolo: duas linhas com a
        // mesma chave fariam o WireGuard aceitar o túnel errado.
        m.create_index(unique(
            "vpn_peers_public_key_unique",
            "vpn_peers",
            &["public_key"],
        ))
        .await?;

        // Um túnel por dispositivo — é o que permite tratar `device.vpnPeer`
        // como relação 1:1 na serialização.
        m.create_index(unique(
            "vpn_peers_device_id_unique",
            "vpn_peers",
            &["device_id"],
        ))
        .await?;

        // Peers ativos de um servidor: lido a cada ciclo de sincronia do WireGuard.
        m.create_index(index(
            "vpn_peers_server_enabled_index",
            "vpn_peers",
            &["vpn_server_id", "enabled"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("vpn_peers")).await?;
        Ok(())
    }
}
