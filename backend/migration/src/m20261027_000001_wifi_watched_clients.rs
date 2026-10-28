//! Clientes Wi-Fi vigiados: aparelhos que deveriam estar sempre conectados
//! (câmera, IoT, impressora). Só eles geram alerta de Wi-Fi — celular que sai
//! do local não é problema (`docs/monitoramento_wifi_sinais.md`).
//!
//! * `mac` — normalizado (`aa:bb:cc:dd:ee:ff`), único: um aparelho é vigiado
//!   uma vez, em qualquer roteador (roaming conta como conectado);
//! * `last_state` — `connected` | `offline`, nulo enquanto nenhuma leitura
//!   completa o viu ou deu pela falta dele;
//! * `last_seen_at` / `last_device_id` — onde e quando apareceu por último,
//!   base do "fora do Wi-Fi há X min" e do escopo do alerta.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("wifi_watched_clients").await? {
            let mut stmt = table("wifi_watched_clients");
            stmt.col(big_pk_auto("id"))
                .col(string_len("mac", 17))
                .col(string_len("name", 120))
                .col(string_len_null("last_state", 16))
                .col(timestamp_with_time_zone_null("last_seen_at"))
                .col(big_integer_null("last_device_id"))
                .col(big_integer_null("created_by"))
                .foreign_key(&mut fk(
                    "wifi_watched_clients",
                    "last_device_id",
                    "devices",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "wifi_watched_clients",
                    "created_by",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "wifi_watched_clients_mac_unique",
                "wifi_watched_clients",
                &["mac"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("wifi_watched_clients")).await?;
        Ok(())
    }
}
