//! Qual Docker Engine uma requisição quer: a desta central ou a de um agente.
//!
//! O controller só conhece a chave (`local` ou `agent-<id>`); aqui ela vira
//! um par de fontes ([`DockerEngine`] + [`DockerMaintenance`]). É o único ponto
//! que decide entre `LocalEngine` e o canal do agente — as regras de
//! mapeamento e auditoria não sabem de onde os dados vieram.

use std::{fmt, str::FromStr, sync::Arc};

use loco_rs::app::AppContext;

use crate::{
    services::{
        agents::{
            hub::AgentHub,
            policy::Permission,
            remote_engine::AgentEngine,
            service::{to_view, AgentService},
        },
        shared::errors::{AppError, AppResult},
    },
    views::agents::DockerHostView,
};

use super::{
    engine, maintenance::DockerMaintenance, metrics, source::DockerEngine, source::LocalEngine,
};

pub const LOCAL_HOST_KEY: &str = "local";
const AGENT_PREFIX: &str = "agent-";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostKey {
    Local,
    Agent(i64),
}

impl HostKey {
    #[must_use]
    pub const fn is_local(self) -> bool {
        matches!(self, Self::Local)
    }

    #[must_use]
    pub const fn agent(probe_id: i64) -> Self {
        Self::Agent(probe_id)
    }
}

impl fmt::Display for HostKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => f.write_str(LOCAL_HOST_KEY),
            Self::Agent(id) => write!(f, "{AGENT_PREFIX}{id}"),
        }
    }
}

impl FromStr for HostKey {
    type Err = AppError;

    fn from_str(value: &str) -> AppResult<Self> {
        let value = value.trim();
        if value == LOCAL_HOST_KEY {
            return Ok(Self::Local);
        }
        value
            .strip_prefix(AGENT_PREFIX)
            .and_then(|id| id.parse::<i64>().ok())
            .filter(|id| *id > 0)
            .map(Self::Agent)
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

impl DockerHost {
    #[must_use]
    pub fn local() -> Self {
        Self {
            key: HostKey::Local,
            engine: Arc::new(LocalEngine),
            maintenance: Arc::new(LocalEngine),
        }
    }

    /// Métricas do host. A local passa pelo cache do processo, que também
    /// serve o coletor SSE; a de um agente já chega calculada.
    pub async fn metrics(&self, ctx: &AppContext) -> crate::views::docker::DockerMetricsResponse {
        match self.key {
            HostKey::Local => metrics::overview(ctx).await,
            HostKey::Agent(_) => self.engine.metrics().await,
        }
    }
}

/// Resolve a chave para as fontes. Agente desconectado é indisponibilidade,
/// não "não encontrado": o host existe, só não está falando agora.
///
/// # Errors
///
/// `service_unavailable` quando o agente não está conectado a este processo.
pub fn resolve(ctx: &AppContext, key: HostKey) -> AppResult<DockerHost> {
    match key {
        HostKey::Local => Ok(DockerHost::local()),
        HostKey::Agent(probe_id) => {
            let session = AgentHub::from_context(ctx)?.get(probe_id).ok_or_else(|| {
                AppError::service_unavailable(
                    "O agente deste servidor não está conectado à central",
                )
            })?;
            let engine = Arc::new(AgentEngine::new(session));
            Ok(DockerHost {
                key,
                engine: engine.clone(),
                maintenance: engine,
            })
        }
    }
}

/// O que o host local oferece: sem compose, porque a central nunca executa o
/// CLI `docker` (AGENTS §7).
const LOCAL_PERMISSIONS: [Permission; 3] =
    [Permission::Read, Permission::Lifecycle, Permission::Update];

/// Hosts disponíveis para a tela: esta central e cada agente cadastrado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn list(ctx: &AppContext) -> AppResult<Vec<DockerHostView>> {
    let local_status = engine::status(&LocalEngine).await;
    let mut hosts = vec![DockerHostView {
        key: LOCAL_HOST_KEY.to_string(),
        name: "Este servidor".to_string(),
        kind: "local".to_string(),
        online: true,
        agent_id: None,
        docker_available: local_status.available,
        compose_available: false,
        policy: LOCAL_PERMISSIONS.to_vec(),
    }];
    let hub = AgentHub::from_context(ctx).ok();
    for agent in AgentService::new(&ctx.db).list().await? {
        let view = to_view(&agent, None, false);
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
        hosts.push(DockerHostView {
            key: view.host_key,
            name: view.name,
            kind: "agent".to_string(),
            online: session.is_some(),
            agent_id: Some(agent.id),
            docker_available,
            compose_available,
            policy,
        });
    }
    Ok(hosts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chave_de_host_faz_ida_e_volta() {
        assert_eq!("local".parse::<HostKey>().expect("local"), HostKey::Local);
        assert_eq!(
            "agent-12".parse::<HostKey>().expect("agente"),
            HostKey::Agent(12)
        );
        assert_eq!(HostKey::Agent(12).to_string(), "agent-12");
        assert_eq!(HostKey::Local.to_string(), "local");
    }

    #[test]
    fn chave_invalida_e_recusada() {
        for value in [
            "", "agent-", "agent-0", "agent--1", "agent-x", "remoto", "agent:3",
        ] {
            assert!(value.parse::<HostKey>().is_err(), "{value}");
        }
    }
}
