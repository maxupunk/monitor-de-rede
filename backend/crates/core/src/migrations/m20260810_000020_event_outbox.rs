//! §6 #20 — `event_outbox`.
//!
//! Caixa de saída de eventos: ponte entre os processos que produzem eventos
//! (scheduler, worker, probes) e o processo HTTP que mantém as conexões SSE.
//! O EventBus é um `broadcast` em memória, então sem esta tabela nada que roda
//! em background chega ao navegador. É consequência direta do ADR 005, que põe
//! o scheduler num processo separado.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("event_outbox");
        stmt.col(big_pk_auto("id"))
            .col(string("type"))
            // Identifica o processo emissor para que ele não reprocesse o próprio evento
            .col(string("origin"))
            .col(json_binary("payload"));

        m.create_table(append_only(stmt.take())).await?;

        m.create_index(index(
            "event_outbox_created_at_index",
            "event_outbox",
            &["created_at"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("event_outbox")).await?;
        Ok(())
    }
}
