//! Padrões de log e o que cada um significa.
//!
//! O Decisor classifica **padrões**, não linhas: "login failure for user admin
//! from #" é uma pergunta só, venha ela mil vezes. A linha guarda só a chave
//! do padrão (`device_logs.template_hash`, criada com a tabela); a categoria,
//! a confiança e a correção do operador moram uma vez em `log_templates`.
//!
//! `confidence` é percentual inteiro (`SMALLINT`), não real: igualdade exata
//! na entidade e o mesmo tipo nos dois bancos.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("log_templates");
        stmt.col(big_integer("template_hash").primary_key())
            .col(text("template"))
            // Uma linha real do padrão: o Decisor lê melhor com um exemplo.
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
        Ok(())
    }
}
