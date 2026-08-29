//! Topologia baseada em `petgraph`, serializada no contrato simples do Vue.

use std::collections::{BTreeMap, BTreeSet};

use petgraph::{algo::is_cyclic_directed, graph::Graph};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::{
    dtos::resources::UnmanagedSwitchInput,
    models::{
        _entities::device_interfaces as device_interfaces_entity,
        _entities::metrics as metrics_entity, device_interfaces, device_links, devices, metrics,
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
    pub ip_address: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub snmp_enabled: bool,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyEdge {
    pub id: i64,
    pub source: i64,
    pub target: i64,
    pub source_device_id: i64,
    pub target_device_id: i64,
    pub source_device_name: Option<String>,
    pub target_device_name: Option<String>,
    pub source_interface_id: Option<i64>,
    pub target_interface_id: Option<i64>,
    pub source_interface_name: Option<String>,
    pub target_interface_name: Option<String>,
    pub source_interface_speed: Option<i64>,
    pub target_interface_speed: Option<i64>,
    pub source_interface_status: Option<String>,
    pub target_interface_status: Option<String>,
    pub in_bps: Option<f64>,
    pub out_bps: Option<f64>,
    pub traffic_bps: Option<f64>,
    pub traffic_label: Option<String>,
    pub link_type: String,
    pub discovery_method: String,
    pub confidence: i32,
    pub confirmed: bool,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyGraph {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

fn format_traffic_bps(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.2} Gbps", bps / 1_000_000_000.0)
    } else if bps >= 100_000_000.0 {
        format!("{:.1} Mbps", bps / 1_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.2} Mbps", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.1} Kbps", bps / 1_000.0)
    } else if bps > 0.0 {
        format!("{:.0} bps", bps)
    } else {
        "0 bps".to_string()
    }
}

pub async fn get_topology_with_live(
    ctx: &loco_rs::app::AppContext,
    site_id: Option<i64>,
    live: bool,
) -> AppResult<TopologyGraph> {
    if live {
        let all_devices = devices::Entity::find().all(&ctx.db).await?;
        let snmp_devices: Vec<_> = all_devices
            .into_iter()
            .filter(|d| {
                d.snmp_enabled
                    && d.ip_address
                        .as_ref()
                        .is_some_and(|ip| !ip.trim().is_empty())
                    && site_id.is_none_or(|site| d.site_id == Some(site))
            })
            .collect();

        if !snmp_devices.is_empty() {
            let futures: Vec<_> = snmp_devices
                .iter()
                .filter_map(|device| {
                    crate::services::snmp::service::device_config(device)
                        .ok()
                        .map(|config| {
                            let ctx_clone = ctx;
                            let device_ref = device;
                            async move {
                                let _ = tokio::time::timeout(
                                    std::time::Duration::from_millis(1500),
                                    crate::services::snmp::service::poll_device(
                                        ctx_clone, device_ref, config,
                                    ),
                                )
                                .await;
                            }
                        })
                })
                .collect();

            futures::future::join_all(futures).await;
        }
    }

    get_topology(&ctx.db, site_id).await
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
    let devices_by_id: BTreeMap<i64, &devices::Model> = devices.iter().map(|d| (d.id, d)).collect();
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
        let parent_name = devices_by_id.get(&parent).map(|d| d.name.clone());
        edges.push(TopologyEdge {
            id: -(device.id * 1000 + parent),
            source: parent,
            target: device.id,
            source_device_id: parent,
            target_device_id: device.id,
            source_device_name: parent_name,
            target_device_name: Some(device.name.clone()),
            source_interface_id: None,
            target_interface_id: None,
            source_interface_name: None,
            target_interface_name: None,
            source_interface_speed: None,
            target_interface_speed: None,
            source_interface_status: None,
            target_interface_status: None,
            in_bps: None,
            out_bps: None,
            traffic_bps: None,
            traffic_label: None,
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
    let devices_by_id: BTreeMap<i64, &devices::Model> = devices.iter().map(|d| (d.id, d)).collect();

    let interfaces = crate::models::device_interfaces::Entity::find()
        .all(db)
        .await?;
    let interfaces_by_id: BTreeMap<i64, &device_interfaces::Model> =
        interfaces.iter().map(|i| (i.id, i)).collect();

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

    let iface_ids: Vec<i64> = links
        .iter()
        .flat_map(|l| [l.source_interface_id, l.target_interface_id])
        .flatten()
        .collect();

    let mut latest_traffic: BTreeMap<i64, (Option<f64>, Option<f64>)> = BTreeMap::new();
    if !iface_ids.is_empty() {
        let recent_metrics = metrics::Entity::find()
            .filter(metrics_entity::Column::InterfaceId.is_in(iface_ids))
            .filter(metrics_entity::Column::Name.is_in([
                "inBps",
                "outBps",
                "traffic_rx_bytes",
                "traffic_tx_bytes",
            ]))
            .order_by_desc(metrics_entity::Column::RecordedAt)
            .all(db)
            .await?;

        for m in recent_metrics {
            if let Some(iface_id) = m.interface_id {
                let entry = latest_traffic.entry(iface_id).or_insert((None, None));
                if (m.name == "inBps" || m.name == "traffic_rx_bytes") && entry.0.is_none() {
                    entry.0 = Some(m.value);
                } else if (m.name == "outBps" || m.name == "traffic_tx_bytes") && entry.1.is_none()
                {
                    entry.1 = Some(m.value);
                }
            }
        }
    }

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

    let mut edges: Vec<_> = links
        .into_iter()
        .map(|l| edge(l, &devices_by_id, &interfaces_by_id, &latest_traffic))
        .collect();
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
                ip_address: device.ip_address,
                vendor: device.vendor,
                model: device.model,
                snmp_enabled: device.snmp_enabled,
                parent_id: device.parent_id,
            })
            .collect(),
        edges,
    })
}

fn edge(
    link: device_links::Model,
    devices_by_id: &BTreeMap<i64, &devices::Model>,
    interfaces_by_id: &BTreeMap<i64, &device_interfaces::Model>,
    latest_traffic: &BTreeMap<i64, (Option<f64>, Option<f64>)>,
) -> TopologyEdge {
    let source_iface = link
        .source_interface_id
        .and_then(|id| interfaces_by_id.get(&id).copied());
    let target_iface = link
        .target_interface_id
        .and_then(|id| interfaces_by_id.get(&id).copied());

    let (src_in, src_out) = link
        .source_interface_id
        .and_then(|id| latest_traffic.get(&id))
        .copied()
        .unwrap_or((None, None));
    let (tgt_in, tgt_out) = link
        .target_interface_id
        .and_then(|id| latest_traffic.get(&id))
        .copied()
        .unwrap_or((None, None));

    let in_bps = src_in.or(tgt_in);
    let out_bps = src_out.or(tgt_out);
    let total_bps = match (in_bps, out_bps) {
        (Some(i), Some(o)) => Some(i + o),
        (Some(i), None) => Some(i),
        (None, Some(o)) => Some(o),
        (None, None) => None,
    };
    let traffic_label = total_bps.map(format_traffic_bps);

    TopologyEdge {
        id: link.id,
        source: link.source_device_id,
        target: link.target_device_id,
        source_device_id: link.source_device_id,
        target_device_id: link.target_device_id,
        source_device_name: devices_by_id
            .get(&link.source_device_id)
            .map(|d| d.name.clone()),
        target_device_name: devices_by_id
            .get(&link.target_device_id)
            .map(|d| d.name.clone()),
        source_interface_id: link.source_interface_id,
        target_interface_id: link.target_interface_id,
        source_interface_name: source_iface.map(|i| i.name.clone()),
        target_interface_name: target_iface.map(|i| i.name.clone()),
        source_interface_speed: source_iface.and_then(|i| i.speed),
        target_interface_speed: target_iface.and_then(|i| i.speed),
        source_interface_status: source_iface
            .and_then(|i| i.oper_status.clone().or_else(|| i.admin_status.clone())),
        target_interface_status: target_iface
            .and_then(|i| i.oper_status.clone().or_else(|| i.admin_status.clone())),
        in_bps,
        out_bps,
        traffic_bps: total_bps,
        traffic_label,
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
    link_type: Option<String>,
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
            link_type: link_type.unwrap_or_else(|| "manual".into()),
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
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Falha ao criar ligação")))
}

pub async fn update_manual_link(
    db: &sea_orm::DatabaseConnection,
    link_id: i64,
    source_interface: Option<i64>,
    target_interface: Option<i64>,
    link_type: Option<String>,
) -> AppResult<device_links::Model> {
    if link_id <= 0 {
        return Err(AppError::validation(
            "Enlaces virtuais de hierarquia não podem ser editados diretamente",
        ));
    }
    let link = device_links::Entity::find_by_id(link_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Ligação não encontrada"))?;

    let mut active: device_links::ActiveModel = link.into();
    active.source_interface_id = sea_orm::Set(source_interface);
    active.target_interface_id = sea_orm::Set(target_interface);
    if let Some(t) = link_type {
        active.link_type = sea_orm::Set(t);
    }
    active.updated_at = sea_orm::Set(chrono::Utc::now().into());
    let updated = active.update(db).await?;
    Ok(updated)
}

pub async fn create_unmanaged_switch(
    db: &sea_orm::DatabaseConnection,
    input: UnmanagedSwitchInput,
) -> AppResult<devices::Model> {
    if input.name.trim().is_empty() {
        return Err(AppError::validation("Nome do switch é obrigatório"));
    }
    let port_count = input.port_count.clamp(1, 128);
    let now = chrono::Utc::now();
    let device_model = devices::ActiveModel {
        name: sea_orm::Set(input.name.trim().to_string()),
        r#type: sea_orm::Set("unmanaged_switch".to_string()),
        vendor: sea_orm::Set(input.vendor.filter(|v| !v.trim().is_empty())),
        model: sea_orm::Set(input.model.filter(|m| !m.trim().is_empty())),
        site_id: sea_orm::Set(input.site_id),
        network_id: sea_orm::Set(input.network_id),
        is_monitored: sea_orm::Set(false),
        snmp_enabled: sea_orm::Set(false),
        snmp_poll_interval_seconds: sea_orm::Set(60),
        status: sea_orm::Set("online".to_string()),
        created_at: sea_orm::Set(now.into()),
        updated_at: sea_orm::Set(now.into()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    for i in 1..=port_count {
        let port_name = format!("Porta {i}");
        device_interfaces::ActiveModel {
            device_id: sea_orm::Set(device_model.id),
            snmp_index: sea_orm::Set(Some(i as i32)),
            name: sea_orm::Set(port_name),
            description: sea_orm::Set(Some(format!("Porta física #{i}"))),
            r#type: sea_orm::Set(Some("ethernetCsmacd".to_string())),
            speed: sea_orm::Set(Some(1_000_000_000)), // 1 Gbps
            admin_status: sea_orm::Set(Some("up".to_string())),
            oper_status: sea_orm::Set(Some("up".to_string())),
            created_at: sea_orm::Set(now.into()),
            updated_at: sea_orm::Set(now.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(device_model)
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
        for source in devices.iter().filter(|device| {
            matches!(
                device.r#type.as_str(),
                "router" | "switch" | "firewall" | "unmanaged_switch"
            )
        }) {
            for target in devices.iter().filter(|device| {
                !matches!(
                    device.r#type.as_str(),
                    "router" | "switch" | "firewall" | "unmanaged_switch"
                )
            }) {
                raw.push(NetworkLink {
                    source_device_id: source.id,
                    target_device_id: target.id,
                    source_interface_id: None,
                    target_interface_id: target.link_interface_id,
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
            snmp_poll_interval_seconds: 60,
            access_mode: None,
            operating_system: None,
            syslog_server_address: None,
            system_key: None,
            link_interface_id: None,
            link_interface_name: None,
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
            source_device_id: source,
            target_device_id: target,
            source_device_name: None,
            target_device_name: None,
            source_interface_id: None,
            target_interface_id: None,
            source_interface_name: None,
            target_interface_name: None,
            source_interface_speed: None,
            target_interface_speed: None,
            source_interface_status: None,
            target_interface_status: None,
            in_bps: None,
            out_bps: None,
            traffic_bps: None,
            traffic_label: None,
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
