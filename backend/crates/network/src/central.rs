//! O que a área Rede pede ao resto da central (ADR 017).
//!
//! Peças que não são do núcleo nem da área: o motor de alertas (que avalia a
//! transição de cada interface observada pelo SNMP e de cada cliente Wi-Fi
//! vigiado, e as regras desses clientes), os endereços da central (o que o
//! equipamento alcança num teste de enlace), a identificação de
//! dispositivo (que os plugins pedem ao observar o equipamento), a porta
//! publicada da API (o espelho de pacotes) e a IA configurada (a revisão de
//! segurança de um plugin importado). O ponto de montagem
//! implementa [`CentralServices`] e o põe no `shared_store` (`after_context`),
//! em todo processo — o scheduler, que roda a coleta SNMP, inclusive.

use std::{ops::Deref, sync::Arc, time::Duration};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    alerts::contracts::AlertEvaluationContext,
    models::devices,
    shared::errors::{AppError, AppResult},
};
use sea_orm::DatabaseConnection;

#[async_trait]
pub trait CentralServices: Send + Sync {
    /// Entrega um conjunto de fatos ao motor de alertas, que decide pelas
    /// regras configuradas se abre, mantém ou fecha um alerta.
    async fn evaluate_alert(
        &self,
        ctx: &AppContext,
        context: &AlertEvaluationContext,
    ) -> AppResult<()>;

    /// Observa o equipamento (SSH, SNMP e o Decisor, como a identificação
    /// de dispositivo) e devolve o cadastro com o sistema observado.
    async fn observe_device(
        &self,
        ctx: &AppContext,
        device: &devices::Model,
        ssh_port: Option<u16>,
    ) -> AppResult<devices::Model>;

    /// Fecha os alertas abertos de um alvo (`scope_key`) com o motivo.
    /// Devolve quantos fechou.
    async fn close_alert_scope(
        &self,
        ctx: &AppContext,
        scope_key: &str,
        reason: &str,
    ) -> AppResult<usize>;

    /// Aplica, uma vez na vida da instalação, as regras dos clientes Wi-Fi
    /// vigiados (quando o primeiro aparelho é vigiado).
    async fn ensure_wifi_watch_rules(&self, db: &DatabaseConnection) -> AppResult<()>;

    /// O endereço da central que um equipamento alcança (a sugestão dos
    /// endereços do servidor). `None` quando não há como saber.
    async fn address_toward(
        &self,
        ctx: &AppContext,
        device: &devices::Model,
    ) -> AppResult<Option<String>>;

    /// A porta publicada da API, por onde o equipamento alcança a central
    /// (o espelho de pacotes dos plugins).
    fn published_api_port(&self, ctx: &AppContext) -> u16;

    /// Uma leitura da IA configurada, sem ferramentas: o texto e o modelo
    /// que respondeu. `Ok(None)` com o assistente desligado; `Err` com a
    /// mensagem da falha (configuração, provedor ou o `timeout` da resposta).
    async fn ask_ai(
        &self,
        db: &DatabaseConnection,
        system: &str,
        user: &str,
        max_tokens: u32,
        timeout: Duration,
    ) -> Result<Option<(String, Option<String>)>, String>;
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
