//! Agentes que podem servir de rota para um recurso (plugin, ponte de banco).
//!
//! Uma lista só para as duas telas: o que muda é a permissão que o agente
//! precisa ter anunciado.

use loco_rs::app::AppContext;

use super::{hub::AgentHub, policy::Permission, service::AgentService};
use crate::{dtos::agents::AgentRouteOption, services::shared::errors::AppResult};

/// Todos os agentes cadastrados, com o que cada um permite agora.
///
/// Desconectado conta como "permitido" até prova em contrário:
/// a política só é conhecida pelo `Hello`, e esconder o agente da lista
/// impediria escolher a rota antes de ele conectar.
///
/// # Errors
///
/// Erro do banco.
pub async fn route_options(
    ctx: &AppContext,
    permission: Permission,
) -> AppResult<Vec<AgentRouteOption>> {
    let hub = AgentHub::from_context(ctx).ok();
    Ok(AgentService::new(&ctx.db)
        .list()
        .await?
        .into_iter()
        .map(|agent| {
            let session = hub.as_ref().and_then(|hub| hub.get(agent.id));
            AgentRouteOption {
                id: agent.id,
                name: agent.name,
                connected: session.is_some(),
                allowed: session
                    .as_ref()
                    .is_none_or(|session| session.allows(permission)),
            }
        })
        .collect())
}
