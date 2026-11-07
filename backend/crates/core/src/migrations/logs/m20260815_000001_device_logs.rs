//! `device_logs` — o único conteúdo do banco de logs.
//!
//! Append-only: só `created_at`/`received_at`, sem `updated_at`. Linha de log
//! nunca é editada, e 8 bytes que ninguém lê multiplicados por milhões de
//! linhas são banda de escrita jogada fora — o mesmo raciocínio de
//! `monitor_results` e `metrics`.
//!
//! **`device_id` não tem FK**: `devices` mora no banco principal e o
//! SQLite não referencia outro arquivo. Apagar um dispositivo não cascateia
//! aqui; as linhas órfãs saem pela retenção normal do `data_pruner`, e a
//! hidratação do nome é feita pelo serviço, em duas consultas.
//!
//! `source` diz de onde a linha veio: `syslog` (rede) ou `application` (o
//! próprio processo). Uma coluna curta e indexável, e não um JSON de contexto:
//! seria uma segunda superfície de busca invisível ao índice de texto, que lê
//! só `message` — os campos do evento do `tracing` são achatados na mensagem.
//!
//! `template_hash` é a chave do padrão da linha em `log_templates` (o Decisor
//! classifica padrões, não linhas). Nula e **sem índice** de propósito: o
//! filtro por categoria já roda dentro da janela de tempo da consulta, e cada
//! índice é mais uma escrita por linha na tabela mais quente do sistema.
//!
//! As duas ficam depois de `created_at` porque chegaram depois.

use sea_orm_migration::prelude::*;

use crate::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("device_logs");
        stmt.col(big_pk_auto("id"))
            .col(big_integer_null("device_id"))
            // 45 caracteres é o pior caso do IPv6 (`::ffff:` + IPv4 mapeado).
            .col(string_len("source_ip", 45))
            // A verdade do sistema. O relógio do dispositivo é palpite: o
            // RFC 3164 vem sem ano e sem fuso, e roteador com NTP quebrado
            // manda data de 1970.
            .col(timestamp_with_time_zone("received_at"))
            .col(timestamp_with_time_zone_null("device_time"))
            // `smallint`: facility vai a 23, severity a 7. Cabe em 2 bytes.
            .col(small_integer_null("facility"))
            .col(small_integer_null("severity"))
            .col(string_null("hostname"))
            .col(string_null("app_name"))
            .col(integer_null("pid"))
            // Tópicos do RouterOS (`system,info,account`) como texto
            // vírgula-separado, não JSON: `LIKE '%firewall%'` vale no SQLite e
            // no PostgreSQL, operador de JSON não vale nos dois.
            .col(string_null("topics"))
            .col(text("message"));

        let mut stmt = append_only(stmt.take());
        stmt.col(string_len("source", 16).default("syslog").not_null())
            .col(big_integer_null("template_hash"));

        m.create_table(stmt).await?;

        // O laço da aba de logs do dispositivo.
        m.create_index(index(
            "device_logs_device_received_index",
            "device_logs",
            &["device_id", "received_at"],
        ))
        .await?;
        // Paginação por cursor da tela geral e o corte da retenção. O B-tree
        // ascendente atende o `ORDER BY received_at DESC` lendo ao contrário.
        m.create_index(index(
            "device_logs_received_at_index",
            "device_logs",
            &["received_at"],
        ))
        .await?;
        // "Só erro e acima, nas últimas 24 h" — o filtro que abre a tela.
        m.create_index(index(
            "device_logs_severity_received_index",
            "device_logs",
            &["severity", "received_at"],
        ))
        .await?;
        // "Só o log da aplicação, do mais novo para o mais velho" — a aba do
        // servidor. Sem ele, filtrar por origem num banco dominado por syslog
        // varreria a tabela.
        m.create_index(index(
            "device_logs_source_received_index",
            "device_logs",
            &["source", "received_at"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("device_logs")).await?;
        Ok(())
    }
}
