//! §6 #11 — `monitor_results`. Histórico bruto de cada checagem.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("monitor_results");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("monitor_id"))
            .col(big_integer_null("probe_id"))
            .col(string("status"))
            .col(timestamp_with_time_zone("started_at"))
            .col(timestamp_with_time_zone("finished_at"))
            .col(integer("duration_ms"))
            // `double` e não `float`: a §5.3 define `latencyMs` como
            // `Option<f64>`. Guardar como `real` (f32) e ler como f64 injetaria
            // ruído de precisão num número que a tela mostra com casas decimais.
            .col(double_null("latency_ms"))
            .col(text_null("message"))
            .col(json_binary_null("data"))
            .foreign_key(&mut fk(
                "monitor_results",
                "monitor_id",
                "monitors",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "monitor_results",
                "probe_id",
                "probes",
                ForeignKeyAction::SetNull,
            ));

        m.create_table(append_only(stmt.take())).await?;

        // Tabela de maior volume e de escrita quente — só dois índices, ambos
        // pagos por consultas de laço: o histórico por monitor (listagem,
        // sparkline, dashboard DNS) e a purga do `DataPrunerService`.
        // O B-tree ascendente atende o `ORDER BY started_at DESC` lendo ao contrário.
        m.create_index(index(
            "monitor_results_monitor_started_index",
            "monitor_results",
            &["monitor_id", "started_at"],
        ))
        .await?;
        m.create_index(index(
            "monitor_results_created_at_index",
            "monitor_results",
            &["created_at"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("monitor_results")).await?;
        Ok(())
    }
}
