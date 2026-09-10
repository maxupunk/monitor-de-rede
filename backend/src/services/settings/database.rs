//! Informações sobre o banco de dados em uso e manutenção de histórico.
//!
//! Expõe o tipo do dialeto (SQLite/PostgreSQL), o tamanho ocupado no
//! disco, os limites temporais do histórico armazenado e a rotina de
//! limpeza para recuperação de espaço em disco.

use sea_orm::{
    prelude::DateTimeWithTimeZone, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, QueryOrder, QuerySelect, Statement,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::{
        _entities::{
            alert_events, discovery_results, discovery_runs, event_outbox, metrics,
            monitor_results, monitor_results_hourly, notification_outbox,
        },
        logs::device_logs,
    },
    services::shared::errors::{AppError, AppResult},
};

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
    pub earliest_record: Option<String>,
    pub latest_record: Option<String>,
}

/// Estatísticas retornadas por `POST /api/settings/clear-history`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ClearHistoryStats {
    pub metrics_deleted: u64,
    pub results_deleted: u64,
    pub logs_deleted: u64,
    pub alerts_deleted: u64,
    pub total_deleted: u64,
}

/// Coleta o tipo, o tamanho e os limites temporais do banco de dados.
///
/// # Errors
///
/// Retorna erro se a consulta ao banco falhar ou devolver um valor inesperado.
pub async fn database_info<C: ConnectionTrait>(
    db: &C,
    database_url: &str,
    logs_db: Option<&DatabaseConnection>,
) -> AppResult<DatabaseInfo> {
    let db_type = infer_db_type(db, database_url);

    let mut size_bytes = match db_type {
        DbType::Sqlite => sqlite_size_bytes(db).await?,
        DbType::Postgres => postgres_size_bytes(db).await?,
    };

    if db_type == DbType::Sqlite {
        if let Some(logs) = logs_db {
            if let Ok(logs_size) = sqlite_size_bytes(logs).await {
                size_bytes = size_bytes.saturating_add(logs_size);
            }
        }
    }

    let (earliest_record, latest_record) = query_history_bounds(db, logs_db).await;

    Ok(DatabaseInfo {
        db_type,
        size_bytes,
        earliest_record,
        latest_record,
    })
}

/// Apaga todo o histórico técnico (métricas, resultados de monitoramento,
/// execuções de discovery, alertas resolvidos/antigos, outbox e logs de dispositivos),
/// mantendo intactos dispositivos, interfaces, links, monitores, regras de alerta e configurações.
///
/// Em SQLite, executa `incremental_vacuum`, `VACUUM` e `wal_checkpoint(TRUNCATE)`
/// para devolver fisicamente o espaço em disco ao sistema de arquivos.
pub async fn clear_history<C: ConnectionTrait>(
    db: &C,
    logs_db: Option<&DatabaseConnection>,
) -> AppResult<ClearHistoryStats> {
    let metrics_deleted = metrics::Entity::delete_many().exec(db).await?.rows_affected;

    let results_deleted = monitor_results::Entity::delete_many()
        .exec(db)
        .await?
        .rows_affected;

    let hourly_deleted = monitor_results_hourly::Entity::delete_many()
        .exec(db)
        .await?
        .rows_affected;

    let _ = discovery_results::Entity::delete_many().exec(db).await;
    let _ = discovery_runs::Entity::delete_many().exec(db).await;

    let alerts_deleted = alert_events::Entity::delete_many()
        .exec(db)
        .await?
        .rows_affected;

    let _ = event_outbox::Entity::delete_many().exec(db).await;
    let _ = notification_outbox::Entity::delete_many().exec(db).await;

    let logs_deleted = if let Some(logs) = logs_db {
        device_logs::Entity::delete_many()
            .exec(logs)
            .await?
            .rows_affected
    } else {
        0
    };

    devolver_disco_sqlite(db).await;
    if let Some(logs) = logs_db {
        devolver_disco_sqlite(logs).await;
    }

    let total_deleted =
        metrics_deleted + results_deleted + hourly_deleted + alerts_deleted + logs_deleted;

    Ok(ClearHistoryStats {
        metrics_deleted,
        results_deleted: results_deleted + hourly_deleted,
        logs_deleted,
        alerts_deleted,
        total_deleted,
    })
}

async fn devolver_disco_sqlite<C: ConnectionTrait>(db: &C) {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        return;
    }
    for pragma in [
        "PRAGMA incremental_vacuum;",
        "VACUUM;",
        "PRAGMA wal_checkpoint(TRUNCATE);",
    ] {
        if let Err(error) = db.execute_unprepared(pragma).await {
            tracing::debug!(%error, pragma, "aviso de compactação do SQLite");
        }
    }
}

async fn query_history_bounds<C: ConnectionTrait>(
    db: &C,
    logs_db: Option<&DatabaseConnection>,
) -> (Option<String>, Option<String>) {
    let min_metrics: Option<DateTimeWithTimeZone> = metrics::Entity::find()
        .select_only()
        .column(metrics::Column::RecordedAt)
        .order_by_asc(metrics::Column::RecordedAt)
        .limit(1)
        .into_tuple()
        .one(db)
        .await
        .ok()
        .flatten();

    let max_metrics: Option<DateTimeWithTimeZone> = metrics::Entity::find()
        .select_only()
        .column(metrics::Column::RecordedAt)
        .order_by_desc(metrics::Column::RecordedAt)
        .limit(1)
        .into_tuple()
        .one(db)
        .await
        .ok()
        .flatten();

    let min_results: Option<DateTimeWithTimeZone> = monitor_results::Entity::find()
        .select_only()
        .column(monitor_results::Column::CreatedAt)
        .order_by_asc(monitor_results::Column::CreatedAt)
        .limit(1)
        .into_tuple()
        .one(db)
        .await
        .ok()
        .flatten();

    let max_results: Option<DateTimeWithTimeZone> = monitor_results::Entity::find()
        .select_only()
        .column(monitor_results::Column::CreatedAt)
        .order_by_desc(monitor_results::Column::CreatedAt)
        .limit(1)
        .into_tuple()
        .one(db)
        .await
        .ok()
        .flatten();

    let min_alerts: Option<DateTimeWithTimeZone> = alert_events::Entity::find()
        .select_only()
        .column(alert_events::Column::StartedAt)
        .order_by_asc(alert_events::Column::StartedAt)
        .limit(1)
        .into_tuple()
        .one(db)
        .await
        .ok()
        .flatten();

    let max_alerts: Option<DateTimeWithTimeZone> = alert_events::Entity::find()
        .select_only()
        .column(alert_events::Column::StartedAt)
        .order_by_desc(alert_events::Column::StartedAt)
        .limit(1)
        .into_tuple()
        .one(db)
        .await
        .ok()
        .flatten();

    let (min_logs, max_logs) = if let Some(logs) = logs_db {
        let min_l: Option<DateTimeWithTimeZone> = device_logs::Entity::find()
            .select_only()
            .column(device_logs::Column::ReceivedAt)
            .order_by_asc(device_logs::Column::ReceivedAt)
            .limit(1)
            .into_tuple()
            .one(logs)
            .await
            .ok()
            .flatten();

        let max_l: Option<DateTimeWithTimeZone> = device_logs::Entity::find()
            .select_only()
            .column(device_logs::Column::ReceivedAt)
            .order_by_desc(device_logs::Column::ReceivedAt)
            .limit(1)
            .into_tuple()
            .one(logs)
            .await
            .ok()
            .flatten();

        (min_l, max_l)
    } else {
        (None, None)
    };

    let earliest = [min_metrics, min_results, min_alerts, min_logs]
        .into_iter()
        .flatten()
        .min();

    let latest = [max_metrics, max_results, max_alerts, max_logs]
        .into_iter()
        .flatten()
        .max();

    (
        earliest.map(|dt| dt.to_rfc3339()),
        latest.map(|dt| dt.to_rfc3339()),
    )
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
    use chrono::Utc;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};

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
        let info = database_info(&db, "sqlite::memory:", None)
            .await
            .expect("tamanho");
        assert_eq!(info.db_type, DbType::Sqlite);
        assert!(info.size_bytes > 0);
        assert_eq!(info.earliest_record, None);
        assert_eq!(info.latest_record, None);
    }

    #[tokio::test]
    async fn sqlite_detecta_datas_e_limpa_historico() {
        let db = sqlite_memory().await;
        let now = Utc::now();

        use crate::models::_entities::devices;

        let device = devices::ActiveModel {
            id: Set(1),
            name: Set("Device 1".into()),
            r#type: Set("router".into()),
            is_monitored: Set(true),
            snmp_enabled: Set(false),
            snmp_poll_interval_seconds: Set(60),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        device.insert(&db).await.expect("insert device");

        let metric = metrics::ActiveModel {
            id: Set(1),
            device_id: Set(1),
            interface_id: Set(None),
            monitor_id: Set(None),
            name: Set("cpu.utilization".into()),
            value: Set(42.0),
            unit: Set("%".into()),
            recorded_at: Set(now.into()),
            created_at: Set(now.into()),
        };
        metric.insert(&db).await.expect("insert metric");

        let info_before = database_info(&db, "sqlite::memory:", None)
            .await
            .expect("database_info");
        assert!(info_before.earliest_record.is_some());
        assert!(info_before.latest_record.is_some());

        let stats = clear_history(&db, None).await.expect("clear_history");
        assert_eq!(stats.metrics_deleted, 1);
        assert_eq!(stats.total_deleted, 1);

        let info_after = database_info(&db, "sqlite::memory:", None)
            .await
            .expect("database_info");
        assert_eq!(info_after.earliest_record, None);
        assert_eq!(info_after.latest_record, None);
    }
}
