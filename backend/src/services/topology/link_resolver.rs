//! Deduplicação e persistência dos enlaces físicos/lógicos.

use std::collections::BTreeMap;

use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{models::device_links, services::shared::errors::AppResult};

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

pub async fn persist_resolved_links_detailed(
    db: &sea_orm::DatabaseConnection,
    links: Vec<NetworkLink>,
) -> AppResult<PersistedLinks> {
    let existing = device_links::Entity::find().all(db).await?;
    let mut saved = Vec::new();
    let mut created = 0;
    let mut updated = 0;
    for link in resolve_links(links) {
        let row = existing.iter().find(|row| {
            (row.source_device_id == link.source_device_id
                && row.target_device_id == link.target_device_id)
                || (row.source_device_id == link.target_device_id
                    && row.target_device_id == link.source_device_id)
        });
        let now = Utc::now();
        if let Some(row) = row {
            let changed = is_material_change(row, &link);
            let saved_row = device_links::ActiveModel {
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
            .update(db)
            .await?;
            if changed {
                updated += 1;
            }
            saved.push(saved_row);
        } else {
            let saved_row = device_links::ActiveModel {
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
            .insert(db)
            .await?;
            created += 1;
            saved.push(saved_row);
        }
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
}
