//! Plano de backup do próprio NetMonitor (`system_backup_plan`).
//!
//! O backup do sistema é um plano só, no mesmo formato do de cada banco de
//! dados: o quê → para onde → quando → quantas manter. Um backup por
//! armazenamento deixaria o mesmo sistema ser copiado com agendas diferentes
//! para lugares diferentes — que a tela não conseguiria explicar. O
//! armazenamento é só o lugar.
//!
//! Linha única (`id = 1`), criada aqui, desligada. Fica **fora** do backup das
//! configurações, como os armazenamentos: restaurar uma cópia não pode trocar
//! para onde vão as próximas.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("system_backup_plan").await? {
            let mut stmt = table("system_backup_plan");
            stmt.col(big_pk_auto("id"))
                .col(big_integer_null("storage_destination_id"))
                .col(boolean("backup_enabled").default(false))
                .col(integer("backup_interval_hours").default(24))
                .col(integer("backup_retention").default(14))
                .col(timestamp_with_time_zone_null("last_backup_at"))
                .col(string_len_null("last_backup_status", 16))
                .col(text_null("last_backup_error"))
                .foreign_key(&mut fk(
                    "system_backup_plan",
                    "storage_destination_id",
                    "storage_destinations",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
        }

        m.get_connection()
            .execute_unprepared(
                "INSERT INTO system_backup_plan (id) \
                 SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM system_backup_plan WHERE id = 1)",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("system_backup_plan")).await
    }
}
