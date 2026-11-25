//! §6 #20 — `event_outbox`.
//!
//! Caixa de saída de eventos de domínio. Cada `EventBus::publish` grava uma
//! linha; no PostgreSQL o mesmo commit dispara o `NOTIFY` que leva o evento
//! aos outros processos da central (ADR 024). A linha é o que garante a
//! entrega quando o aviso não cabe nos 8000 bytes do `NOTIFY` (ele leva só o
//! `id`) e o que um processo relê depois de perder a conexão de escuta.
//!
//! `snapshot_key` marca a linha de um snapshot grande demais para o aviso: o
//! processo que a recebe a guarda no cache de snapshots, e não como evento.

use sea_orm_migration::prelude::*;

use crate::schema::*;

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
            .col(json_binary("payload"))
            // Chave do snapshot (`EventBus::publish_snapshot`); nula em evento.
            .col(string_null("snapshot_key"));

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
