//! O que a tela do registry recebe.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use netmonitor_core::models::{registry_exceptions, registry_findings, registry_tokens};

use crate::{settings::Settings, tokens};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryScanSummary {
    /// `pending`, `done`, `no_sbom`, `failed`.
    pub status: String,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub unknown: u32,
    pub packages: Option<i32>,
    /// `sbom` ou `layers`.
    pub source: Option<String>,
    pub scanned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryRepositoryView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[ts(type = "number | null")]
    pub application_id: Option<i64>,
    pub application: Option<String>,
    pub versions: u32,
    /// Bytes dos blobs vinculados (cada um uma vez).
    #[ts(type = "number")]
    pub bytes: i64,
    pub latest_tag: Option<String>,
    pub last_push: Option<String>,
    pub latest_scan: Option<RegistryScanSummary>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryFeedView {
    pub ecosystem: String,
    pub synced_at: Option<String>,
    #[ts(type = "number")]
    pub entries: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryOverview {
    pub settings: Settings,
    #[ts(type = "number")]
    pub used_bytes: u64,
    #[ts(type = "number")]
    pub quota_bytes: u64,
    pub repositories: Vec<RegistryRepositoryView>,
    pub feeds: Vec<RegistryFeedView>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryVersionView {
    #[ts(type = "number")]
    pub id: i64,
    pub digest: String,
    pub tags: Vec<String>,
    /// Tag de commit: não muda de conteúdo.
    pub immutable: bool,
    pub media_type: String,
    pub platforms: Vec<String>,
    #[ts(type = "number")]
    pub bytes: i64,
    pub pushed_by: Option<String>,
    #[ts(type = "Record<string, unknown> | null")]
    pub provenance: Option<Value>,
    pub pinned: bool,
    pub created_at: String,
    pub scan: Option<RegistryScanSummary>,
    /// Alvos de deploy com esta versão no ar.
    pub used_by: Vec<String>,
    /// No ar, guardada para o rollback ou a caminho de algum alvo: não pode
    /// ser apagada.
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryRepositoryDetail {
    pub repository: RegistryRepositoryView,
    pub versions: Vec<RegistryVersionView>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryFindingView {
    pub advisory: String,
    pub aliases: Vec<String>,
    pub package: String,
    pub ecosystem: String,
    pub installed: String,
    pub fixed: Option<String>,
    pub severity: String,
    pub summary: Option<String>,
    /// Coberto por uma exceção válida.
    pub excepted: bool,
}

impl RegistryFindingView {
    #[must_use]
    pub fn new(row: &registry_findings::Model, excepted: bool) -> Self {
        Self {
            advisory: row.advisory.clone(),
            aliases: row
                .aliases
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect(),
            package: row.package.clone(),
            ecosystem: row.ecosystem.clone(),
            installed: row.installed.clone(),
            fixed: row.fixed.clone(),
            severity: row.severity.clone(),
            summary: row.summary.clone(),
            excepted,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryTokenView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub owner: String,
    pub hint: String,
    pub scope: tokens::Scope,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    pub last_used_ip: Option<String>,
    pub revoked: bool,
    pub expired: bool,
    pub created_at: String,
}

impl RegistryTokenView {
    #[must_use]
    pub fn new(row: &registry_tokens::Model, owner: String) -> Self {
        Self {
            id: row.id,
            name: row.name.clone(),
            owner,
            hint: row.hint.clone(),
            scope: tokens::scope_of(row),
            expires_at: row.expires_at.to_rfc3339(),
            last_used_at: row.last_used_at.map(|at| at.to_rfc3339()),
            last_used_ip: row.last_used_ip.clone(),
            revoked: row.revoked_at.is_some(),
            expired: row.expires_at.with_timezone(&chrono::Utc) <= chrono::Utc::now(),
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

/// O token recém-criado: o texto aparece só aqui.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryTokenCreated {
    pub token: RegistryTokenView,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryExceptionView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number | null")]
    pub application_id: Option<i64>,
    pub application: Option<String>,
    pub advisory: String,
    pub reason: String,
    pub expires_at: String,
    pub expired: bool,
    pub created_by: String,
}

impl RegistryExceptionView {
    #[must_use]
    pub fn new(row: &registry_exceptions::Model, application: Option<String>) -> Self {
        Self {
            id: row.id,
            application_id: row.application_id,
            application,
            advisory: row.advisory.clone(),
            reason: row.reason.clone(),
            expires_at: row.expires_at.to_rfc3339(),
            expired: row.expires_at.with_timezone(&chrono::Utc) <= chrono::Utc::now(),
            created_by: row.created_by.clone(),
        }
    }
}
