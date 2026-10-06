//! O que a prévia e a restauração de um backup respondem.
//!
//! Compartilhado entre o backup por arquivo (`/api/backup`) e o backup guardado
//! num armazenamento (`/api/storages/{id}/backups`): a tela mostra o mesmo
//! quadro de contagens nos dois casos.

use serde::Serialize;
use ts_rs::TS;

use crate::services::backup::service::TableCounts;

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
