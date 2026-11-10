//! Qual Docker Engine uma requisição quer.
//!
//! Todo host Docker é um agente (ADR 014): o desta central é o agente local,
//! num container ao lado dela; os demais são os agentes remotos (ADR 011). O
//! controller só conhece a chave (`agent-<id>`); aqui ela vira o par de
//! fontes ([`DockerEngine`] + [`DockerMaintenance`]) que fala pelo canal do
//! agente. As regras de mapeamento e auditoria não sabem de qual host vieram.

use std::sync::Arc;

use loco_rs::app::AppContext;

use netmonitor_core::{
    agents::{hub::AgentHub, identity},
    shared::errors::{AppError, AppResult},
};

use super::{
    builds::ImageHost, maintenance::DockerMaintenance, remote_engine::AgentEngine,
    source::DockerEngine, stacks::StackHost,
};
use crate::views::docker::DockerHostView;

pub use netmonitor_core::agents::HostKey;

/// Fontes resolvidas para um host.
#[derive(Clone)]
pub struct DockerHost {
    pub key: HostKey,
    pub engine: Arc<dyn DockerEngine>,
    pub maintenance: Arc<dyn DockerMaintenance>,
    /// Deploy contínuo de stacks (ADR 021).
    pub stacks: Arc<dyn StackHost>,
    /// Compilação e trânsito de imagens (ADR 022/023).
    pub images: Arc<dyn ImageHost>,
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
        maintenance: engine.clone(),
        stacks: engine.clone(),
        images: engine,
    })
}

/// O host desta central — o do agente local —, para quem não escolhe um
/// (a IA sem `host`, as menções do chat).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn default_key(ctx: &AppContext) -> AppResult<Option<HostKey>> {
    Ok(identity::find_local(&ctx.db)
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
    let mut agents = identity::list(&ctx.db).await?;
    agents.sort_by_key(|agent| !identity::is_local(agent));
    Ok(agents
        .iter()
        .map(|agent| {
            let local = identity::is_local(agent);
            let host = identity::host_info(agent);
            let session = hub.as_ref().and_then(|hub| hub.get(agent.id));
            let (docker_available, compose_available, policy) = session.as_ref().map_or_else(
                || (host.docker.available, host.compose.available, host.policy),
                |session| {
                    (
                        session.hello.docker.available,
                        session.hello.compose.available,
                        session.hello.policy.clone(),
                    )
                },
            );
            DockerHostView {
                key: HostKey::agent(agent.id).to_string(),
                name: agent.name.clone(),
                kind: if local { "local" } else { "agent" }.to_string(),
                online: session.is_some(),
                agent_id: agent.id,
                docker_available,
                compose_available,
                policy,
            }
        })
        .collect())
}
