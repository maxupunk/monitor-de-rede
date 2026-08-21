//! Fase 3 do roadmap — trilha de auditoria.
//!
//! Registra quem alterou cada recurso (usuário, timestamp, mudança) e eventos de
//! autenticação. A tabela é append-only e consultada por administradores.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("audit_logs").await? {
            let mut stmt = table("audit_logs");
            stmt.col(big_pk_auto("id"))
                .col(big_integer_null("user_id"))
                .col(string_len("action", 32))
                .col(string_len_null("resource_type", 64))
                .col(big_integer_null("resource_id"))
                .col(string_len_null("resource_label", 255))
                .col(text_null("description"))
                .col(json_binary_null("changes"))
                .col(string_len_null("ip_address", 64))
                .col(string_len_null("user_agent", 512))
                .foreign_key(&mut fk(
                    "audit_logs",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));

            m.create_table(append_only(stmt.take())).await?;

            m.create_index(index(
                "audit_logs_created_at_index",
                "audit_logs",
                &["created_at"],
            ))
            .await?;
            m.create_index(index("audit_logs_user_index", "audit_logs", &["user_id"]))
                .await?;
            m.create_index(index(
                "audit_logs_resource_index",
                "audit_logs",
                &["resource_type", "resource_id"],
            ))
            .await?;
            m.create_index(index("audit_logs_action_index", "audit_logs", &["action"]))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("audit_logs")).await?;
        Ok(())
    }
}
