//! Subscrições de navegadores para Web Push (PWA).
//!
//! Guarda os endpoints e chaves criptográficas (p256dh, auth) de cada navegador/dispositivo
//! cadastrado para recebimento de notificações mesmo em segundo plano ou com o app fechado.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("push_subscriptions").await? {
            let mut stmt = table("push_subscriptions");
            stmt.col(big_pk_auto("id"))
                .col(big_integer_null("user_id"))
                .col(text("endpoint"))
                .col(text("p256dh"))
                .col(text("auth"))
                .col(string_len_null("user_agent", 512))
                .foreign_key(&mut fk(
                    "push_subscriptions",
                    "user_id",
                    "users",
                    ForeignKeyAction::Cascade,
                ));

            m.create_table(with_timestamps(stmt.take())).await?;

            m.create_index(unique(
                "push_subscriptions_endpoint_unique",
                "push_subscriptions",
                &["endpoint"],
            ))
            .await?;
            m.create_index(index(
                "push_subscriptions_user_id_index",
                "push_subscriptions",
                &["user_id"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("push_subscriptions")).await?;
        Ok(())
    }
}
