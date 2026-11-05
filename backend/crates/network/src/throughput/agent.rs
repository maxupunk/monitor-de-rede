//! Um agente remoto como origem de teste de vazão (ADR 015 §6).
//!
//! [`AgentDriver`] implementa o mesmo [`ThroughputDriver`] do `LocalDriver`:
//! o orquestrador não sabe de onde o teste parte. O pedido vai como
//! `Command::Throughput` pelo canal que o agente mantém — a central nunca o
//! chama, e o agente só **dirige** (cliente), nunca serve. As amostras chegam
//! em `Chunk` e vão para o `SampleSink`; cancelar manda `Cancel`.
//!
//! O agente decide: `throughput` precisa estar no `AGENT_ALLOW` do host e o
//! alvo, numa rede que ele aceite. As checagens daqui só evitam um pedido que
//! seria recusado.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use tokio_util::sync::CancellationToken;

use agent_protocol::{
    policy::Permission,
    {Command, ErrorCode, RemoteError},
};
use netmonitor_core::agents::{hub::AgentHub, session::AgentSession};

use crate::{
    plugins::transport::is_reachable_target,
    throughput::{
        cancelled,
        driver::{DriveRequest, ThroughputDriver},
        SampleSink, ThroughputReport, ThroughputSample, ThroughputSpecExt, DEADLINE_MARGIN,
    },
};
use netmonitor_core::shared::errors::{AppError, AppResult};

/// Folga sobre o prazo dos motores para a volta pelo canal.
const GRACE: Duration = Duration::from_secs(10);

/// Dirige o teste a partir de um agente conectado.
pub struct AgentDriver {
    session: Arc<AgentSession>,
}

impl AgentDriver {
    /// # Errors
    ///
    /// O agente não anunciou `throughput` na política.
    pub fn new(session: Arc<AgentSession>) -> AppResult<Self> {
        if !session.allows(Permission::Throughput) {
            return Err(not_allowed(&session.name));
        }
        Ok(Self { session })
    }

    /// Erro do agente com a mensagem que a tela mostra. Regra de negócio do
    /// motor volta com o texto exato (EC-SRP5 é reconhecido por ele).
    pub fn map_error(&self, error: RemoteError) -> AppError {
        let name = &self.session.name;
        match error.code {
            ErrorCode::Disconnected => AppError::service_unavailable(format!(
                "O agente {name} se desconectou durante o teste de vazão"
            )),
            ErrorCode::Timeout => AppError::service_unavailable(format!(
                "O agente {name} não concluiu o teste de vazão no prazo"
            )),
            ErrorCode::Forbidden if !self.session.allows(Permission::Throughput) => {
                not_allowed(name)
            }
            ErrorCode::Forbidden => {
                AppError::business_rule(format!("O agente {name} recusou o teste: {error}"))
            }
            ErrorCode::Unavailable => AppError::service_unavailable(error.message),
            ErrorCode::Conflict => AppError::conflict(error.message),
            ErrorCode::Validation => AppError::validation(error.message),
            ErrorCode::NotFound => AppError::not_found(error.message),
            ErrorCode::Engine => AppError::business_rule(error.message),
            ErrorCode::Disabled | ErrorCode::Unsupported | ErrorCode::Internal => {
                AppError::business_rule(format!("Agente {name}: {error}"))
            }
        }
    }
}

fn not_allowed(name: &str) -> AppError {
    AppError::business_rule(format!(
        "O agente {name} não permite testes de vazão: adicione `throughput` ao AGENT_ALLOW \
         daquele host"
    ))
}

/// Prazo do pedido inteiro: cada sentido do iperf3 é uma fase com o próprio
/// prazo (`duração + margem`), mais a volta pelo canal.
pub fn request_timeout(request: &DriveRequest) -> Duration {
    let direction = request.spec.direction;
    let phases = u32::from(direction.sends()) + u32::from(direction.receives());
    (request.spec.duration() + DEADLINE_MARGIN) * phases.max(1) + GRACE
}

#[async_trait]
impl ThroughputDriver for AgentDriver {
    async fn drive(
        &self,
        request: &DriveRequest,
        sink: &SampleSink,
        cancel: &CancellationToken,
    ) -> AppResult<ThroughputReport> {
        request.spec.validate()?;
        let ip = request.target.ip();
        if !is_reachable_target(ip) {
            return Err(AppError::validation(format!(
                "Um agente só testa até equipamentos de rede privada: {ip} é endereço público"
            )));
        }
        let result = self
            .session
            .stream(
                Command::from(request),
                request_timeout(request),
                cancel,
                |chunk| {
                    if let Ok(sample) = serde_json::from_value::<ThroughputSample>(chunk) {
                        let _ = sink.send(sample);
                    }
                },
            )
            .await;
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        let value = result.map_err(|error| self.map_error(error))?;
        serde_json::from_value(value).map_err(|error| {
            AppError::Internal(anyhow::anyhow!("relatório inválido do agente: {error}"))
        })
    }

    fn origin(&self) -> String {
        format!("agente {}", self.session.name)
    }
}

/// O agente `probe_id` como origem de teste de vazão.
///
/// # Errors
///
/// Agente desconectado (`service_unavailable`) ou sem `throughput` no
/// `AGENT_ALLOW`.
pub fn agent_driver(ctx: &AppContext, probe_id: i64) -> AppResult<Arc<dyn ThroughputDriver>> {
    let hub = AgentHub::from_context(ctx).map_err(|_| disconnected(probe_id))?;
    from_hub(&hub, probe_id)
}

pub fn from_hub(hub: &AgentHub, probe_id: i64) -> AppResult<Arc<dyn ThroughputDriver>> {
    let session = hub
        .get(probe_id)
        .filter(|session| !session.is_closed())
        .ok_or_else(|| disconnected(probe_id))?;
    Ok(Arc::new(AgentDriver::new(session)?))
}

fn disconnected(probe_id: i64) -> AppError {
    AppError::service_unavailable(format!(
        "O agente #{probe_id} não está conectado à central: o teste de vazão parte dele"
    ))
}
