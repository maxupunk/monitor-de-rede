//! Contratos do servidor MCP no núcleo (ADR 017 / ADR 024).
//!
//! O crate `netmonitor-mcp` depende apenas do núcleo (`netmonitor-core`): não
//! enxerga Servidores diretamente. Por isso o catálogo de aplicações de deploy
//! usado ao criar/exibir tokens escopados cruza a fronteira por este trait,
//! preenchido no ponto de montagem (`src/registries.rs`).

use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::shared::errors::AppResult;

pub use crate::ai::args::McpToolContext;

/// Resumo de uma aplicação de deploy para escopo de token MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDeployAppSummary {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub repository: String,
}

/// Catálogo de aplicações de deploy fornecido pela área Servidores.
#[async_trait]
pub trait McpDeployAppCatalog: Send + Sync {
    /// Lista as aplicações de deploy cadastradas, em ordem alfabética.
    ///
    /// # Errors
    ///
    /// Propaga falhas de consulta ao banco.
    async fn list_applications(
        &self,
        db: &DatabaseConnection,
    ) -> AppResult<Vec<McpDeployAppSummary>>;
}

#[derive(Clone)]
struct RegisteredDeployAppCatalog(Arc<dyn McpDeployAppCatalog>);

/// Instala o catálogo de aplicações de deploy no `shared_store` do processo.
pub fn install_deploy_app_catalog(ctx: &AppContext, catalog: Arc<dyn McpDeployAppCatalog>) {
    ctx.shared_store.insert(RegisteredDeployAppCatalog(catalog));
}

/// Catálogo de aplicações de deploy instalado, se a área Servidores foi montada.
#[must_use]
pub fn deploy_app_catalog(ctx: &AppContext) -> Option<Arc<dyn McpDeployAppCatalog>> {
    ctx.shared_store
        .get::<RegisteredDeployAppCatalog>()
        .map(|registered| registered.0)
}
