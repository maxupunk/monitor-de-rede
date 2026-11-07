//! Serialização das janelas de manutenção (Fase 3).
//!
//! Os tipos aqui são fonte da verdade do `frontend/src/stores/maintenanceWindows.ts`.

use serde::Serialize;
use ts_rs::TS;

use crate::models::maintenance_windows;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MaintenanceWindowResponse {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number | null")]
    pub site_id: Option<i64>,
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    #[ts(type = "number | null")]
    pub created_by: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<maintenance_windows::Model> for MaintenanceWindowResponse {
    fn from(row: maintenance_windows::Model) -> Self {
        Self {
            id: row.id,
            site_id: row.site_id,
            device_id: row.device_id,
            name: row.name,
            description: row.description,
            starts_at: row.starts_at.to_rfc3339(),
            ends_at: row.ends_at.to_rfc3339(),
            created_by: row.created_by,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}
