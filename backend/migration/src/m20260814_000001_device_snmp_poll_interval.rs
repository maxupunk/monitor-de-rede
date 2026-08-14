//! Centraliza a cadência de coleta SNMP no dispositivo.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const DEFAULT_SNMP_POLL_INTERVAL_SECONDS: i32 = 15;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m
            .has_column("devices", "snmp_poll_interval_seconds")
            .await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .add_column(
                        integer("snmp_poll_interval_seconds")
                            .not_null()
                            .default(DEFAULT_SNMP_POLL_INTERVAL_SECONDS),
                    )
                    .to_owned(),
            )
            .await?;
        }

        let db = m.get_connection();
        let backend = db.get_database_backend();
        // A menor cadência existente preserva a observação mais frequente que o
        // operador já tinha configurado. Em seguida todos os monitores SNMP do
        // dispositivo passam a compartilhar esse valor canônico.
        db.execute_raw(Statement::from_string(
            backend,
            format!(
                "UPDATE devices \
                 SET snmp_poll_interval_seconds = COALESCE((\
                    SELECT MIN(interval_seconds) FROM monitors \
                    WHERE monitors.device_id = devices.id AND monitors.type = 'snmp'\
                 ), {DEFAULT_SNMP_POLL_INTERVAL_SECONDS})"
            ),
        ))
        .await?;
        db.execute_raw(Statement::from_string(
            backend,
            "UPDATE monitors \
             SET interval_seconds = (\
                SELECT snmp_poll_interval_seconds FROM devices \
                WHERE devices.id = monitors.device_id\
             ) \
             WHERE type = 'snmp' AND device_id IS NOT NULL"
                .to_string(),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // O SQLite de produção suporta ADD COLUMN, mas não oferece um DROP
        // COLUMN compatível com todas as versões que ainda atendemos.
        if m.get_connection().get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("devices", "snmp_poll_interval_seconds")
                .await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("devices"))
                    .drop_column(Alias::new("snmp_poll_interval_seconds"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
