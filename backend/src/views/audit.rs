//! Serialização dos eventos de auditoria (Fase 3 do roadmap).

use serde::Serialize;
use ts_rs::TS;

use crate::models::audit_logs;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AuditLogResponse {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number | null")]
    pub user_id: Option<i64>,
    pub user_email: Option<String>,
    pub action: String,
    pub resource_type: Option<String>,
    #[ts(type = "number | null")]
    pub resource_id: Option<i64>,
    pub resource_label: Option<String>,
    pub description: Option<String>,
    #[ts(type = "any")]
    pub changes: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: String,
}

impl AuditLogResponse {
    pub fn from_model(row: audit_logs::Model, user_email: Option<String>) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            user_email,
            action: row.action,
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            resource_label: row.resource_label,
            description: row.description,
            changes: row.changes,
            ip_address: row.ip_address,
            user_agent: row.user_agent,
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AuditLogListResponse {
    pub data: Vec<AuditLogResponse>,
    pub meta: crate::services::shared::pagination::LucidMeta,
}
