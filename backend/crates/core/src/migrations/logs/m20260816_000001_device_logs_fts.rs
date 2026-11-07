//! Índice de texto para a busca na mensagem — dialeto-específico.
//!
//! **Por que existe.** Medido no SPIKE-06 com 1 M de linhas: `LIKE '%termo%'`
//! numa janela de 7 dias leva 366 µs quando **encontra** (o `LIMIT 51` sai cedo
//! percorrendo o índice de `received_at` ao contrário) e **577 ms quando não
//! encontra** — sem casamento não há saída antecipada, e provar que o termo não
//! existe custa varrer a janela inteira. O segundo caso é o comum: o operador
//! procura um erro específico na semana e não acha.
//!
//! O índice de texto inverte isso: achar o termo vira uma sondagem, e "não
//! existe" é a resposta mais barata de todas.
//!
//! **Nada aqui é obrigatório.** Se a criação falhar — SQLite compilado sem
//! FTS5, permissão negada no Postgres —, a migration passa assim mesmo e a
//! busca continua no `LIKE`. É o `search::select` que descobre em tempo de
//! execução o que existe. Um índice de conforto não pode impedir o boot.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let sql = match db.get_database_backend() {
            // `content='device_logs'` é FTS5 de conteúdo externo: guarda só o
            // índice invertido, não uma segunda cópia do texto. Os gatilhos
            // mantêm os dois lados em dia — inclusive na purga da retenção, que
            // apaga em blocos.
            sea_orm::DatabaseBackend::Sqlite => {
                "CREATE VIRTUAL TABLE IF NOT EXISTS device_logs_fts USING fts5(
                     message,
                     content='device_logs',
                     content_rowid='id',
                     tokenize='unicode61'
                 );
                 CREATE TRIGGER IF NOT EXISTS device_logs_fts_insert
                 AFTER INSERT ON device_logs BEGIN
                     INSERT INTO device_logs_fts(rowid, message)
                     VALUES (new.id, new.message);
                 END;
                 CREATE TRIGGER IF NOT EXISTS device_logs_fts_delete
                 AFTER DELETE ON device_logs BEGIN
                     INSERT INTO device_logs_fts(device_logs_fts, rowid, message)
                     VALUES ('delete', old.id, old.message);
                 END;"
            }
            // Não há `UPDATE`: a tabela é append-only, e um gatilho para um
            // caminho que não existe é código morto com custo de escrita.
            sea_orm::DatabaseBackend::Postgres => {
                "CREATE INDEX IF NOT EXISTS device_logs_message_fts_index
                 ON device_logs USING GIN (to_tsvector('simple', message));"
            }
            _ => return Ok(()),
        };

        if let Err(error) = db.execute_unprepared(sql).await {
            // Ver a nota do módulo: a busca degrada para `LIKE`, que funciona.
            // O crate de migration não depende de `tracing`, e acrescentá-lo
            // por uma linha não se paga — `eprintln!` chega ao mesmo lugar num
            // passo que só roda no boot.
            eprintln!(
                "[netmonitor] índice de texto do banco de logs não criado ({error}); \
                 a busca usará LIKE"
            );
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        let sql = match db.get_database_backend() {
            sea_orm::DatabaseBackend::Sqlite => {
                "DROP TRIGGER IF EXISTS device_logs_fts_insert;
                 DROP TRIGGER IF EXISTS device_logs_fts_delete;
                 DROP TABLE IF EXISTS device_logs_fts;"
            }
            sea_orm::DatabaseBackend::Postgres => {
                "DROP INDEX IF EXISTS device_logs_message_fts_index;"
            }
            _ => return Ok(()),
        };
        let _ = db.execute_unprepared(sql).await;
        Ok(())
    }
}
