//! §6 #15 — `alert_rules`. Regras do motor de alertas.
//!
//! Os parâmetros dos alertas inteligentes ficam depois dos timestamps, na
//! ordem em que chegaram. São parâmetros de **regra** (cada escopo tolera o que
//! fizer sentido para ele), e os defaults deixam tudo desligado — os valores
//! sensatos vêm do catálogo:
//!
//! * `recovery_window_seconds` — quanto tempo o alvo precisa ficar estável,
//!   contado do último problema, antes de o alerta fechar. `0` resolve na
//!   primeira checagem ok.
//! * `flap_threshold`/`flap_window_seconds` — quantas recaídas dentro da
//!   janela declaram o alvo oscilando. `0` desliga a detecção.
//! * `notification_cooldown_seconds` — intervalo mínimo entre notificações de
//!   problema do mesmo par (regra, alvo), mesmo quando o evento fecha e reabre.
//! * `inhibit_when_parent_down` — com o pai (`devices.parent_id`) em alerta, o
//!   alerta do filho não notifica. Parar de avisar é o lado perigoso do erro,
//!   então quem liga é o catálogo ou o operador.

use sea_orm_migration::prelude::*;

use crate::schema::*;

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

        let mut stmt = with_timestamps(stmt.take());
        stmt.col(integer("recovery_window_seconds").not_null().default(0))
            .col(integer("flap_threshold").not_null().default(0))
            .col(integer("flap_window_seconds").not_null().default(900))
            .col(
                integer("notification_cooldown_seconds")
                    .not_null()
                    .default(0),
            )
            .col(
                boolean("inhibit_when_parent_down")
                    .not_null()
                    .default(false),
            );

        m.create_table(stmt).await?;

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
