//! Respostas do backup do próprio NetMonitor: o plano, as cópias guardadas num
//! destino e o que a prévia e a restauração encontraram.
//!
//! O quadro de contagens é o mesmo para o arquivo enviado pelo navegador e para
//! a cópia lida de um destino.

use chrono::Utc;
use serde::Serialize;
use ts_rs::TS;

use crate::{
    models::system_backup_plan,
    services::backup::{
        copies::{BackupEntry, RunOutcome},
        plan,
        service::TableCounts,
    },
};

/// O plano de backup do sistema, como a tela o mostra.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SystemBackupPlanResponse {
    #[ts(type = "number | null")]
    pub storage_destination_id: Option<i64>,
    pub storage_destination_name: Option<String>,
    pub backup_enabled: bool,
    pub backup_interval_hours: i32,
    pub backup_retention: i32,
    pub last_backup_at: Option<String>,
    /// `success` | `failed`.
    pub last_backup_status: Option<String>,
    pub last_backup_error: Option<String>,
    pub next_backup_at: Option<String>,
    /// Há uma cópia em andamento agora.
    pub running: bool,
}

impl SystemBackupPlanResponse {
    #[must_use]
    pub fn new(
        row: &system_backup_plan::Model,
        storage_name: Option<String>,
        running: bool,
    ) -> Self {
        Self {
            storage_destination_id: row.storage_destination_id,
            storage_destination_name: storage_name,
            backup_enabled: row.backup_enabled,
            backup_interval_hours: row.backup_interval_hours,
            backup_retention: row.backup_retention,
            last_backup_at: row.last_backup_at.map(|at| at.to_rfc3339()),
            last_backup_status: row.last_backup_status.clone(),
            last_backup_error: row.last_backup_error.clone(),
            next_backup_at: plan::policy(row)
                .next_run(Utc::now())
                .map(|at| at.to_rfc3339()),
            running,
        }
    }
}

/// Uma cópia da configuração guardada num destino.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SystemBackupCopyResponse {
    pub key: String,
    pub name: String,
    #[ts(type = "number | null")]
    pub size: Option<i64>,
    pub last_modified: Option<String>,
}

impl From<BackupEntry> for SystemBackupCopyResponse {
    fn from(entry: BackupEntry) -> Self {
        Self {
            key: entry.key,
            name: entry.name,
            size: entry.size,
            last_modified: entry.last_modified,
        }
    }
}

/// Resultado de "Fazer backup agora".
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SystemBackupRunResponse {
    pub copy: SystemBackupCopyResponse,
    pub storage_destination_name: String,
    /// Cópias antigas apagadas pela retenção.
    #[ts(type = "number")]
    pub pruned: usize,
}

impl SystemBackupRunResponse {
    #[must_use]
    pub fn new(outcome: RunOutcome, storage_name: String) -> Self {
        Self {
            copy: outcome.entry.into(),
            storage_destination_name: storage_name,
            pruned: outcome.pruned,
        }
    }
}

/// `{"tables": [{"table": "...", "rows": n}], "totalRows": n}`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct BackupCountsResponse {
    pub tables: Vec<BackupTableCount>,
    #[ts(type = "number")]
    pub total_rows: usize,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct BackupTableCount {
    pub table: String,
    #[ts(type = "number")]
    pub rows: usize,
}

impl From<TableCounts> for BackupCountsResponse {
    fn from(counts: TableCounts) -> Self {
        Self {
            total_rows: counts.iter().map(|(_, rows)| rows).sum(),
            tables: counts
                .into_iter()
                .map(|(table, rows)| BackupTableCount { table, rows })
                .collect(),
        }
    }
}
