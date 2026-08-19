//! De onde a linha veio: `syslog` (rede) ou `application` (o próprio processo).
//!
//! # Uma coluna, e só uma
//!
//! A tentação era uma coluna JSON de contexto, com os campos estruturados do
//! evento do `tracing`. Seria uma **segunda superfície de busca** na tabela mais
//! quente do sistema, invisível ao índice de texto: o operador procuraria por
//! `monitor_id` na barra de busca e não acharia nada, porque o FTS indexa
//! `message` e não sabe ler JSON. Os campos do evento são achatados na mensagem,
//! como o próprio formatador do `tracing` faz, e o FTS os encontra de graça.
//!
//! O que a coluna resolve é outra pergunta, essa sim inexprimível na mensagem:
//! "isto é log do parque ou do servidor?". Ela é curta, indexável e tem
//! cardinalidade dois.
//!
//! `NOT NULL DEFAULT 'syslog'`: toda linha que já existe veio da rede, e o
//! default evita reescrever a tabela inteira num `UPDATE` de milhões de linhas.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_column("device_logs", "source").await? {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("device_logs"))
                    .add_column(
                        string_len("source", 16)
                            .default("syslog")
                            .not_null()
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;
        }

        // O índice cobre "só o log da aplicação, do mais novo para o mais
        // velho" — a consulta da aba do servidor. Sem ele, filtrar por origem
        // num banco dominado por syslog varreria a tabela.
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "CREATE INDEX IF NOT EXISTS device_logs_source_received_index \
             ON device_logs (source, received_at)"
                .to_string(),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_raw(Statement::from_string(
            db.get_database_backend(),
            "DROP INDEX IF EXISTS device_logs_source_received_index".to_string(),
        ))
        .await?;
        if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite
            && m.has_column("device_logs", "source").await?
        {
            m.alter_table(
                Table::alter()
                    .table(Alias::new("device_logs"))
                    .drop_column(Alias::new("source"))
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
