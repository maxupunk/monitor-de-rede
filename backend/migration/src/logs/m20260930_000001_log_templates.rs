//! Padrões de log e o que cada um significa.
//!
//! O Laya classifica **padrões**, não linhas: "login failure for user admin
//! from #" é uma pergunta só, venha ela mil vezes. A linha guarda só a chave
//! do padrão (`template_hash`); a categoria, a confiança e a correção do
//! operador moram uma vez em `log_templates`.
//!
//! # `device_logs.template_hash`: nulo, sem default e sem índice
//!
//! É a tabela mais quente do sistema. Coluna nula sem default é mudança só de
//! metadado no PostgreSQL — não reescreve milhões de linhas — e não custa nada
//! no SQLite. Sem índice de propósito: o filtro por categoria já roda dentro da
//! janela de tempo da consulta, e cada índice é mais uma escrita por linha.
//! Linhas antigas ficam nulas; a categoria vale do deploy em diante.
//!
//! `confidence` é percentual inteiro (`SMALLINT`), não real: igualdade exata
//! na entidade e o mesmo tipo nos dois bancos.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("device_logs", "template_hash").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("device_logs"))
                    .add_column(big_integer_null("template_hash"))
                    .to_owned(),
            )
            .await?;
        }

        let mut stmt = table("log_templates");
        stmt.col(big_integer("template_hash").primary_key())
            .col(text("template"))
            // Uma linha real do padrão: o Laya lê melhor com um exemplo.
            .col(text("example"))
            .col(string_null("app_name"))
            .col(string_len_null("category", 32))
            .col(small_integer_null("confidence"))
            .col(string_null("model"))
            .col(timestamp_with_time_zone_null("classified_at"))
            // A correção do operador vale mais que o palpite, e o palpite
            // nunca a sobrescreve.
            .col(string_len_null("user_category", 32))
            .col(timestamp_with_time_zone_null("confirmed_at"))
            .col(timestamp_with_time_zone("first_seen_at"));
        m.create_table(stmt.take()).await?;

        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "CREATE INDEX IF NOT EXISTS log_templates_category_index \
             ON log_templates (category)"
                .to_string(),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("log_templates")).await?;
        let db = m.get_connection();
        if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("device_logs", "template_hash").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("device_logs"))
                    .drop_column(Alias::new("template_hash"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
