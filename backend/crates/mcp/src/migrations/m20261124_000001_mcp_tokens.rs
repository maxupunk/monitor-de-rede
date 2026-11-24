//! Tokens pessoais do servidor MCP (ADR 024).
//!
//! * `mcp_tokens` — credenciais escopadas por usuário (`nmm_*`), armazenadas
//!   apenas como hash SHA-256, com validade obrigatória e restrição opcional a
//!   aplicações de deploy.

use sea_orm_migration::prelude::*;

use netmonitor_core::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("mcp_tokens").await? {
            let mut stmt = table("mcp_tokens");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("user_id"))
                .col(string_len("name", 120))
                .col(string_len("token_hash", 64))
                // Os últimos caracteres, para reconhecer o token na lista.
                .col(string_len("hint", 8))
                // Lista JSON de escopos (`["read", "deploys:read", ...]`).
                .col(json_binary("scopes"))
                // Lista opcional de IDs de `deploy_applications`; `NULL` = sem restrição.
                .col(json_binary_null("allowed_application_ids"))
                .col(timestamp_with_time_zone("expires_at"))
                .col(timestamp_with_time_zone_null("last_used_at"))
                .col(string_len_null("last_used_ip", 64))
                .col(timestamp_with_time_zone_null("revoked_at"))
                .foreign_key(&mut fk(
                    "mcp_tokens",
                    "user_id",
                    "users",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "mcp_tokens_hash_unique",
                "mcp_tokens",
                &["token_hash"],
            ))
            .await?;
            m.create_index(index(
                "mcp_tokens_user_id_index",
                "mcp_tokens",
                &["user_id"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("mcp_tokens")).await?;
        Ok(())
    }
}
