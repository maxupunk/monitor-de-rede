//! Conversas do Assistente IA, por usuário.
//!
//! Antes viviam no `localStorage` do navegador: trocar de computador era
//! perder o histórico. As mensagens ficam num JSON só — a conversa é lida e
//! gravada inteira, nunca consultada por dentro —, e `message_count` existe
//! para a listagem não precisar carregar esse JSON.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("ai_conversations").await? {
            let mut stmt = table("ai_conversations");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("user_id"))
                .col(string_len("title", 160))
                .col(json_binary("messages"))
                .col(integer("message_count").default(0))
                .foreign_key(&mut fk(
                    "ai_conversations",
                    "user_id",
                    "users",
                    ForeignKeyAction::Cascade,
                ));

            m.create_table(with_timestamps(stmt.take())).await?;

            // A listagem é sempre "as conversas deste usuário, mais recentes
            // primeiro".
            m.create_index(index(
                "ai_conversations_user_updated_index",
                "ai_conversations",
                &["user_id", "updated_at"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("ai_conversations")).await?;
        Ok(())
    }
}
