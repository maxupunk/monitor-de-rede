//! Entradas das rotas de bancos de dados (`/api/databases`).

use serde::Deserialize;
use ts_rs::TS;

use crate::services::databases::{
    backups::RestoreRequest,
    service::{ConnectionInput, ProbeInput},
    DatabaseEngine, RestoreMode, SslMode,
};

/// Senha vazia é "não mexer": a tela nunca recebe a gravada para reenviar.
fn filled(password: Option<String>) -> Option<String> {
    password.filter(|value| !value.is_empty())
}

/// Formulário de criação e edição.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseConnectionInput {
    pub name: String,
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: i32,
    pub username: String,
    #[ts(optional)]
    pub password: Option<String>,
    pub ssl_mode: SslMode,
    /// Vazio é "todos os bancos".
    #[serde(default)]
    pub databases: Vec<String>,
    #[ts(type = "number | null")]
    pub storage_destination_id: Option<i64>,
    #[serde(default)]
    pub backup_enabled: bool,
    pub backup_interval_hours: i32,
    pub backup_retention: i32,
}

impl From<DatabaseConnectionInput> for ConnectionInput {
    fn from(input: DatabaseConnectionInput) -> Self {
        Self {
            name: input.name,
            engine: input.engine,
            host: input.host,
            port: input.port,
            username: input.username,
            password: filled(input.password),
            ssl_mode: input.ssl_mode,
            databases: input.databases,
            storage_destination_id: input.storage_destination_id,
            backup_enabled: input.backup_enabled,
            backup_interval_hours: input.backup_interval_hours,
            backup_retention: input.backup_retention,
        }
    }
}

/// "Testar e listar bancos" do formulário.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseProbeInput {
    /// Conexão em edição: senha vazia vem dela.
    #[ts(type = "number | null", optional)]
    pub id: Option<i64>,
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: i32,
    pub username: String,
    #[ts(optional)]
    pub password: Option<String>,
    pub ssl_mode: SslMode,
}

impl From<DatabaseProbeInput> for ProbeInput {
    fn from(input: DatabaseProbeInput) -> Self {
        Self {
            id: input.id,
            engine: input.engine,
            host: input.host,
            port: input.port,
            username: input.username,
            password: filled(input.password),
            ssl_mode: input.ssl_mode,
        }
    }
}

/// Restaurar uma cópia.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseRestoreInput {
    #[ts(type = "number")]
    pub target_connection_id: i64,
    pub database: String,
    pub mode: RestoreMode,
    /// No modo substituir, o nome do banco digitado de novo — a mesma
    /// confirmação que a tela pede, conferida aqui também.
    #[ts(optional)]
    pub confirm_database: Option<String>,
}

impl DatabaseRestoreInput {
    /// # Errors
    ///
    /// Modo substituir sem a confirmação igual ao nome.
    pub fn into_request(
        self,
        backup_id: i64,
    ) -> crate::services::shared::errors::AppResult<RestoreRequest> {
        if self.mode == RestoreMode::Replace
            && self.confirm_database.as_deref().map(str::trim) != Some(self.database.trim())
        {
            return Err(crate::services::shared::errors::AppError::validation(
                "Para substituir, digite o nome do banco exatamente como ele é",
            ));
        }
        Ok(RestoreRequest {
            backup_id,
            target_connection_id: self.target_connection_id,
            database: self.database,
            mode: self.mode,
        })
    }
}
