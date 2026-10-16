//! Deduplicação e persistência dos enlaces físicos/lógicos.

use std::collections::BTreeMap;

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};

use crate::{
    models::device_links,
    services::{
        shared::{errors::AppResult, transaction::begin_write},
        topology::edge_state::OBSERVED_METHODS,
    },
};

#[derive(Debug, Clone)]
pub struct NetworkLink {
    pub source_device_id: i64,
    pub target_device_id: i64,
    pub source_interface_id: Option<i64>,
    pub target_interface_id: Option<i64>,
    pub link_type: String,
    pub discovery_method: String,
    pub confidence: i32,
    pub confirmed: bool,
}

#[derive(Debug)]
pub struct PersistedLinks {
    pub links: Vec<device_links::Model>,
    pub created: usize,
    pub updated: usize,
}

#[must_use]
pub fn resolve_links(raw: Vec<NetworkLink>) -> Vec<NetworkLink> {
    let mut deduplicated = BTreeMap::<(i64, i64), NetworkLink>::new();
    for link in raw
        .into_iter()
        .filter(|link| link.source_device_id != link.target_device_id)
    {
        let key = (
            link.source_device_id.min(link.target_device_id),
            link.source_device_id.max(link.target_device_id),
        );
        if deduplicated
            .get(&key)
            .is_none_or(|current| current.confidence < link.confidence)
        {
            deduplicated.insert(key, link);
        }
    }
    deduplicated.into_values().collect()
}

/// O enlace mudou de forma que interessa ao grafo?
///
/// `last_seen_at` fica **de fora** de propósito (matriz de paridade #49): ele é
/// prova de vida e avança em todo ciclo de descoberta. Contá-lo como alteração
/// faria a topologia reportar "N enlaces atualizados" a cada varredura, com o
/// desenho idêntico — e o operador perderia a única pista de que algo de fato
/// mudou no cabeamento.
#[must_use]
pub fn is_material_change(row: &device_links::Model, link: &NetworkLink) -> bool {
    row.source_interface_id != link.source_interface_id
        || row.target_interface_id != link.target_interface_id
        || row.link_type != link.link_type
        || row.discovery_method != link.discovery_method
        || row.confidence != link.confidence
        || row.confirmed != link.confirmed
}

/// O que fazer com um enlace que chegou, dada a linha já gravada para o par.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpsertPlan {
    /// Par inédito: grava o enlace.
    Insert,
    /// O enlace que chegou vale pelo menos o gravado: substitui.
    Overwrite { changed: bool },
    /// O gravado vale mais (manual, confirmado ou mais confiável): **não** é
    /// rebaixado. Só completa interfaces que ele não tinha e, quando o enlace
    /// que chegou é observação do cabeamento, renova a prova de vida.
    Keep {
        source_interface_id: Option<i64>,
        target_interface_id: Option<i64>,
        touch_last_seen: bool,
    },
}

/// Decide como um enlace que chegou afeta a linha existente do mesmo par.
///
/// A regra é "nunca rebaixar": uma varredura LLDP (95, não confirmada) que
/// encontra o enlace que o operador desenhou à mão (100, confirmado) não troca
/// o tipo, o método nem a confiança — só confirma que o cabo continua lá.
#[must_use]
pub fn plan_upsert(row: Option<&device_links::Model>, link: &NetworkLink) -> UpsertPlan {
    let Some(row) = row else {
        return UpsertPlan::Insert;
    };
    let downgrade = link.confidence < row.confidence || (row.confirmed && !link.confirmed);
    if !downgrade {
        return UpsertPlan::Overwrite {
            changed: is_material_change(row, link),
        };
    }
    // As interfaces vêm na orientação do enlace que chegou; se o par está
    // gravado ao contrário, as pontas trocam de lugar.
    let same_direction = row.source_device_id == link.source_device_id;
    let (incoming_source, incoming_target) = if same_direction {
        (link.source_interface_id, link.target_interface_id)
    } else {
        (link.target_interface_id, link.source_interface_id)
    };
    UpsertPlan::Keep {
        source_interface_id: row.source_interface_id.or(incoming_source),
        target_interface_id: row.target_interface_id.or(incoming_target),
        touch_last_seen: OBSERVED_METHODS.contains(&link.discovery_method.as_str()),
    }
}

/// Grava os enlaces resolvidos, um por par de dispositivos, sem rebaixar os
/// que valem mais (ver [`plan_upsert`]). Tudo numa transação de escrita.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn persist_resolved_links_detailed(
    db: &sea_orm::DatabaseConnection,
    links: Vec<NetworkLink>,
) -> AppResult<PersistedLinks> {
    if links.is_empty() {
        return Ok(PersistedLinks {
            links: Vec::new(),
            created: 0,
            updated: 0,
        });
    }
    let txn = begin_write(db).await?;
    let persisted = persist_links_in(&txn, links).await?;
    txn.commit().await?;
    Ok(persisted)
}

/// Mesma gravação de [`persist_resolved_links_detailed`], dentro de uma
/// conexão/transação que o chamador já abriu.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn persist_links_in<C: ConnectionTrait>(
    txn: &C,
    links: Vec<NetworkLink>,
) -> AppResult<PersistedLinks> {
    let links = resolve_links(links);
    if links.is_empty() {
        return Ok(PersistedLinks {
            links: Vec::new(),
            created: 0,
            updated: 0,
        });
    }
    let existing = device_links::Entity::find().all(txn).await?;
    let mut saved = Vec::new();
    let mut created = 0;
    let mut updated = 0;
    for link in links {
        let row = existing.iter().find(|row| {
            (row.source_device_id == link.source_device_id
                && row.target_device_id == link.target_device_id)
                || (row.source_device_id == link.target_device_id
                    && row.target_device_id == link.source_device_id)
        });
        let now = Utc::now();
        let saved_row = match (plan_upsert(row, &link), row) {
            (UpsertPlan::Overwrite { changed }, Some(row)) => {
                if changed {
                    updated += 1;
                }
                device_links::ActiveModel {
                    id: Set(row.id),
                    source_interface_id: Set(link.source_interface_id),
                    target_interface_id: Set(link.target_interface_id),
                    link_type: Set(link.link_type),
                    discovery_method: Set(link.discovery_method),
                    confidence: Set(link.confidence),
                    confirmed: Set(link.confirmed),
                    last_seen_at: Set(Some(now.into())),
                    ..Default::default()
                }
                .update(txn)
                .await?
            }
            (
                UpsertPlan::Keep {
                    source_interface_id,
                    target_interface_id,
                    touch_last_seen,
                },
                Some(row),
            ) => {
                let filled = source_interface_id != row.source_interface_id
                    || target_interface_id != row.target_interface_id;
                if !filled && !touch_last_seen {
                    saved.push(row.clone());
                    continue;
                }
                if filled {
                    updated += 1;
                }
                let mut active: device_links::ActiveModel = row.clone().into();
                active.source_interface_id = Set(source_interface_id);
                active.target_interface_id = Set(target_interface_id);
                if touch_last_seen {
                    active.last_seen_at = Set(Some(now.into()));
                }
                active.update(txn).await?
            }
            _ => {
                created += 1;
                device_links::ActiveModel {
                    source_device_id: Set(link.source_device_id),
                    target_device_id: Set(link.target_device_id),
                    source_interface_id: Set(link.source_interface_id),
                    target_interface_id: Set(link.target_interface_id),
                    link_type: Set(link.link_type),
                    discovery_method: Set(link.discovery_method),
                    confidence: Set(link.confidence),
                    confirmed: Set(link.confirmed),
                    last_seen_at: Set(Some(now.into())),
                    ..Default::default()
                }
                .insert(txn)
                .await?
            }
        };
        saved.push(saved_row);
    }
    Ok(PersistedLinks {
        links: saved,
        created,
        updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deduplica_par_bidirecional_pela_maior_confianca() {
        let links = resolve_links(vec![
            NetworkLink {
                source_device_id: 1,
                target_device_id: 2,
                source_interface_id: None,
                target_interface_id: None,
                link_type: "snmp".into(),
                discovery_method: "x".into(),
                confidence: 80,
                confirmed: false,
            },
            NetworkLink {
                source_device_id: 2,
                target_device_id: 1,
                source_interface_id: None,
                target_interface_id: None,
                link_type: "manual".into(),
                discovery_method: "x".into(),
                confidence: 100,
                confirmed: true,
            },
        ]);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, "manual");
    }

    fn link() -> NetworkLink {
        NetworkLink {
            source_device_id: 1,
            target_device_id: 2,
            source_interface_id: Some(10),
            target_interface_id: Some(20),
            link_type: "snmp".into(),
            discovery_method: "lldp".into(),
            confidence: 90,
            confirmed: true,
        }
    }

    fn linha_de(link: &NetworkLink, visto_ha_minutos: i64) -> device_links::Model {
        device_links::Model {
            id: 1,
            source_device_id: link.source_device_id,
            target_device_id: link.target_device_id,
            source_interface_id: link.source_interface_id,
            target_interface_id: link.target_interface_id,
            link_type: link.link_type.clone(),
            discovery_method: link.discovery_method.clone(),
            confidence: link.confidence,
            confirmed: link.confirmed,
            last_seen_at: Some((Utc::now() - chrono::Duration::minutes(visto_ha_minutos)).into()),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    #[test]
    fn last_seen_at_nao_conta_como_alteracao() {
        // Matriz de paridade #49: a prova de vida avança a cada varredura. Se
        // ela contasse, a topologia reportaria "atualizado" todo ciclo com o
        // desenho idêntico.
        let link = link();
        assert!(!is_material_change(&linha_de(&link, 0), &link));
        assert!(!is_material_change(&linha_de(&link, 60 * 24), &link));
    }

    #[test]
    fn troca_de_porta_ou_de_confianca_conta_como_alteracao() {
        let link = link();
        let linha = linha_de(&link, 0);

        for mutacao in [
            NetworkLink {
                source_interface_id: Some(11),
                ..link.clone()
            },
            NetworkLink {
                target_interface_id: None,
                ..link.clone()
            },
            NetworkLink {
                link_type: "manual".into(),
                ..link.clone()
            },
            NetworkLink {
                discovery_method: "cdp".into(),
                ..link.clone()
            },
            NetworkLink {
                confidence: 50,
                ..link.clone()
            },
            NetworkLink {
                confirmed: false,
                ..link.clone()
            },
        ] {
            assert!(
                is_material_change(&linha, &mutacao),
                "mudança material não detectada: {mutacao:?}"
            );
        }
    }

    fn manual() -> device_links::Model {
        device_links::Model {
            id: 7,
            source_device_id: 1,
            target_device_id: 2,
            source_interface_id: None,
            target_interface_id: None,
            link_type: "manual".into(),
            discovery_method: "user_defined".into(),
            confidence: 100,
            confirmed: true,
            last_seen_at: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn lldp(source: i64, target: i64) -> NetworkLink {
        NetworkLink {
            source_device_id: source,
            target_device_id: target,
            source_interface_id: Some(10),
            target_interface_id: Some(20),
            link_type: "lldp".into(),
            discovery_method: "lldp".into(),
            confidence: 95,
            confirmed: false,
        }
    }

    #[test]
    fn par_inedito_e_inserido() {
        assert_eq!(plan_upsert(None, &lldp(1, 2)), UpsertPlan::Insert);
    }

    #[test]
    fn lldp_nao_rebaixa_enlace_manual_so_completa_portas() {
        let plano = plan_upsert(Some(&manual()), &lldp(1, 2));
        assert_eq!(
            plano,
            UpsertPlan::Keep {
                source_interface_id: Some(10),
                target_interface_id: Some(20),
                touch_last_seen: true,
            }
        );
    }

    #[test]
    fn portas_descobertas_no_sentido_inverso_trocam_de_ponta() {
        // Gravado 1 → 2; o LLDP de 2 viu o enlace como 2 → 1.
        let plano = plan_upsert(Some(&manual()), &lldp(2, 1));
        assert_eq!(
            plano,
            UpsertPlan::Keep {
                source_interface_id: Some(20),
                target_interface_id: Some(10),
                touch_last_seen: true,
            }
        );
    }

    #[test]
    fn portas_escolhidas_pelo_operador_sao_preservadas() {
        let linha = device_links::Model {
            source_interface_id: Some(1),
            target_interface_id: Some(2),
            ..manual()
        };
        let plano = plan_upsert(Some(&linha), &lldp(1, 2));
        assert_eq!(
            plano,
            UpsertPlan::Keep {
                source_interface_id: Some(1),
                target_interface_id: Some(2),
                touch_last_seen: true,
            }
        );
    }

    #[test]
    fn inferencia_nao_renova_a_prova_de_vida_do_lldp() {
        // Senão o recálculo por sub-rede esconderia um enlace LLDP obsoleto.
        let linha = device_links::Model {
            link_type: "lldp".into(),
            discovery_method: "lldp".into(),
            confidence: 95,
            confirmed: false,
            ..manual()
        };
        let inferido = NetworkLink {
            link_type: "inferred".into(),
            discovery_method: "subnet_inference".into(),
            confidence: 60,
            source_interface_id: None,
            target_interface_id: None,
            ..lldp(1, 2)
        };
        assert_eq!(
            plan_upsert(Some(&linha), &inferido),
            UpsertPlan::Keep {
                source_interface_id: None,
                target_interface_id: None,
                touch_last_seen: false,
            }
        );
    }

    #[test]
    fn enlace_de_mesma_confianca_ou_maior_substitui() {
        let linha = device_links::Model {
            link_type: "lldp".into(),
            discovery_method: "lldp".into(),
            confidence: 95,
            confirmed: false,
            ..manual()
        };
        assert_eq!(
            plan_upsert(Some(&linha), &lldp(1, 2)),
            UpsertPlan::Overwrite { changed: true }
        );
        let manual_novo = NetworkLink {
            link_type: "manual".into(),
            discovery_method: "user_defined".into(),
            confidence: 100,
            confirmed: true,
            ..lldp(1, 2)
        };
        assert!(matches!(
            plan_upsert(Some(&linha), &manual_novo),
            UpsertPlan::Overwrite { changed: true }
        ));
    }
}
