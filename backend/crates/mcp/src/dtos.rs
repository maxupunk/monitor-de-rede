//! Contratos HTTP da gestão do servidor MCP (`/api/mcp/*`).
//!
//! Todos os structs aqui usam `camelCase` para espelhar exatamente
//! `frontend/lib/features/ai/models/mcp_models.dart`.

use chrono::Utc;
use serde::{Deserialize, Serialize};

use netmonitor_core::{mcp::McpDeployAppSummary, models::mcp_tokens};

use crate::{
    scopes::McpScope,
    settings::{McpSettings, ENDPOINT_PATH},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSettingsView {
    pub enabled: bool,
    pub allow_public_origin: bool,
    pub endpoint_path: String,
}

impl From<McpSettings> for McpSettingsView {
    fn from(settings: McpSettings) -> Self {
        Self {
            enabled: settings.enabled,
            allow_public_origin: settings.allow_public_origin,
            endpoint_path: ENDPOINT_PATH.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMcpSettingsInput {
    pub enabled: bool,
    pub allow_public_origin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpScopeDescriptor {
    pub id: String,
    pub label: String,
    pub description: String,
    pub granted_by_role: bool,
    pub requires_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOverview {
    pub settings: McpSettingsView,
    pub scopes: Vec<McpScopeDescriptor>,
    pub deploy_applications: Vec<McpDeployAppSummary>,
    pub tools_count: usize,
    pub can_manage_settings: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMcpTokenInput {
    pub name: String,
    pub scopes: Vec<String>,
    pub allowed_application_ids: Option<Vec<i64>>,
    pub ttl_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTokenView {
    pub id: i64,
    pub user_id: i64,
    pub owner: String,
    pub name: String,
    pub hint: String,
    pub scopes: Vec<String>,
    pub allowed_application_ids: Option<Vec<i64>>,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    pub last_used_ip: Option<String>,
    pub revoked: bool,
    pub expired: bool,
    pub created_at: String,
}

impl McpTokenView {
    #[must_use]
    pub fn from_model(row: &mcp_tokens::Model, owner: String) -> Self {
        let scopes = crate::tokens::scopes_of(row)
            .into_iter()
            .map(|scope| scope.id().to_owned())
            .collect();
        let allowed_application_ids = crate::tokens::allowed_application_ids_of(row);
        Self {
            id: row.id,
            user_id: row.user_id,
            owner,
            name: row.name.clone(),
            hint: row.hint.clone(),
            scopes,
            allowed_application_ids,
            expires_at: row.expires_at.to_rfc3339(),
            last_used_at: row.last_used_at.map(|at| at.to_rfc3339()),
            last_used_ip: row.last_used_ip.clone(),
            revoked: row.revoked_at.is_some(),
            expired: row.expires_at.with_timezone(&Utc) <= Utc::now(),
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTokenCreated {
    pub token: McpTokenView,
    pub secret: String,
}

impl McpScopeDescriptor {
    #[must_use]
    pub fn for_scope(scope: McpScope, granted_by_role: bool) -> Self {
        Self {
            id: scope.id().to_owned(),
            label: scope.label().to_owned(),
            description: scope.description().to_owned(),
            granted_by_role,
            requires_admin: scope.requires_admin(),
        }
    }
}
