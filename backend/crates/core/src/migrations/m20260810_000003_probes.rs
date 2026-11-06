//! §6 #03 — `probes`. Agentes que executam checagens fora do alcance do
//! servidor central.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("probes");
        stmt.col(big_pk_auto("id"))
            .col(big_integer_null("site_id"))
            .col(string("name"))
            .col(string("token_hash"))
            .col(string("status").default("pending").take())
            .col(string_null("version"))
            .col(timestamp_with_time_zone_null("last_seen_at"))
            .col(timestamp_with_time_zone_null("registered_at"))
            .col(timestamp_with_time_zone_null("revoked_at"))
            .col(json_binary_null("configuration"))
            // Anulável **com CASCADE**: apagar o site apaga os probes dele. É a
            // combinação que o helper `refs` do Loco não sabe expressar.
            .foreign_key(&mut fk(
                "probes",
                "site_id",
                "sites",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        // Autentica todo `POST /heartbeat`, `GET /tasks` e `POST /results`. Não é
        // UNIQUE de propósito: o `DEFAULT_VPN_PROBE_TOKEN` permite que mais de um
        // agente zero-config compartilhe o mesmo token.
        m.create_index(index("probes_token_hash_index", "probes", &["token_hash"]))
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("probes")).await?;
        Ok(())
    }
}
