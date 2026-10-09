//! Serialização das conexões de banco e do histórico de backups.
//!
//! A senha não sai nunca: só `passwordSet`.

use chrono::Utc;
use serde::Serialize;
use ts_rs::TS;

use crate::{
    models::{database_backups, database_connections},
    services::databases::{schedule, service::databases_of, DatabaseEngine, Probe, SslMode},
};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseConnectionResponse {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password_set: bool,
    pub ssl_mode: SslMode,
    /// Vazio é "todos os bancos".
    pub databases: Vec<String>,
    #[ts(type = "number | null")]
    pub storage_destination_id: Option<i64>,
    pub storage_destination_name: Option<String>,
    /// Agente pelo qual a central chega ao banco; nulo é direto.
    #[ts(type = "number | null")]
    pub via_probe_id: Option<i64>,
    pub via_probe_name: Option<String>,
    pub backup_enabled: bool,
    pub backup_interval_hours: i32,
    pub backup_retention: i32,
    pub last_backup_at: Option<String>,
    /// `success` | `failed`.
    pub last_backup_status: Option<String>,
    pub last_backup_error: Option<String>,
    pub next_backup_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl DatabaseConnectionResponse {
    /// `storage_name` e `agent_name`: os nomes do armazenamento e do agente
    /// da conexão, quando há.
    #[must_use]
    pub fn new(
        row: &database_connections::Model,
        storage_name: Option<String>,
        agent_name: Option<String>,
    ) -> Self {
        Self {
            id: row.id,
            name: row.name.clone(),
            engine: DatabaseEngine::parse(&row.engine).unwrap_or(DatabaseEngine::Postgres),
            host: row.host.clone(),
            port: row.port,
            username: row.username.clone(),
            password_set: !row.password_encrypted.is_empty(),
            ssl_mode: SslMode::parse(&row.ssl_mode).unwrap_or(SslMode::Prefer),
            databases: databases_of(row),
            storage_destination_id: row.storage_destination_id,
            storage_destination_name: storage_name,
            via_probe_id: row.via_probe_id,
            via_probe_name: agent_name,
            backup_enabled: row.backup_enabled,
            backup_interval_hours: row.backup_interval_hours,
            backup_retention: row.backup_retention,
            last_backup_at: row.last_backup_at.map(|at| at.to_rfc3339()),
            last_backup_status: row.last_backup_status.clone(),
            last_backup_error: row.last_backup_error.clone(),
            next_backup_at: schedule::policy(row)
                .next_run(Utc::now())
                .map(|at| at.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseProbeResponse {
    #[ts(type = "number")]
    pub latency_ms: i64,
    pub version: String,
    pub databases: Vec<String>,
}

impl From<Probe> for DatabaseProbeResponse {
    fn from(probe: Probe) -> Self {
        Self {
            latency_ms: probe.latency_ms,
            version: probe.version,
            databases: probe.databases,
        }
    }
}

/// Uma linha do histórico.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseBackupResponse {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub connection_id: i64,
    pub database_name: String,
    #[ts(type = "number | null")]
    pub storage_destination_id: Option<i64>,
    pub object_key: Option<String>,
    #[ts(type = "number | null")]
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub tables: i32,
    #[ts(type = "number")]
    pub rows: i64,
    #[ts(type = "number")]
    pub duration_ms: i64,
    /// `running` | `success` | `failed`.
    pub status: String,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    /// `manual` | `scheduled`.
    pub trigger: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

impl From<database_backups::Model> for DatabaseBackupResponse {
    fn from(row: database_backups::Model) -> Self {
        Self {
            id: row.id,
            connection_id: row.connection_id,
            database_name: row.database_name,
            storage_destination_id: row.storage_destination_id,
            object_key: row.object_key,
            size_bytes: row.size_bytes,
            checksum: row.checksum,
            tables: row.tables,
            rows: row.rows,
            duration_ms: row.duration_ms,
            status: row.status,
            error: row.error,
            warnings: serde_json::from_value(row.warnings).unwrap_or_default(),
            trigger: row.trigger,
            started_at: row.started_at.to_rfc3339(),
            finished_at: row.finished_at.map(|at| at.to_rfc3339()),
        }
    }
}
