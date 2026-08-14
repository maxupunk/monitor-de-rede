//! §6 #15 — `alert_rules`. Regras do motor de alertas.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("alert_rules");
        stmt.col(big_pk_auto("id"))
            .col(big_integer_null("site_id"))
            .col(big_integer_null("device_id"))
            .col(big_integer_null("monitor_id"))
            .col(string("name"))
            .col(string("type"))
            // `template_key` liga a regra ao item do catálogo que a originou. É a
            // chave de idempotência usada ao aplicar as regras pré-configuradas: uma
            // regra já derivada de um template nunca é recriada.
            .col(string_null("template_key"))
            .col(json_binary("condition"))
            .col(string("severity"))
            .col(integer("duration_seconds").default(0).take())
            .col(boolean("enabled").default(true).take())
            // As três anuláveis com CASCADE: uma regra com escopo definido não
            // sobrevive ao desaparecimento do escopo. Escopo nulo = regra global.
            .foreign_key(&mut fk(
                "alert_rules",
                "site_id",
                "sites",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "alert_rules",
                "device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "alert_rules",
                "monitor_id",
                "monitors",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(with_timestamps(stmt.take())).await?;

        m.create_index(index(
            "alert_rules_template_key_index",
            "alert_rules",
            &["template_key"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("alert_rules")).await?;
        Ok(())
    }
}
