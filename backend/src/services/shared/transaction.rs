//! Transação de escrita que se comporta igual no SQLite e no PostgreSQL.
//!
//! O `BEGIN` padrão do SQLite é **deferido**: a transação nasce de leitura e
//! só vira de escrita no primeiro `INSERT`/`UPDATE`. Em modo WAL, se outra
//! conexão escreveu entre a leitura e a escrita, a promoção falha **na hora**
//! com `database is locked` — o `busy_timeout` nem é consultado, porque
//! esperar poderia dar impasse. Dois monitores terminando juntos bastavam para
//! perder um resultado por minuto (o `process_result` lê o monitor e depois
//! grava a observação).
//!
//! `BEGIN IMMEDIATE` pega o lock de escrita já no início: quem chega depois
//! espera o `busy_timeout` na fila, como no PostgreSQL, em vez de falhar. No
//! PostgreSQL a opção é ignorada pela `sea-orm`.

use sea_orm::{DbErr, SqliteTransactionMode, TransactionOptions, TransactionTrait};

/// Abre uma transação que vai escrever. Use no lugar de `begin()` sempre que
/// a transação ler e depois gravar.
///
/// # Errors
///
/// Falha ao abrir a transação (banco indisponível ou ocupado além do
/// `busy_timeout`).
pub async fn begin_write<T: TransactionTrait>(db: &T) -> Result<T::Transaction, DbErr> {
    db.begin_with_options(TransactionOptions {
        sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
        ..TransactionOptions::default()
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

    use super::*;

    /// Banco SQLite em arquivo, em WAL, como o de produção.
    async fn wal_database(name: &str) -> (DatabaseConnection, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "netmonitor-tx-{name}-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let url = format!("sqlite://{}?mode=rwc", path.display()).replace('\\', "/");
        let mut options = ConnectOptions::new(url);
        options.max_connections(4).min_connections(2);
        let db = Database::connect(options).await.unwrap();
        db.execute_unprepared("PRAGMA journal_mode = WAL;")
            .await
            .unwrap();
        db.execute_unprepared("CREATE TABLE results (id INTEGER PRIMARY KEY, monitor INTEGER)")
            .await
            .unwrap();
        (db, path)
    }

    /// O que o `process_result` faz: lê, espera um instante e grava.
    async fn read_then_write(db: DatabaseConnection, monitor: i64, immediate: bool) -> bool {
        let txn = if immediate {
            begin_write(&db).await
        } else {
            db.begin().await
        };
        let Ok(txn) = txn else {
            return false;
        };
        if txn
            .execute_unprepared("SELECT COUNT(*) FROM results")
            .await
            .is_err()
        {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        let wrote = txn
            .execute_unprepared(&format!("INSERT INTO results (monitor) VALUES ({monitor})"))
            .await
            .is_ok();
        wrote && txn.commit().await.is_ok()
    }

    #[tokio::test]
    async fn begin_deferido_perde_escrita_concorrente_e_immediate_nao() {
        let (db, path) = wal_database("race").await;

        let deferred = tokio::join!(
            read_then_write(db.clone(), 1, false),
            read_then_write(db.clone(), 2, false),
        );
        assert!(
            !(deferred.0 && deferred.1),
            "o BEGIN deferido devia reproduzir o 'database is locked'"
        );

        let immediate = tokio::join!(
            read_then_write(db.clone(), 3, true),
            read_then_write(db.clone(), 4, true),
        );
        assert!(immediate.0 && immediate.1, "com IMMEDIATE as duas gravam");

        db.close().await.unwrap();
        let _ = std::fs::remove_file(&path);
    }
}
