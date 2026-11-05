//! O que o assistente pede ao resto da central (ADR 017).
//!
//! Peças que não são do núcleo nem do assistente: o Web Push (pedido de
//! aprovação a quem não está olhando), os canais de notificação (o resumo
//! periódico), a inibição topológica dos alertas (a triagem), os plugins de
//! dispositivo (o "Aceitar automaticamente" e a falta de credencial) e o
//! Docker dos hosts (marcações e nomes de container). O ponto de montagem
//! implementa [`CentralServices`] e o põe no `shared_store` (`after_context`),
//! em todo processo — as rotinas automáticas do servidor inclusive.

use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    notifications::NotificationMessage,
    shared::errors::{AppError, AppResult},
};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::mentions::ContainerDirectory;

#[async_trait]
pub trait CentralServices: Send + Sync {
    /// Manda um Web Push aos aparelhos inscritos do usuário. Devolve quantos
    /// receberam.
    async fn push_to_user(
        &self,
        db: &DatabaseConnection,
        user_id: i64,
        payload: &Value,
    ) -> AppResult<u64>;

    /// Envia pelos canais de notificação padrão (o resumo periódico).
    async fn notify(&self, ctx: &AppContext, message: &NotificationMessage);

    /// O ancestral topológico cujo alerta aberto explica o do dispositivo
    /// (inibição dos alertas).
    async fn explaining_ancestor(
        &self,
        db: &DatabaseConnection,
        device_id: i64,
    ) -> AppResult<Option<i64>>;

    /// A falta de credencial de acesso a um equipamento num erro de
    /// ferramenta: o dispositivo e o transporte, para o cartão oferecer
    /// cadastrá-la ali mesmo.
    fn missing_device_access(&self, error: &AppError) -> Option<(i64, Value)>;

    /// O modo "Aceitar automaticamente" está ligado para esta conversa e este
    /// equipamento (ADR 012)?
    async fn device_auto_accepted(
        &self,
        db: &DatabaseConnection,
        conversation_key: &str,
        device_id: i64,
    ) -> bool;

    /// Os nomes de todo container ao alcance — o host desta central e os
    /// agentes conectados —, sem repetição.
    async fn container_names(&self, ctx: &AppContext) -> Vec<String>;

    /// Os containers do host desta central (o agente local), quando ele existe.
    async fn local_containers(&self, ctx: &AppContext) -> Option<Box<dyn ContainerDirectory>>;
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
                "Serviços da central para o assistente não inicializados"
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
