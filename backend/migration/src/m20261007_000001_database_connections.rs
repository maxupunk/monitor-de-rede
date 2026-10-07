//! Bancos de dados de terceiros (MySQL, MariaDB, PostgreSQL) e o histórico dos
//! backups deles.
//!
//! `database_connections` guarda como chegar ao servidor — a senha cifrada com
//! a `ENCRYPTION_KEY` — e a política de backup: quais bancos, para qual
//! armazenamento, de quanto em quanto tempo e quantas cópias manter.
//!
//! `database_backups` é uma linha por banco por execução. É ela que a tela de
//! histórico lista e que a retenção percorre: o arquivo no armazenamento sozinho
//! não diz quantas linhas tinha, quanto demorou nem que avisos o dump deu.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("database_connections").await? {
            let mut stmt = table("database_connections");
            stmt.col(big_pk_auto("id"))
                .col(string_len("name", 120))
                .col(string_len("engine", 16))
                .col(string_len("host", 255))
                .col(integer("port"))
                .col(string_len("username", 128))
                .col(text("password_encrypted"))
                .col(string_len("ssl_mode", 16))
                // Lista de nomes; vazia é "todos os bancos", inclusive os
                // criados depois do cadastro.
                .col(json_binary("databases"))
                .col(big_integer_null("storage_destination_id"))
                .col(boolean("backup_enabled").default(false))
                .col(integer("backup_interval_hours").default(24))
                .col(integer("backup_retention").default(14))
                .col(timestamp_with_time_zone_null("last_backup_at"))
                .col(string_len_null("last_backup_status", 16))
                .col(text_null("last_backup_error"))
                // Apagar o armazenamento não apaga a conexão: ela só fica sem
                // destino, e a tela pede um novo.
                .foreign_key(&mut fk(
                    "database_connections",
                    "storage_destination_id",
                    "storage_destinations",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;

            m.create_index(unique(
                "database_connections_name_unique",
                "database_connections",
                &["name"],
            ))
            .await?;
        }

        if !m.has_table("database_backups").await? {
            let mut stmt = table("database_backups");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("connection_id"))
                .col(string_len("database_name", 128))
                .col(big_integer_null("storage_destination_id"))
                .col(string_len_null("object_key", 512))
                .col(big_integer_null("size_bytes"))
                .col(string_len_null("checksum", 64))
                .col(integer("tables").default(0))
                .col(big_integer("rows").default(0))
                .col(big_integer("duration_ms").default(0))
                .col(string_len("status", 16))
                .col(text_null("error"))
                .col(json_binary("warnings"))
                .col(string_len("trigger", 16))
                .col(timestamp_with_time_zone("started_at"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .foreign_key(&mut fk(
                    "database_backups",
                    "connection_id",
                    "database_connections",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "database_backups",
                    "storage_destination_id",
                    "storage_destinations",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(append_only(stmt.take())).await?;

            // O histórico é sempre "desta conexão, mais recentes primeiro".
            m.create_index(index(
                "database_backups_connection_started_index",
                "database_backups",
                &["connection_id", "started_at"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("database_backups")).await?;
        m.drop_table(drop("database_connections")).await
    }
}
