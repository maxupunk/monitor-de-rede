//! Entradas das rotas de armazenamento (`/api/storages`).

use serde::Deserialize;
use ts_rs::TS;

use crate::services::storage::{
    service::StorageInput, StorageConfig, StorageProvider, DEFAULT_LIST_LIMIT,
};

/// Formulário de criação e edição. Na edição, segredo em branco mantém o
/// gravado.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageDestinationInput {
    pub name: String,
    pub provider: StorageProvider,
    pub config: StorageConfig,
}

impl From<StorageDestinationInput> for StorageInput {
    fn from(input: StorageDestinationInput) -> Self {
        Self {
            name: input.name,
            provider: input.provider,
            config: input.config,
        }
    }
}

/// "Testar conexão" do formulário, antes de salvar. Com `id`, os segredos em
/// branco vêm do cadastro.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageTestInput {
    #[ts(type = "number | null", optional)]
    pub id: Option<i64>,
    pub provider: StorageProvider,
    pub config: StorageConfig,
}

/// Uma pasta do explorador.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageBrowseQuery {
    #[serde(default)]
    pub path: String,
    pub cursor: Option<String>,
}

impl StorageBrowseQuery {
    #[must_use]
    pub fn options(&self) -> crate::services::storage::ListOptions {
        crate::services::storage::ListOptions {
            cursor: self.cursor.clone().filter(|value| !value.is_empty()),
            limit: Some(DEFAULT_LIST_LIMIT),
            prefix: None,
        }
    }
}

/// Um objeto do destino (download e exclusão pelo explorador).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectQuery {
    pub key: String,
    #[serde(default)]
    pub directory: bool,
}
