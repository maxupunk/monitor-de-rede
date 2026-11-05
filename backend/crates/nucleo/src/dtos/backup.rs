//! Entradas do backup do próprio NetMonitor (`/api/backup/system`).

use serde::Deserialize;
use ts_rs::TS;

use crate::services::backup::plan::PlanInput;

/// O plano: para onde, de quanto em quanto tempo, quantas manter.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SystemBackupPlanInput {
    #[ts(type = "number | null")]
    pub storage_destination_id: Option<i64>,
    pub backup_enabled: bool,
    pub backup_interval_hours: i32,
    pub backup_retention: i32,
}

impl From<SystemBackupPlanInput> for PlanInput {
    fn from(input: SystemBackupPlanInput) -> Self {
        Self {
            storage_destination_id: input.storage_destination_id,
            backup_enabled: input.backup_enabled,
            backup_interval_hours: input.backup_interval_hours,
            backup_retention: input.backup_retention,
        }
    }
}

/// De qual destino listar as cópias; sem ele, o do plano.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemBackupCopiesQuery {
    pub storage_destination_id: Option<i64>,
}

/// Uma cópia guardada num destino (prévia e restauração).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SystemBackupCopyInput {
    #[ts(type = "number")]
    pub storage_destination_id: i64,
    pub key: String,
}
