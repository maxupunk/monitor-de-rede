//! Qual Docker Engine uma requisição quer.
//!
//! Todo host Docker é um agente (ADR 014): o desta central é o agente local,
//! num container ao lado dela; os demais são os agentes remotos (ADR 011). O
//! controller só conhece a chave (`agent-<id>`); aqui ela vira o par de
//! fontes ([`DockerEngine`] + [`DockerMaintenance`]) que fala pelo canal do
//! agente. As regras de mapeamento e auditoria não sabem de qual host vieram.

use std::{fmt, str::FromStr, sync::Arc};

use loco_rs::app::AppContext;

use crate::{
    services::{
        agents::{
            hub::AgentHub,
            local,
            remote_engine::AgentEngine,
            service::{to_view, AgentService},
        },
        shared::errors::{AppError, AppResult},
    },
    views::agents::DockerHostView,
};

use super::{maintenance::DockerMaintenance, source::DockerEngine};

const AGENT_PREFIX: &str = "agent-";

/// Chave de um host Docker: o id do agente que o atende.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostKey(i64);

impl HostKey {
    #[must_use]
    pub const fn agent(probe_id: i64) -> Self {
        Self(probe_id)
    }

    #[must_use]
    pub const fn probe_id(self) -> i64 {
        self.0
    }
}

impl fmt::Display for HostKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{AGENT_PREFIX}{}", self.0)
    }
}

impl FromStr for HostKey {
    type Err = AppError;

    fn from_str(value: &str) -> AppResult<Self> {
        value
            .trim()
            .strip_prefix(AGENT_PREFIX)
            .and_then(|id| id.parse::<i64>().ok())
            .filter(|id| *id > 0)
            .map(Self)
            .ok_or_else(|| AppError::validation("Host Docker inválido"))
    }
}

/// Fontes resolvidas para um host.
#[derive(Clone)]
pub struct DockerHost {
    pub key: HostKey,
    pub engine: Arc<dyn DockerEngine>,
    pub maintenance: Arc<dyn DockerMaintenance>,
}

/// Resolve a chave para as fontes. Agente desconectado é indisponibilidade,
/// não "não encontrado": o host existe, só não está falando agora.
///
/// # Errors
///
/// `service_unavailable` quando o agente não está conectado a este processo.
pub fn resolve(ctx: &AppContext, key: HostKey) -> AppResult<DockerHost> {
    let session = AgentHub::from_context(ctx)?
        .get(key.probe_id())
        .ok_or_else(|| {
            AppError::service_unavailable("O agente deste servidor não está conectado à central")
        })?;
    let engine = Arc::new(AgentEngine::new(session));
    Ok(DockerHost {
        key,
        engine: engine.clone(),
        maintenance: engine,
    })
}

/// O host desta central — o do agente local —, para quem não escolhe um
/// (a IA sem `host`, as menções do chat).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn default_key(ctx: &AppContext) -> AppResult<Option<HostKey>> {
    Ok(local::find(&ctx.db)
        .await?
        .map(|probe| HostKey::agent(probe.id)))
}

/// As fontes do host desta central, quando o agente local existe e está
/// conectado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn default_host(ctx: &AppContext) -> AppResult<Option<DockerHost>> {
    Ok(default_key(ctx)
        .await?
        .and_then(|key| resolve(ctx, key).ok()))
}

/// Hosts disponíveis para a tela: o agente local primeiro, depois os remotos
/// pelo nome.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn list(ctx: &AppContext) -> AppResult<Vec<DockerHostView>> {
    let hub = AgentHub::from_context(ctx).ok();
    let mut agents = AgentService::new(&ctx.db).list().await?;
    agents.sort_by_key(|agent| !local::is_local(agent));
    Ok(agents
        .iter()
        .map(|agent| {
            let view = to_view(agent, None, false);
            let session = hub.as_ref().and_then(|hub| hub.get(agent.id));
            let (docker_available, compose_available, policy) = session.as_ref().map_or_else(
                || {
                    (
                        view.host.docker.available,
                        view.host.compose.available,
                        view.host.policy.clone(),
                    )
                },
                |session| {
                    (
                        session.hello.docker.available,
                        session.hello.compose.available,
                        session.hello.policy.clone(),
                    )
                },
            );
            DockerHostView {
                key: view.host_key,
                name: view.name,
                kind: if view.local { "local" } else { "agent" }.to_string(),
                online: session.is_some(),
                agent_id: agent.id,
                docker_available,
                compose_available,
                policy,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chave_de_host_faz_ida_e_volta() {
        assert_eq!(
            "agent-12".parse::<HostKey>().expect("agente"),
            HostKey::agent(12)
        );
        assert_eq!(HostKey::agent(12).to_string(), "agent-12");
        assert_eq!(HostKey::agent(12).probe_id(), 12);
    }

    #[test]
    fn chave_invalida_e_recusada() {
        for value in [
            "", "local", "agent-", "agent-0", "agent--1", "agent-x", "remoto", "agent:3",
        ] {
            assert!(value.parse::<HostKey>().is_err(), "{value}");
        }
    }
}
