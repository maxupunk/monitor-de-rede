//! §6 #10 — `monitors`. A unidade de checagem.
//!
//! `interface_id` (depois dos timestamps, porque chegou depois) liga o monitor
//! de tráfego à linha de `device_interfaces` pelo `id`, não pelo nome
//! `Interface {ifName}`: pelo nome, homônimas dividiam um monitor, renomear a
//! porta o deixava órfão e a limpeza de interfaces sumidas apagava o monitor
//! errado. Monitores do dispositivo inteiro (CPU, ping, TCP) ficam com ele nulo.
//! Sem FK, como as demais referências a `device_interfaces`.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("monitors");
        stmt.col(big_pk_auto("id"))
            // Nem toda checagem pertence a um equipamento: medir a latência de um
            // servidor DNS público ou a disponibilidade de um site externo não
            // depende de um dispositivo cadastrado.
            .col(big_integer_null("device_id"))
            .col(big_integer_null("probe_id"))
            .col(string("type"))
            .col(string("name"))
            .col(json_binary("configuration"))
            .col(integer("interval_seconds").default(15).take())
            .col(integer("timeout_seconds").default(10).take())
            .col(integer("retry_count").default(3).take())
            .col(boolean("enabled").default(true).take())
            .col(timestamp_with_time_zone_null("next_run_at"))
            .col(timestamp_with_time_zone_null("last_run_at"))
            .col(string("status").default("unknown").take())
            .foreign_key(&mut fk(
                "monitors",
                "device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ))
            // Probe removido não pode levar o monitor junto: ele volta a rodar
            // localmente (o fallback do §9.2).
            .foreign_key(&mut fk(
                "monitors",
                "probe_id",
                "probes",
                ForeignKeyAction::SetNull,
            ));

        let mut stmt = with_timestamps(stmt.take());
        stmt.col(big_integer_null("interface_id"));

        m.create_table(stmt).await?;

        // Seleção dos monitores vencidos: o laço central do `scheduler_run`.
        m.create_index(index(
            "monitors_enabled_next_run_at_index",
            "monitors",
            &["enabled", "next_run_at"],
        ))
        .await?;
        m.create_index(index(
            "monitors_device_id_enabled_index",
            "monitors",
            &["device_id", "enabled"],
        ))
        .await?;

        // Um único monitor **gerenciado** (`system_health`) por dispositivo:
        // dois boots simultâneos podem passar juntos pelo `SELECT` do
        // provisionamento, e quem decide a corrida é o banco. O índice é
        // parcial porque a coleta SNMP cria dois monitores `snmp` no mesmo
        // equipamento (CPU e memória). `CREATE UNIQUE INDEX ... WHERE` tem a
        // mesma sintaxe no SQLite e no PostgreSQL, mas o `IndexCreate` do
        // SeaORM não o expressa — daí o SQL cru.
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "CREATE UNIQUE INDEX IF NOT EXISTS monitors_managed_unique \
             ON monitors (device_id, type) WHERE type = 'system_health'"
                .to_string(),
        ))
        .await?;
        m.create_index(index(
            "monitors_interface_id_index",
            "monitors",
            &["interface_id"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("monitors")).await?;
        Ok(())
    }
}
