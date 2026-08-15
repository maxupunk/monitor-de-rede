//! Fase 4 do roadmap de alertas inteligentes — higiene de notificações.
//!
//! Três coisas nascem aqui, e as três são a mesma ideia: **a decisão de
//! notificar deixa de ser um efeito colateral do motor de alertas e passa a ser
//! um registro**.
//!
//! - `alert_rules.notification_cooldown_seconds`: intervalo mínimo entre
//!   notificações de problema do mesmo par (regra, alvo), **mesmo quando o
//!   evento fecha e reabre**. `0` desliga — é o default, para que instalações
//!   existentes não emudeçam sozinhas. O catálogo é o veículo dos valores
//!   sensatos (§3.4 do roadmap).
//! - `alert_rules.inhibit_when_parent_down`: quando o dispositivo tem pai
//!   declarado (`devices.parent_id`) e o pai está em alerta, o alerta do filho
//!   não notifica. Default `false` pelo mesmo motivo: parar de avisar alguém é
//!   o lado perigoso do erro, então quem liga é o catálogo ou o operador.
//! - `notification_outbox`: o diário de notificações. Toda decisão — entregue,
//!   represada no digest ou suprimida, e por quê — vira linha. É o que sustenta
//!   o cooldown (que precisa saber quando foi a última entrega), o digest (que
//!   precisa de uma fila) e a entrega ao menos uma vez (que precisa sobreviver
//!   a um crash entre o INSERT do alerta e o envio).

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m
            .has_column("alert_rules", "notification_cooldown_seconds")
            .await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("alert_rules"))
                    .add_column(
                        integer("notification_cooldown_seconds")
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        }
        if !m
            .has_column("alert_rules", "inhibit_when_parent_down")
            .await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("alert_rules"))
                    .add_column(
                        boolean("inhibit_when_parent_down")
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;
        }

        if !m.has_table("notification_outbox").await? {
            let mut stmt = table("notification_outbox");
            stmt.col(big_pk_auto("id"))
                // De qual regra e de qual alvo — o par que o cooldown mede.
                .col(big_integer_null("alert_rule_id"))
                .col(string_null("scope_key"))
                // Guardado na linha para a inibição ser reavaliada na hora da
                // entrega, e não só quando a mensagem foi enfileirada: o pai
                // costuma ser detectado *depois* do filho.
                .col(big_integer_null("device_id"))
                // Chave de correlação do digest: `site:3`, `device:12`, `global`.
                .col(string("group_key"))
                // `problem` | `flapping` | `resolved`.
                .col(string("kind"))
                .col(text("title"))
                .col(text("body"))
                .col(string("severity"))
                .col(json_binary("metadata"))
                // `pending` | `sent` | `suppressed`.
                .col(string("status"))
                // `cooldown` | `inhibited` | `unannounced` | `silenced`.
                .col(string_null("suppress_reason"))
                // A regra manda suprimir quando o pai está em alerta: a linha
                // espera a carência antes de ser julgada.
                .col(boolean("inhibitable").not_null().default(false))
                // Antes disto a linha não sai: é o que o digest usa para
                // acumular a rajada numa mensagem só.
                .col(timestamp_with_time_zone("deliver_after"))
                .col(timestamp_with_time_zone_null("sent_at"))
                // Regra apagada não leva o histórico de notificação junto: o
                // registro é de que a mensagem foi (ou não foi) entregue.
                .foreign_key(&mut fk(
                    "notification_outbox",
                    "alert_rule_id",
                    "alert_rules",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "notification_outbox",
                    "device_id",
                    "devices",
                    ForeignKeyAction::SetNull,
                ));

            m.create_table(append_only(stmt.take())).await?;

            // A varredura do despachante: o que está pendente e já venceu.
            m.create_index(index(
                "notification_outbox_status_deliver_index",
                "notification_outbox",
                &["status", "deliver_after"],
            ))
            .await?;
            // O cooldown: última entrega do par (regra, alvo).
            m.create_index(index(
                "notification_outbox_rule_scope_sent_index",
                "notification_outbox",
                &["alert_rule_id", "scope_key", "sent_at"],
            ))
            .await?;
            // O digest: última entrega do grupo correlato.
            m.create_index(index(
                "notification_outbox_group_sent_index",
                "notification_outbox",
                &["group_key", "sent_at"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("notification_outbox")).await?;
        // Mesmo motivo do m20260815_000001: o SQLite de produção suporta ADD
        // COLUMN, mas não um DROP COLUMN compatível com todas as versões que
        // ainda atendemos.
        if m.get_connection().get_database_backend() == sea_orm::DatabaseBackend::Sqlite {
            return Ok(());
        }
        for column in ["inhibit_when_parent_down", "notification_cooldown_seconds"] {
            if m.has_column("alert_rules", column).await? {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("alert_rules"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }
}
