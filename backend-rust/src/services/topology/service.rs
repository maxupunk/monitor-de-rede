//! Topologia baseada em `petgraph`, serializada no contrato simples do Vue.

use std::collections::{BTreeMap, BTreeSet};

use petgraph::{algo::is_cyclic_directed, graph::Graph};
use sea_orm::EntityTrait;

use crate::{
    models::{device_links, devices},
    services::{
        shared::errors::{AppError, AppResult},
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
    for device in &devices {
        if let Some(parent) = device.parent_id.filter(|parent| ids.contains(parent)) {
            if !edges.iter().any(|edge| {
                (edge.source == parent && edge.target == device.id)
                    || (edge.target == parent && edge.source == device.id)
            }) {
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
    }
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
