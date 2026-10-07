//! Plano de backup do próprio NetMonitor (`system_backup_plan`).
//!
//! Antes, cada armazenamento tinha o seu "backup automático das
//! configurações", e o mesmo sistema podia ser copiado com agendas diferentes
//! para lugares diferentes — que a tela não conseguia explicar. Agora o backup
//! do sistema é um plano só, no mesmo formato do de cada banco de dados: o quê
//! → para onde → quando → quantas manter. O armazenamento volta a ser só o
//! lugar.
//!
//! Linha única (`id = 1`). Fica **fora** do backup das configurações, como os
//! armazenamentos: restaurar uma cópia não pode trocar para onde vão as
//! próximas.
//!
//! O plano herda o armazenamento que já tinha o backup ligado (o de cópia mais
//! recente), e as colunas antigas saem de `storage_destinations`.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const OLD_COLUMNS: [&str; 6] = [
    "backup_enabled",
    "backup_interval_hours",
    "backup_retention",
    "last_backup_at",
    "last_backup_status",
    "last_backup_error",
];

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

        let db = m.get_connection();
        let had_plan = m
            .has_column("storage_destinations", "backup_enabled")
            .await?;
        if had_plan {
            // O destino que já recebia a cópia automática: primeiro o que vinha
            // dando certo, depois o de cópia mais recente. A ordem de nulos é
            // explícita porque o PostgreSQL os põe primeiro no `DESC` e o SQLite
            // por último. Sem nenhum ligado, o plano nasce desligado.
            db.execute_unprepared(
                "INSERT INTO system_backup_plan (id, storage_destination_id, backup_enabled, \
                 backup_interval_hours, backup_retention, last_backup_at, last_backup_status, \
                 last_backup_error) \
                 SELECT 1, id, backup_enabled, backup_interval_hours, backup_retention, \
                 last_backup_at, last_backup_status, last_backup_error \
                 FROM storage_destinations WHERE backup_enabled = true \
                 ORDER BY CASE WHEN last_backup_status = 'success' THEN 0 \
                 WHEN last_backup_at IS NOT NULL THEN 1 ELSE 2 END, last_backup_at DESC, id \
                 LIMIT 1",
            )
            .await?;
        }
        db.execute_unprepared(
            "INSERT INTO system_backup_plan (id) \
             SELECT 1 WHERE NOT EXISTS (SELECT 1 FROM system_backup_plan WHERE id = 1)",
        )
        .await?;

        if had_plan {
            // Um `ALTER` por coluna: o SQLite não aceita várias num comando.
            for column in OLD_COLUMNS {
                m.alter_table(
                    Table::alter()
                        .table(Alias::new("storage_destinations"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let columns = [
            boolean("backup_enabled").default(false).take(),
            integer("backup_interval_hours").default(24).take(),
            integer("backup_retention").default(14).take(),
            timestamp_with_time_zone_null("last_backup_at"),
            string_len_null("last_backup_status", 16),
            text_null("last_backup_error"),
        ];
        for mut column in columns {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("storage_destinations"))
                    .add_column(&mut column)
                    .to_owned(),
            )
            .await?;
        }
        m.drop_table(drop("system_backup_plan")).await
    }
}
