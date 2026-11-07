//! Quem é agente e quem é o agente local — leituras do cadastro (`probes`)
//! que o núcleo e as áreas compartilham. O cadastro em si (criar, revogar,
//! token) continua no serviço de agentes.

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;

use super::views::AgentHostInfo;
use crate::{models::probes, shared::errors::AppResult};

/// Papel gravado em `probes.configuration.role` de um agente remoto.
pub const ROLE_AGENT: &str = "agent";

/// O probe é um agente (e não um probe de rede antigo)?
#[must_use]
pub fn is_agent(probe: &probes::Model) -> bool {
    probe
        .configuration
        .as_ref()
        .and_then(|value| value.get("role"))
        .and_then(Value::as_str)
        == Some(ROLE_AGENT)
}

/// O agente é o local desta central?
#[must_use]
pub fn is_local(probe: &probes::Model) -> bool {
    probe
        .configuration
        .as_ref()
        .and_then(|value| value.get("local"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// O agente local cadastrado, se houver.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn find_local<C: ConnectionTrait>(db: &C) -> AppResult<Option<probes::Model>> {
    Ok(probes::Entity::find()
        .filter(probes::Column::Status.ne(probes::STATUS_REVOKED))
        .order_by_asc(probes::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .find(|probe| is_agent(probe) && is_local(probe)))
}

/// Os agentes cadastrados, pelo nome.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<probes::Model>> {
    Ok(probes::Entity::find()
        .order_by_asc(probes::Column::Name)
        .all(db)
        .await?
        .into_iter()
        .filter(is_agent)
        .collect())
}

/// O que a última conexão do agente anunciou (`probes.configuration.host`).
#[must_use]
pub fn host_info(probe: &probes::Model) -> AgentHostInfo {
    probe
        .configuration
        .as_ref()
        .and_then(|value| value.get("host"))
        .cloned()
        .and_then(|value| serde_json::from_value::<AgentHostInfo>(value).ok())
        .unwrap_or_default()
}
