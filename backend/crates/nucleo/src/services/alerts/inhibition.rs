//! Inibição por dependência (Fase 4 do roadmap, §5.1 — tempestade em cascata).
//!
//! Um roteador cai e os 200 dispositivos atrás dele param de responder. Os 200
//! alertas estão **corretos** — e são inúteis: o operador precisa saber do
//! roteador, e os filhos são consequência. Aqui fica a pergunta que separa uma
//! coisa da outra: *algum ancestral deste dispositivo está em alerta agora?*
//!
//! **A hierarquia é `devices.parent_id`, não `device_links`** — e a escolha é
//! deliberada. `device_links` é topologia *descoberta* (LLDP, CDP, inferência
//! por sub-rede) e não é direcionada: um enlace diz que dois equipamentos se
//! enxergam, não qual depende de qual. Suprimir por enlace calaria o vizinho
//! junto com o filho. `parent_id` é hierarquia **declarada pelo operador**, tem
//! direção e já é o que a topologia desenha como aresta de hierarquia
//! (`topology/service.rs`).
//!
//! A supressão nunca é decidida no enfileiramento: ver
//! [`super::super::notifications::policy::INHIBITION_GRACE_SECONDS`].

use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, devices},
    services::{alerts::contracts::AlertStatus, shared::errors::AppResult},
};

/// Teto da subida na hierarquia.
///
/// Uma rede real não empilha oito níveis de pai declarado, e o limite protege
/// contra o ciclo que um cadastro errado pode criar (A pai de B, B pai de A) —
/// o conjunto de visitados já barraria o laço, mas o teto também limita o
/// número de consultas a um valor conhecido.
pub const MAX_DEPTH: usize = 8;

/// Os status de evento aberto que **explicam** a queda de um filho.
///
/// `Recovering` fica de fora de propósito: o pai já voltou e está apenas sob
/// observação de estabilidade — ele não explica mais nada, e usá-lo para calar
/// o filho esconderia um problema real.
const EXPLAINING: [AlertStatus; 4] = [
    AlertStatus::Active,
    AlertStatus::Acknowledged,
    AlertStatus::Silenced,
    AlertStatus::Flapping,
];

/// A cadeia de ancestrais declarados, do pai imediato para cima.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn ancestors<C: ConnectionTrait>(db: &C, device_id: i64) -> AppResult<Vec<i64>> {
    let mut chain = Vec::new();
    let mut visited = HashSet::from([device_id]);
    let mut current = device_id;

    while chain.len() < MAX_DEPTH {
        let Some(device) = devices::Entity::find_by_id(current).one(db).await? else {
            break;
        };
        let Some(parent) = device.parent_id else {
            break;
        };
        // Cadastro em ciclo não pode virar laço infinito nem consulta repetida.
        if !visited.insert(parent) {
            tracing::warn!(
                device_id,
                parent,
                "hierarquia de dispositivos em ciclo; inibição interrompida"
            );
            break;
        }
        chain.push(parent);
        current = parent;
    }
    Ok(chain)
}

/// O ancestral que está em alerta agora, se houver.
///
/// Devolve o **mais próximo** na cadeia: é ele que o operador vai olhar
/// primeiro. `None` significa "nada acima explica esta queda" — a notificação
/// do filho é legítima e sai.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn explaining_ancestor<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
) -> AppResult<Option<i64>> {
    let chain = ancestors(db, device_id).await?;
    if chain.is_empty() {
        return Ok(None);
    }

    let in_alarm: HashSet<i64> = alert_events::Entity::find()
        .filter(alert_events_entity::Column::DeviceId.is_in(chain.clone()))
        .filter(alert_events_entity::Column::Status.is_in(EXPLAINING))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|event| event.device_id)
        .collect();

    Ok(chain.into_iter().find(|id| in_alarm.contains(id)))
}

/// Resolve em lote o ancestral mais próximo em alerta para vários dispositivos,
/// evitando N+1 durante tempestades em cascata no despacho do outbox.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn explaining_ancestors<C: ConnectionTrait>(
    db: &C,
    device_ids: &[i64],
) -> AppResult<HashMap<i64, i64>> {
    let unique_ids: Vec<i64> = {
        let mut seen = HashSet::with_capacity(device_ids.len());
        device_ids
            .iter()
            .copied()
            .filter(|id| seen.insert(*id))
            .collect()
    };
    if unique_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Sobe a hierarquia por ondas em lote (no máximo `MAX_DEPTH` rodadas).
    let mut parent_by_device: HashMap<i64, Option<i64>> = HashMap::new();
    let mut frontier = unique_ids.clone();
    for _ in 0..MAX_DEPTH {
        if frontier.is_empty() {
            break;
        }
        let mut next_frontier = Vec::new();
        for chunk in frontier.chunks(400) {
            let rows: Vec<(i64, Option<i64>)> = devices::Entity::find()
                .select_only()
                .column(devices::Column::Id)
                .column(devices::Column::ParentId)
                .filter(devices::Column::Id.is_in(chunk.to_vec()))
                .into_tuple()
                .all(db)
                .await?;
            for (id, parent_id) in rows {
                parent_by_device.insert(id, parent_id);
                if let Some(parent) = parent_id {
                    if !parent_by_device.contains_key(&parent) {
                        next_frontier.push(parent);
                    }
                }
            }
        }
        next_frontier.sort_unstable();
        next_frontier.dedup();
        frontier = next_frontier;
    }

    let mut chains: HashMap<i64, Vec<i64>> = HashMap::with_capacity(unique_ids.len());
    let mut all_ancestors: HashSet<i64> = HashSet::new();
    for device_id in unique_ids {
        let mut chain = Vec::new();
        let mut visited = HashSet::from([device_id]);
        let mut current = device_id;
        while chain.len() < MAX_DEPTH {
            let Some(Some(parent)) = parent_by_device.get(&current).copied() else {
                break;
            };
            if !visited.insert(parent) {
                tracing::warn!(
                    device_id,
                    parent,
                    "hierarquia de dispositivos em ciclo; inibição interrompida"
                );
                break;
            }
            chain.push(parent);
            all_ancestors.insert(parent);
            current = parent;
        }
        if !chain.is_empty() {
            chains.insert(device_id, chain);
        }
    }

    if all_ancestors.is_empty() {
        return Ok(HashMap::new());
    }

    let ancestor_list: Vec<i64> = all_ancestors.into_iter().collect();
    let mut in_alarm: HashSet<i64> = HashSet::new();
    for chunk in ancestor_list.chunks(400) {
        let rows: Vec<Option<i64>> = alert_events::Entity::find()
            .select_only()
            .column(alert_events_entity::Column::DeviceId)
            .filter(alert_events_entity::Column::DeviceId.is_in(chunk.to_vec()))
            .filter(alert_events_entity::Column::Status.is_in(EXPLAINING))
            .into_tuple()
            .all(db)
            .await?;
        for device_id in rows.into_iter().flatten() {
            in_alarm.insert(device_id);
        }
    }

    let mut result = HashMap::new();
    for (device_id, chain) in chains {
        if let Some(ancestor) = chain.into_iter().find(|id| in_alarm.contains(id)) {
            result.insert(device_id, ancestor);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovering_nao_explica_a_queda_do_filho() {
        // O pai voltou e só observa estabilidade: calar o filho aqui esconderia
        // um problema que continua de pé.
        assert!(!EXPLAINING.contains(&AlertStatus::Recovering));
        assert!(!EXPLAINING.contains(&AlertStatus::Resolved));
        // Todo o resto dos estados abertos explica.
        for status in AlertStatus::OPEN {
            assert_eq!(
                EXPLAINING.contains(&status),
                status != AlertStatus::Recovering,
                "{status} classificado errado"
            );
        }
    }

    #[test]
    fn o_teto_de_profundidade_e_conhecido() {
        // O número aparece no comentário do módulo e nos testes de integração:
        // mudá-lo em silêncio mudaria quantas consultas a inibição faz por
        // mensagem.
        assert_eq!(MAX_DEPTH, 8);
    }
}
