//! §6 #16 — `alert_events`. Ciclo de vida de cada alerta disparado.

use sea_orm_migration::prelude::*;

use crate::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("alert_events");
        stmt.col(big_pk_auto("id"))
            .col(big_integer_null("alert_rule_id"))
            .col(big_integer_null("device_id"))
            .col(big_integer_null("monitor_id"))
            // `scope_key` identifica o alvo concreto do alerta (`monitor:12`,
            // `interface:34`, ...). Sem ele não é possível deduplicar nem normalizar
            // alertas de alvos que não são monitores — duas interfaces do mesmo
            // dispositivo colapsariam no mesmo evento.
            .col(string_null("scope_key"))
            .col(string("status"))
            .col(string("severity"))
            .col(timestamp_with_time_zone("started_at"))
            .col(timestamp_with_time_zone_null("resolved_at"))
            .col(text_null("message"))
            .col(json_binary_null("data"))
            // Regra apagada leva o histórico dela junto; device/monitor apagados
            // deixam o evento no lugar, porque ele é o registro de que aquilo
            // aconteceu.
            .foreign_key(&mut fk(
                "alert_events",
                "alert_rule_id",
                "alert_rules",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "alert_events",
                "device_id",
                "devices",
                ForeignKeyAction::SetNull,
            ))
            .foreign_key(&mut fk(
                "alert_events",
                "monitor_id",
                "monitors",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        m.create_index(index(
            "alert_events_scope_key_index",
            "alert_events",
            &["scope_key"],
        ))
        .await?;

        // Deduplicação: procura o evento aberto da regra para aquele alvo a cada
        // resultado de monitor processado.
        m.create_index(index(
            "alert_events_rule_scope_status_index",
            "alert_events",
            &["alert_rule_id", "scope_key", "status"],
        ))
        .await?;
        // Um alerta aberto por (regra, alvo), garantido pelo banco: a leitura
        // de deduplicação do `trigger_alert` não basta no PostgreSQL, onde
        // duas transações concorrentes leem "nenhum aberto" e inserem ambas.
        // Índice parcial (o `sea-query` não o monta) — SQL igual nos dois
        // dialetos. A lista de status é `AlertStatus::OPEN`.
        m.get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX alert_events_open_rule_scope_unique \
                 ON alert_events (alert_rule_id, scope_key) \
                 WHERE alert_rule_id IS NOT NULL AND scope_key IS NOT NULL \
                 AND status IN ('active', 'acknowledged', 'silenced', 'recovering', 'flapping')",
            )
            .await?;
        m.create_index(index(
            "alert_events_device_created_index",
            "alert_events",
            &["device_id", "created_at"],
        ))
        .await?;
        m.create_index(index(
            "alert_events_monitor_created_index",
            "alert_events",
            &["monitor_id", "created_at"],
        ))
        .await?;
        m.create_index(index(
            "alert_events_created_at_index",
            "alert_events",
            &["created_at"],
        ))
        .await?;
        m.create_index(index(
            "alert_events_status_started_index",
            "alert_events",
            &["status", "started_at"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("alert_events")).await?;
        Ok(())
    }
}
