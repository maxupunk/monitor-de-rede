//! Histórico de roaming: cada vez que um aparelho troca de AP.
//!
//! * `mac` — normalizado (`aa:bb:cc:dd:ee:ff`);
//! * `from_device_id` / `to_device_id` — os APs, anulados se o equipamento
//!   for apagado; o nome fica gravado (`*_device_name`) para o histórico
//!   continuar legível;
//! * `signal_before_dbm` / `signal_after_dbm` — o sinal no AP antigo (última
//!   leitura lá) e no novo;
//! * `ping_pong` — esta troca fechou o ping-pong (trocas demais na janela);
//! * `created_at` — quando a troca foi vista; a retenção apaga por ele.

use sea_orm_migration::prelude::*;

use netmonitor_core::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("wifi_roaming_events").await? {
            let mut stmt = table("wifi_roaming_events");
            stmt.col(big_pk_auto("id"))
                .col(string_len("mac", 17))
                .col(big_integer_null("from_device_id"))
                .col(string_len("from_device_name", 120))
                .col(big_integer_null("to_device_id"))
                .col(string_len("to_device_name", 120))
                .col(integer_null("signal_before_dbm"))
                .col(integer_null("signal_after_dbm"))
                .col(boolean("ping_pong").default(false).take())
                .foreign_key(&mut fk(
                    "wifi_roaming_events",
                    "from_device_id",
                    "devices",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "wifi_roaming_events",
                    "to_device_id",
                    "devices",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(append_only(stmt.take())).await?;
            // O histórico de um aparelho (painel) e a contagem do ping-pong.
            m.create_index(index(
                "wifi_roaming_events_mac_created_at_idx",
                "wifi_roaming_events",
                &["mac", "created_at"],
            ))
            .await?;
            // A retenção.
            m.create_index(index(
                "wifi_roaming_events_created_at_idx",
                "wifi_roaming_events",
                &["created_at"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("wifi_roaming_events")).await?;
        Ok(())
    }
}
