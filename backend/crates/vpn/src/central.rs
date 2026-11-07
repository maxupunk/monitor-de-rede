//! O que a área VPN pede ao resto da central (ADR 017).
//!
//! Três peças que não são do núcleo nem da área: o motor de alertas (que
//! avalia o dataset de cada túnel que mudou de estado), a apresentação de um dispositivo (a
//! mesma das telas de dispositivos) e a lista de endereços da central (que
//! esquece uma correção manual quando ela vira cópia do `public_endpoint`).
//! O ponto de montagem implementa [`CentralServices`] e o põe no
//! `shared_store` (`after_context`), em todo processo — o scheduler, que roda
//! a telemetria dos túneis, inclusive.

use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    alerts::contracts::AlertEvaluationContext,
    models::devices,
    shared::errors::{AppError, AppResult},
};
use sea_orm::DatabaseConnection;
use serde_json::Value;

#[async_trait]
pub trait CentralServices: Send + Sync {
    /// Entrega um conjunto de fatos ao motor de alertas, que decide pelas
    /// regras configuradas se abre, mantém ou fecha um alerta.
    async fn evaluate_alert(
        &self,
        ctx: &AppContext,
        context: &AlertEvaluationContext,
    ) -> AppResult<()>;

    /// O dispositivo do peer como as telas da VPN o mostram — sem o próprio
    /// vínculo de VPN. `None` quando não serializa.
    async fn present_device(
        &self,
        db: &DatabaseConnection,
        device: devices::Model,
    ) -> AppResult<Option<Value>>;

    /// Apaga a correção manual do endereço `kind` se ela for igual a
    /// `detected`. Devolve se algo foi apagado.
    async fn forget_address_override(
        &self,
        db: &DatabaseConnection,
        kind: &str,
        detected: &str,
    ) -> AppResult<bool>;
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
                "Serviços da central para a VPN não inicializados"
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
