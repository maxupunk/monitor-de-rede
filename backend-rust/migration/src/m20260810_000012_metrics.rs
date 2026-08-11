//! §6 #12 — `metrics`. Série temporal de valores coletados.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("metrics");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("device_id"))
            .col(big_integer_null("interface_id"))
            .col(big_integer_null("monitor_id"))
            .col(string("name"))
            .col(double("value"))
            .col(string("unit"))
            .col(timestamp_with_time_zone("recorded_at"))
            .foreign_key(&mut fk(
                "metrics",
                "device_id",
                "devices",
                ForeignKeyAction::Cascade,
            ))
            // Interface ou monitor removido não apaga o histórico: a série
            // continua valendo para o equipamento.
            .foreign_key(&mut fk(
                "metrics",
                "interface_id",
                "device_interfaces",
                ForeignKeyAction::SetNull,
            ))
            .foreign_key(&mut fk(
                "metrics",
                "monitor_id",
                "monitors",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(append_only(stmt.take())).await?;

        // Série temporal com inserção em rajada: cada índice custa em toda coleta
        // SNMP, então são só os quatro que atendem laços quentes.
        //
        // O primeiro serve o "último valor por interface" (tráfego SNMP) e, pelo
        // prefixo `device_id`, também os filtros por equipamento. O segundo serve
        // o "último valor por métrica" (bytes da VPN, sparkline de CPU/memória).
        m.create_index(index(
            "metrics_device_interface_name_recorded_index",
            "metrics",
            &["device_id", "interface_id", "name", "recorded_at"],
        ))
        .await?;
        m.create_index(index(
            "metrics_device_name_recorded_index",
            "metrics",
            &["device_id", "name", "recorded_at"],
        ))
        .await?;
        m.create_index(index(
            "metrics_interface_recorded_index",
            "metrics",
            &["interface_id", "recorded_at"],
        ))
        .await?;
        m.create_index(index(
            "metrics_created_at_index",
            "metrics",
            &["created_at"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("metrics")).await?;
        Ok(())
    }
}
