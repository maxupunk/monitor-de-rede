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

use sea_orm::{
    ConnectionTrait, DatabaseBackend, DbErr, IsolationLevel, SqliteTransactionMode,
    TransactionOptions, TransactionTrait,
};

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

/// Abre uma transação de escrita em que **todas** as leituras veem o mesmo
/// instante do banco — para quem lê um conjunto, decide com base nele e depois
/// apaga exatamente esse conjunto (o rollup agrega o bruto e purga o que
/// agregou).
///
/// No PostgreSQL é `REPEATABLE READ`: uma linha confirmada depois do começo da
/// transação — inclusive uma de `id` menor, reservado antes e confirmado
/// depois — não aparece na leitura **nem** é alcançada pelo `DELETE`. No
/// SQLite é o [`begin_write`]: o `BEGIN IMMEDIATE` já impede qualquer outra
/// escrita até o commit, o que dá a mesma garantia (o SQLite não aceita nível
/// de isolamento, e a `sea-orm` só registraria um aviso).
///
/// # Errors
///
/// Falha ao abrir a transação.
pub async fn begin_snapshot_write<T>(db: &T) -> Result<T::Transaction, DbErr>
where
    T: TransactionTrait + ConnectionTrait,
{
    let isolation_level = (db.get_database_backend() == DatabaseBackend::Postgres)
        .then_some(IsolationLevel::RepeatableRead);
    db.begin_with_options(TransactionOptions {
        isolation_level,
        sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
        ..TransactionOptions::default()
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use tokio::sync::Barrier;

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
    ///
    /// `both_read`, no caso deferido, segura a escrita até as duas transações
    /// terem lido: sem ela, sob a suíte cheia uma podia terminar inteira antes
    /// da outra começar, e a corrida não se reproduzia. No caso `IMMEDIATE` não
    /// cabe barreira — a segunda espera a primeira já no `BEGIN`.
    async fn read_then_write(
        db: DatabaseConnection,
        monitor: i64,
        immediate: bool,
        both_read: Option<Arc<Barrier>>,
    ) -> bool {
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
        if let Some(barrier) = both_read {
            barrier.wait().await;
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

        let both_read = Arc::new(Barrier::new(2));
        let deferred = tokio::join!(
            read_then_write(db.clone(), 1, false, Some(Arc::clone(&both_read))),
            read_then_write(db.clone(), 2, false, Some(both_read)),
        );
        assert!(
            !(deferred.0 && deferred.1),
            "o BEGIN deferido devia reproduzir o 'database is locked'"
        );

        let immediate = tokio::join!(
            read_then_write(db.clone(), 3, true, None),
            read_then_write(db.clone(), 4, true, None),
        );
        assert!(immediate.0 && immediate.1, "com IMMEDIATE as duas gravam");

        db.close().await.unwrap();
        let _ = std::fs::remove_file(&path);
    }
}
