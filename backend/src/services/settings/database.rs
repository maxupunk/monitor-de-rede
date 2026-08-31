//! Informações sobre o banco de dados em uso.
//!
//! Expõe o tipo do dialeto (SQLite/PostgreSQL) e o tamanho ocupado no
//! disco, útil para o operador acompanhar o crescimento do arquivo/histórico.

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::shared::errors::{AppError, AppResult};

/// Tipo do banco de dados detectado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DbType {
    Sqlite,
    Postgres,
}

/// Dados retornados por `GET /api/settings/database-size`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseInfo {
    pub db_type: DbType,
    pub size_bytes: i64,
}

/// Coleta o tipo e o tamanho do banco de dados.
///
/// # Errors
///
/// Retorna erro se a consulta ao banco falhar ou devolver um valor inesperado.
pub async fn database_info<C: ConnectionTrait>(
    db: &C,
    database_url: &str,
) -> AppResult<DatabaseInfo> {
    let db_type = infer_db_type(db, database_url);

    let size_bytes = match db_type {
        DbType::Sqlite => sqlite_size_bytes(db).await?,
        DbType::Postgres => postgres_size_bytes(db).await?,
    };

    Ok(DatabaseInfo {
        db_type,
        size_bytes,
    })
}

fn infer_db_type<C: ConnectionTrait>(db: &C, database_url: &str) -> DbType {
    match db.get_database_backend() {
        DatabaseBackend::Sqlite => DbType::Sqlite,
        DatabaseBackend::Postgres => DbType::Postgres,
        // Fallback conservador para quando `sea-orm` não consegue distinguir.
        _ => {
            let url = database_url.to_lowercase();
            if url.starts_with("postgres://") || url.starts_with("postgresql://") {
                DbType::Postgres
            } else {
                DbType::Sqlite
            }
        }
    }
}

async fn sqlite_size_bytes<C: ConnectionTrait>(db: &C) -> AppResult<i64> {
    let page_count = query_single_i64(
        db,
        DatabaseBackend::Sqlite,
        "PRAGMA page_count;",
        "page_count",
    )
    .await?;
    let page_size = query_single_i64(
        db,
        DatabaseBackend::Sqlite,
        "PRAGMA page_size;",
        "page_size",
    )
    .await?;

    page_count
        .checked_mul(page_size)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("tamanho do SQLite excede i64")))
}

async fn postgres_size_bytes<C: ConnectionTrait>(db: &C) -> AppResult<i64> {
    query_single_i64(
        db,
        DatabaseBackend::Postgres,
        "SELECT pg_database_size(current_database()) AS size;",
        "size",
    )
    .await
}

async fn query_single_i64<C: ConnectionTrait>(
    db: &C,
    backend: DatabaseBackend,
    sql: &str,
    column: &str,
) -> AppResult<i64> {
    let statement = Statement::from_string(backend, sql.to_owned());
    let row = db
        .query_one_raw(statement)
        .await
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))?
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!("consulta de tamanho não retornou linha"))
        })?;

    row.try_get("", column)
        .map_err(|e| AppError::Internal(anyhow::Error::new(e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::DatabaseConnection;

    async fn sqlite_memory() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    #[tokio::test]
    async fn sqlite_retorna_tamanho_positivo() {
        let db = sqlite_memory().await;
        let info = database_info(&db, "sqlite::memory:")
            .await
            .expect("tamanho");
        assert_eq!(info.db_type, DbType::Sqlite);
        assert!(info.size_bytes > 0);
    }
}
