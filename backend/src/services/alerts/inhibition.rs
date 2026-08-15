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

use std::collections::HashSet;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

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
