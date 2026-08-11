//! Topologia baseada em `petgraph`, serializada no contrato simples do Vue.

use std::collections::{BTreeMap, BTreeSet};

use petgraph::{algo::is_cyclic_directed, graph::Graph};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{
    models::{
        _entities::device_interfaces as device_interfaces_entity, device_interfaces, device_links,
        devices,
    },
    services::{
        shared::errors::{AppError, AppResult},
        snmp::collectors::LldpNeighbor,
        topology::link_resolver::{persist_resolved_links_detailed, NetworkLink},
    },
};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyNode {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub status: String,
    pub site_id: Option<i64>,
    pub interface_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyEdge {
    pub id: i64,
    pub source: i64,
    pub target: i64,
    pub source_interface_id: Option<i64>,
    pub target_interface_id: Option<i64>,
    pub link_type: String,
    pub discovery_method: String,
    pub confidence: i32,
    pub confirmed: bool,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TopologyGraph {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

/// Acrescenta a aresta virtual do `parentId` quando não existe enlace real
/// entre pai e filho (matriz de paridade #48).
///
/// O `id` é **negativo** de propósito: a tela usa o id da aresta para abrir o
/// enlace, e um id positivo aqui apontaria para uma linha de `device_links` que
/// não existe — o clique daria 404. O sinal é o que distingue "hierarquia
/// declarada pelo operador" de "enlace descoberto".
///
/// A aresta só entra se pai e filho estiverem **ambos** no recorte pedido: um
/// pai de outro site viraria nó fantasma no grafo.
fn append_parent_edges(
    edges: &mut Vec<TopologyEdge>,
    devices: &[devices::Model],
    ids: &BTreeSet<i64>,
) {
    for device in devices {
        let Some(parent) = device.parent_id.filter(|parent| ids.contains(parent)) else {
            continue;
        };
        // Um enlace real entre os dois já descreve a ligação — duplicar
        // desenharia duas linhas entre o mesmo par de nós.
        let ja_ligados = edges.iter().any(|edge| {
            (edge.source == parent && edge.target == device.id)
                || (edge.target == parent && edge.source == device.id)
        });
        if ja_ligados {
            continue;
        }
        edges.push(TopologyEdge {
            id: -(device.id * 1000 + parent),
            source: parent,
            target: device.id,
            source_interface_id: None,
            target_interface_id: None,
            link_type: "parent".into(),
            discovery_method: "parent_hierarchy".into(),
            confidence: 100,
            confirmed: true,
            status: "up".into(),
        });
    }
}

pub async fn get_topology(
    db: &sea_orm::DatabaseConnection,
    site_id: Option<i64>,
) -> AppResult<TopologyGraph> {
    let all = devices::Entity::find().all(db).await?;
    let devices: Vec<_> = all
        .into_iter()
        .filter(|device| site_id.is_none_or(|site| device.site_id == Some(site)))
        .collect();
    let ids: BTreeSet<_> = devices.iter().map(|device| device.id).collect();
    let interfaces = crate::models::device_interfaces::Entity::find()
        .all(db)
        .await?;
    let counts = interfaces
        .iter()
        .filter(|interface| ids.contains(&interface.device_id))
        .fold(BTreeMap::<i64, usize>::new(), |mut counts, interface| {
            *counts.entry(interface.device_id).or_default() += 1;
            counts
        });
    let links = device_links::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .filter(|link| ids.contains(&link.source_device_id) && ids.contains(&link.target_device_id))
        .collect::<Vec<_>>();

    let mut graph = Graph::<i64, i64>::new();
    let nodes_map: BTreeMap<_, _> = devices
        .iter()
        .map(|device| (device.id, graph.add_node(device.id)))
        .collect();
    for link in &links {
        if let (Some(source), Some(target)) = (
            nodes_map.get(&link.source_device_id),
            nodes_map.get(&link.target_device_id),
        ) {
            graph.add_edge(*source, *target, link.id);
        }
    }
    // Detecta grafo inconsistente, preservando ciclos válidos de redundância.
    let _has_cycle = is_cyclic_directed(&graph);

    let mut edges: Vec<_> = links.into_iter().map(edge).collect();
    append_parent_edges(&mut edges, &devices, &ids);
    Ok(TopologyGraph {
        nodes: devices
            .into_iter()
            .map(|device| TopologyNode {
                id: device.id,
                name: device.name,
                device_type: device.r#type,
                status: device.status,
                site_id: device.site_id,
                interface_count: *counts.get(&device.id).unwrap_or(&0),
            })
            .collect(),
        edges,
    })
}

fn edge(link: device_links::Model) -> TopologyEdge {
    TopologyEdge {
        id: link.id,
        source: link.source_device_id,
        target: link.target_device_id,
        source_interface_id: link.source_interface_id,
        target_interface_id: link.target_interface_id,
        link_type: link.link_type,
        discovery_method: link.discovery_method,
        confidence: link.confidence,
        confirmed: link.confirmed,
        status: "up".into(),
    }
}

pub async fn create_manual_link(
    db: &sea_orm::DatabaseConnection,
    source: i64,
    target: i64,
    source_interface: Option<i64>,
    target_interface: Option<i64>,
) -> AppResult<device_links::Model> {
    if source == target {
        return Err(AppError::validation(
            "Uma ligação deve conectar dispositivos diferentes",
        ));
    }
    let result = persist_resolved_links_detailed(
        db,
        vec![NetworkLink {
            source_device_id: source,
            target_device_id: target,
            source_interface_id: source_interface,
            target_interface_id: target_interface,
            link_type: "manual".into(),
            discovery_method: "user_defined".into(),
            confidence: 100,
            confirmed: true,
        }],
    )
    .await?;
    result
        .links
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Ligação manual não persistida")))
}

pub async fn delete_link(db: &sea_orm::DatabaseConnection, id: i64) -> AppResult<bool> {
    Ok(device_links::Entity::delete_by_id(id)
        .exec(db)
        .await?
        .rows_affected
        > 0)
}

pub async fn infer_subnet_links(db: &sea_orm::DatabaseConnection) -> AppResult<usize> {
    let devices = devices::Entity::find().all(db).await?;
    let mut raw = Vec::new();
    let mut groups = BTreeMap::<i64, Vec<_>>::new();
    for device in devices {
        if let Some(network) = device.network_id {
            groups.entry(network).or_default().push(device);
        }
    }
    for devices in groups.into_values() {
        for source in devices
            .iter()
            .filter(|device| matches!(device.r#type.as_str(), "router" | "switch" | "firewall"))
        {
            for target in devices.iter().filter(|device| {
                !matches!(device.r#type.as_str(), "router" | "switch" | "firewall")
            }) {
                raw.push(NetworkLink {
                    source_device_id: source.id,
                    target_device_id: target.id,
                    source_interface_id: None,
                    target_interface_id: None,
                    link_type: "inferred".into(),
                    discovery_method: "subnet_inference".into(),
                    confidence: 60,
                    confirmed: false,
                });
            }
        }
    }
    Ok(persist_resolved_links_detailed(db, raw).await?.links.len())
}

/// Converte os vizinhos LLDP/CDP observados no poll SNMP em enlaces persistidos.
/// O nome e o endereço de gerência são usados apenas para relacionar dispositivos
/// já cadastrados; uma resposta externa nunca cria um dispositivo implicitamente.
pub async fn resolve_discovered_neighbors(
    ctx: &loco_rs::app::AppContext,
    source: &devices::Model,
    neighbors: &[LldpNeighbor],
) -> AppResult<Vec<device_links::Model>> {
    let candidates = devices::Entity::find().all(&ctx.db).await?;
    let source_interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(source.id))
        .all(&ctx.db)
        .await?;
    let mut raw = Vec::new();
    for neighbor in neighbors {
        let target = candidates.iter().find(|device| {
            device.id != source.id
                && (neighbor.remote_sys_name.as_ref().is_some_and(|name| {
                    device.name.eq_ignore_ascii_case(name)
                        || device
                            .ip_address
                            .as_ref()
                            .is_some_and(|ip| ip.eq_ignore_ascii_case(name))
                }) || neighbor
                    .remote_mgmt_address
                    .as_ref()
                    .is_some_and(|address| {
                        device.ip_address.as_ref().is_some_and(|ip| ip == address)
                    }))
        });
        let Some(target) = target else {
            continue;
        };
        let target_interfaces = device_interfaces::Entity::find()
            .filter(device_interfaces_entity::Column::DeviceId.eq(target.id))
            .all(&ctx.db)
            .await?;
        let source_interface_id = neighbor
            .local_port
            .parse::<i32>()
            .ok()
            .and_then(|index| {
                source_interfaces
                    .iter()
                    .find(|interface| interface.snmp_index == Some(index))
            })
            .map(|interface| interface.id);
        let target_interface_id = neighbor.remote_port.as_ref().and_then(|port| {
            target_interfaces
                .iter()
                .find(|interface| {
                    interface.name.eq_ignore_ascii_case(port)
                        || interface.alias.as_ref().is_some_and(|alias| alias == port)
                })
                .map(|interface| interface.id)
        });
        let (confidence, method) = if neighbor.protocol == "lldp" {
            (95, "lldp")
        } else {
            (90, "cdp")
        };
        raw.push(NetworkLink {
            source_device_id: source.id,
            target_device_id: target.id,
            source_interface_id,
            target_interface_id,
            link_type: neighbor.protocol.clone(),
            discovery_method: method.into(),
            confidence,
            confirmed: false,
        });
    }
    Ok(persist_resolved_links_detailed(&ctx.db, raw).await?.links)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispositivo(id: i64, parent_id: Option<i64>) -> devices::Model {
        devices::Model {
            id,
            site_id: None,
            network_id: None,
            parent_id,
            zabbix_template_id: None,
            ip_address: None,
            name: format!("dev-{id}"),
            r#type: "switch".into(),
            vendor: None,
            model: None,
            serial_number: None,
            description: None,
            is_monitored: true,
            snmp_enabled: false,
            snmp_community: None,
            snmp_version: None,
            status: "online".into(),
            last_seen_at: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn aresta_real(id: i64, source: i64, target: i64) -> TopologyEdge {
        TopologyEdge {
            id,
            source,
            target,
            source_interface_id: None,
            target_interface_id: None,
            link_type: "snmp".into(),
            discovery_method: "lldp".into(),
            confidence: 90,
            confirmed: true,
            status: "up".into(),
        }
    }

    #[test]
    fn parent_id_vira_aresta_virtual_com_id_negativo() {
        let devices = vec![dispositivo(1, None), dispositivo(7, Some(1))];
        let ids = BTreeSet::from([1, 7]);
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &ids);

        assert_eq!(edges.len(), 1);
        let aresta = &edges[0];
        // Matriz de paridade #48: id negativo distingue hierarquia de enlace real.
        assert_eq!(aresta.id, -(7 * 1000 + 1));
        assert!(aresta.id < 0);
        assert_eq!(aresta.source, 1);
        assert_eq!(aresta.target, 7);
        assert_eq!(aresta.link_type, "parent");
        assert_eq!(aresta.discovery_method, "parent_hierarchy");
    }

    #[test]
    fn enlace_real_existente_dispensa_a_aresta_virtual() {
        let devices = vec![dispositivo(1, None), dispositivo(7, Some(1))];
        let ids = BTreeSet::from([1, 7]);

        // Nos dois sentidos: o enlace descoberto pode ter nascido de qualquer lado.
        for real in [aresta_real(50, 1, 7), aresta_real(50, 7, 1)] {
            let mut edges = vec![real];
            append_parent_edges(&mut edges, &devices, &ids);
            assert_eq!(edges.len(), 1, "aresta virtual duplicou um enlace real");
            assert_eq!(edges[0].id, 50);
        }
    }

    #[test]
    fn pai_fora_do_recorte_nao_vira_no_fantasma() {
        // O filho está no site pedido, o pai não. Desenhar a aresta criaria um
        // nó que a lista de `nodes` não contém.
        let devices = vec![dispositivo(7, Some(99))];
        let ids = BTreeSet::from([7]);
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &ids);
        assert!(edges.is_empty());
    }

    #[test]
    fn dispositivo_sem_pai_nao_gera_aresta() {
        let devices = vec![dispositivo(1, None)];
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &BTreeSet::from([1]));
        assert!(edges.is_empty());
    }
}
