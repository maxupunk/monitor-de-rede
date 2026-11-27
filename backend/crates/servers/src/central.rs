//! O que a área Servidores pede ao resto da central (ADR 017).
//!
//! O deploy contínuo avisa pelos canais de notificação (Telegram, Discord,
//! e-mail, webhook) e por Web Push a quem tem acesso à área; o registry
//! entrega as vulnerabilidades das imagens ao motor de alertas — peças do
//! núcleo que esta área não pode nomear. O ponto de montagem implementa
//! [`CentralServices`] e o põe no `shared_store`, em todo processo.

use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    alerts::contracts::AlertEvaluationContext,
    notifications::NotificationMessage,
    shared::errors::{AppError, AppResult},
};
use sea_orm::DatabaseConnection;
use serde_json::Value;

#[async_trait]
pub trait CentralServices: Send + Sync {
    /// Envia pelos canais de notificação padrão.
    async fn notify(&self, ctx: &AppContext, message: &NotificationMessage);

    /// Web Push a quem tem acesso à área Servidores (ADR 019). Devolve quantos
    /// aparelhos receberam.
    async fn push_to_servers(&self, db: &DatabaseConnection, payload: &Value) -> AppResult<u64>;

    /// Entrega um conjunto de fatos ao motor de alertas, que decide pelas
    /// regras configuradas se abre, mantém ou fecha um alerta.
    async fn evaluate_alert(
        &self,
        ctx: &AppContext,
        context: &AlertEvaluationContext,
    ) -> AppResult<()>;

    /// Cria, uma vez na vida da instalação, as regras de vulnerabilidade nas
    /// imagens — chamado quando a primeira vulnerabilidade aparece.
    async fn ensure_image_vulnerability_rules(&self, db: &DatabaseConnection) -> AppResult<()>;
}

/// O [`CentralServices`] do processo, guardado no `shared_store`.
#[derive(Clone)]
pub struct Central(Arc<dyn CentralServices>);

impl Central {
    #[must_use]
    pub fn new(services: Arc<dyn CentralServices>) -> Self {
        Self(services)
    }

    /// # Errors
    ///
    /// `Internal` quando o ponto de montagem não registrou os serviços.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store.get::<Self>().ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "Serviços da central para a área Servidores não inicializados"
            ))
        })
    }
}

impl Deref for Central {
    type Target = dyn CentralServices;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
