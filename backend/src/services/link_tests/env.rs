//! O que o orquestrador usa do resto do sistema — atrás de contratos, para
//! que a máquina de estados seja testada sem equipamento, sem SSH e sem a
//! porta 2000 (inversão de dependência, AGENTS §10).
//!
//! * [`PluginInvoker`] — as ações `prepare`/`serve`/`stop`/`run` dos plugins
//!   (padrão: [`invoke::prepare`] + [`invoke::finish`]);
//! * [`DriverResolver`] — quem dirige pela central ([`LocalDriver`]) ou por um
//!   agente ([`agent_driver`], o `AgentDriver` do protocolo 4);
//! * [`BtestArmer`] — o servidor btest armado da central;
//! * [`CentralAddress`] — por qual endereço um equipamento alcança a central.

use std::{net::IpAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use sea_orm::DatabaseConnection;
use tokio::sync::oneshot;

use crate::{
    models::devices,
    services::{
        agents::throughput::agent_driver,
        events::EventBus,
        network_tools::throughput::{
            btest::{ArmedSession, BtestServer},
            driver::{LocalDriver, ThroughputDriver},
        },
        plugins::invoke::{self, RunOutcome, ServiceRunRequest},
        server_addresses,
        shared::errors::{AppError, AppResult},
    },
};

/// Chama uma ação de plugin num equipamento.
#[async_trait]
pub trait PluginInvoker: Send + Sync {
    /// Roda a ação até o fim. `started` recebe o `run_id` assim que a
    /// execução existe (a etapa mostra o transcript antes de terminar).
    ///
    /// # Errors
    ///
    /// O que impede a execução de existir (plugin inativo, credencial,
    /// parâmetros). Falha **da ação** vem no [`RunOutcome`].
    async fn execute(
        &self,
        request: ServiceRunRequest,
        started: Option<oneshot::Sender<i64>>,
    ) -> AppResult<RunOutcome>;
}

/// O padrão: a mesma execução da tela ([`invoke`]).
pub struct ContextInvoker {
    ctx: AppContext,
}

#[async_trait]
impl PluginInvoker for ContextInvoker {
    async fn execute(
        &self,
        request: ServiceRunRequest,
        started: Option<oneshot::Sender<i64>>,
    ) -> AppResult<RunOutcome> {
        let prepared = invoke::prepare(&self.ctx, request).await?;
        if let Some(started) = started {
            let _ = started.send(prepared.run_id());
        }
        invoke::finish(&self.ctx, prepared).await
    }
}

/// Quem dirige um teste nativo.
pub trait DriverResolver: Send + Sync {
    /// A própria central.
    fn central(&self) -> Arc<dyn ThroughputDriver>;

    /// Um agente remoto (ADR 015 §6).
    ///
    /// # Errors
    ///
    /// Agente desconectado ou sem a permissão `throughput`.
    fn agent(&self, agent_id: i64) -> AppResult<Arc<dyn ThroughputDriver>>;
}

/// O padrão: [`LocalDriver`] na central e [`agent_driver`] nos agentes.
pub struct DefaultDrivers {
    ctx: AppContext,
}

impl DriverResolver for DefaultDrivers {
    fn central(&self) -> Arc<dyn ThroughputDriver> {
        Arc::new(LocalDriver)
    }

    fn agent(&self, agent_id: i64) -> AppResult<Arc<dyn ThroughputDriver>> {
        agent_driver(&self.ctx, agent_id)
    }
}

/// O servidor btest da central, armado por teste.
pub trait BtestArmer: Send + Sync {
    /// O servidor está no ar neste processo.
    fn available(&self) -> bool;

    /// Porta em que ele escuta (a que o MikroTik deve discar).
    fn port(&self) -> u16;

    /// Arma uma sessão de uso único para `peer`.
    ///
    /// # Errors
    ///
    /// Servidor fora do ar, ou sessão já armada para o mesmo IP.
    fn arm(&self, peer: IpAddr, ttl: Duration) -> AppResult<ArmedSession>;
}

/// O padrão: o servidor aberto pelo `initializers::btest`, se houver.
pub struct ServerArmer(pub Option<BtestServer>);

impl BtestArmer for ServerArmer {
    fn available(&self) -> bool {
        self.0.is_some()
    }

    fn port(&self) -> u16 {
        self.0.as_ref().map_or(
            crate::services::network_tools::throughput::btest::DEFAULT_PORT,
            |server| server.local_addr().port(),
        )
    }

    fn arm(&self, peer: IpAddr, ttl: Duration) -> AppResult<ArmedSession> {
        self.0
            .as_ref()
            .ok_or_else(|| {
                AppError::ServiceUnavailable(
                    crate::services::network_tools::throughput::btest::UNAVAILABLE_MESSAGE
                        .to_owned(),
                )
            })?
            .arm(peer, ttl)
    }
}

/// Por qual endereço um equipamento alcança a central.
#[async_trait]
pub trait CentralAddress: Send + Sync {
    /// `None` quando não há como saber (a tela de endereços do servidor
    /// resolve).
    ///
    /// # Errors
    ///
    /// Erro do banco.
    async fn toward(&self, device: &devices::Model) -> AppResult<Option<String>>;
}

/// O padrão: a sugestão de `server_addresses` para o equipamento.
pub struct ServerAddresses {
    ctx: AppContext,
}

#[async_trait]
impl CentralAddress for ServerAddresses {
    async fn toward(&self, device: &devices::Model) -> AppResult<Option<String>> {
        let nat = server_addresses::nat_detector(&self.ctx);
        let list = server_addresses::list(&self.ctx.db, &nat).await?;
        let Some((id, _)) = server_addresses::suggest_for(&self.ctx.db, device, &list).await?
        else {
            return Ok(None);
        };
        Ok(list
            .into_iter()
            .find(|address| address.id == id)
            .and_then(|address| address.value))
    }
}

/// Tudo que o orquestrador usa, montado uma vez por pedido.
pub struct LinkTestEnv {
    pub db: DatabaseConnection,
    pub bus: Option<EventBus>,
    pub plugins: Arc<dyn PluginInvoker>,
    pub drivers: Arc<dyn DriverResolver>,
    pub btest: Arc<dyn BtestArmer>,
    pub central: Arc<dyn CentralAddress>,
}

impl LinkTestEnv {
    /// As implementações de produção, a partir do contexto do processo.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Arc<Self> {
        Arc::new(Self {
            db: ctx.db.clone(),
            bus: EventBus::from_context(ctx).ok(),
            plugins: Arc::new(ContextInvoker { ctx: ctx.clone() }),
            drivers: Arc::new(DefaultDrivers { ctx: ctx.clone() }),
            btest: Arc::new(ServerArmer(BtestServer::from_context(ctx).ok())),
            central: Arc::new(ServerAddresses { ctx: ctx.clone() }),
        })
    }
}
