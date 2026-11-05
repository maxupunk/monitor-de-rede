//! O que a área Rede pede ao resto da central (ADR 017).
//!
//! Peças que não são do núcleo nem da área: o motor de alertas (que avalia a
//! transição de cada interface observada pelo SNMP). O ponto de montagem
//! implementa [`CentralServices`] e o põe no `shared_store` (`after_context`),
//! em todo processo — o scheduler, que roda a coleta SNMP, inclusive.

use std::{ops::Deref, sync::Arc};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    alerts::contracts::AlertEvaluationContext,
    shared::errors::{AppError, AppResult},
};

#[async_trait]
pub trait CentralServices: Send + Sync {
    /// Entrega um conjunto de fatos ao motor de alertas, que decide pelas
    /// regras configuradas se abre, mantém ou fecha um alerta.
    async fn evaluate_alert(
        &self,
        ctx: &AppContext,
        context: &AlertEvaluationContext,
    ) -> AppResult<()>;
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
                "Serviços da central para a Rede não inicializados"
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
