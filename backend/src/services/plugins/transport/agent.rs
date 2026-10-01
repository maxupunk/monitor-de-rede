//! Encaminha a E/S do plugin a um agente remoto (ADR 011).
//!
//! O agente só disca para fora: a central não abre conexão com ele, usa o
//! canal que ele mantém. E o agente decide — `Permission::DeviceIo` precisa
//! estar no `AGENT_ALLOW` do host, e o alvo precisa ser da rede privada. A
//! checagem de permissão aqui só evita mandar um pedido que seria recusado.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;

use super::{DeviceIoCall, DeviceIoReply, DeviceTransport};
use crate::services::{
    agents::{policy::Permission, protocol::Command, session::AgentSession},
    shared::errors::{AppError, AppResult},
};

/// Folga sobre o timeout da própria chamada para a volta pelo túnel.
const GRACE: Duration = Duration::from_secs(10);

pub struct AgentTransport {
    session: Arc<AgentSession>,
}

impl AgentTransport {
    /// # Errors
    ///
    /// O agente não anunciou `device_io` na política.
    pub fn new(session: Arc<AgentSession>) -> AppResult<Self> {
        if !session.allows(Permission::DeviceIo) {
            return Err(AppError::business_rule(format!(
                "O agente {} não libera acesso a equipamentos. Inclua `device_io` no AGENT_ALLOW \
                 daquele host para usar plugins pela rede dele.",
                session.name
            )));
        }
        Ok(Self { session })
    }
}

#[async_trait]
impl DeviceTransport for AgentTransport {
    async fn execute(&self, call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
        let timeout = Duration::from_millis(call.timeout_ms()) + GRACE;
        let value = self
            .session
            .request(Command::DeviceIo { call: call.clone() }, timeout)
            .await
            .map_err(|error| {
                AppError::business_rule(format!("Agente {}: {error}", self.session.name))
            })?;
        serde_json::from_value(value).map_err(|error| {
            AppError::Internal(anyhow::anyhow!("resposta inválida do agente: {error}"))
        })
    }

    fn origin(&self) -> String {
        format!("agente {}", self.session.name)
    }
}
