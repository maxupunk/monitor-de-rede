//! Histórico de métricas de host e de containers em baldes de 1 minuto.
//!
//! O agente (e o coletor local da central) agregam as amostras de ~10 s e só
//! o minuto fechado chega aqui. A chave do host é texto (`local`,
//! `agent-<id>`) e não FK: o host local não é linha de `probes`, e um `UNIQUE`
//! com coluna nula se comporta diferente em SQLite e PostgreSQL.
//!
//! O container é identificado pelo **nome**, não pelo id: o id muda a cada
//! recreate e a série histórica de um serviço não pode quebrar por isso.
//!
//! Retenção: `METRICS_RETENTION_DAYS` (30 por padrão), aplicada pelo ciclo de
//! manutenção do scheduler usando o índice por `bucket_at`.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("host_metrics_1m").await? {
            let mut stmt = table("host_metrics_1m");
            stmt.col(big_pk_auto("id"))
                .col(string_len("host_key", 64))
                .col(timestamp_with_time_zone("bucket_at"))
                .col(integer("samples"))
                .col(double("cpu_avg"))
                .col(double("cpu_max"))
                .col(double("memory_used_avg"))
                .col(big_integer("memory_total"))
                .col(double("load1"))
                .col(double("net_rx_bps"))
                .col(double("net_tx_bps"))
                .col(json_binary("disks"));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(unique(
                "host_metrics_1m_host_bucket_unique",
                "host_metrics_1m",
                &["host_key", "bucket_at"],
            ))
            .await?;
            m.create_index(index(
                "host_metrics_1m_bucket_index",
                "host_metrics_1m",
                &["bucket_at"],
            ))
            .await?;
        }

        if !m.has_table("container_metrics_1m").await? {
            let mut stmt = table("container_metrics_1m");
            stmt.col(big_pk_auto("id"))
                .col(string_len("host_key", 64))
                .col(string_len("container_name", 255))
                .col(string_len_null("project", 255))
                .col(timestamp_with_time_zone("bucket_at"))
                .col(double("cpu_avg"))
                .col(double("cpu_max"))
                .col(double("memory_avg"))
                .col(big_integer("memory_max"))
                .col(big_integer("net_rx_bytes"))
                .col(big_integer("net_tx_bytes"))
                .col(big_integer("block_read_bytes"))
                .col(big_integer("block_write_bytes"));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(unique(
                "container_metrics_1m_host_name_bucket_unique",
                "container_metrics_1m",
                &["host_key", "container_name", "bucket_at"],
            ))
            .await?;
            // "Os containers deste host nesta janela" — a consulta da tela.
            m.create_index(index(
                "container_metrics_1m_host_bucket_index",
                "container_metrics_1m",
                &["host_key", "bucket_at"],
            ))
            .await?;
            m.create_index(index(
                "container_metrics_1m_bucket_index",
                "container_metrics_1m",
                &["bucket_at"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("container_metrics_1m")).await?;
        m.drop_table(drop("host_metrics_1m")).await?;
        Ok(())
    }
}
