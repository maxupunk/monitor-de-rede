//! O que a topologia contribui com a IA (ADR 017): `get_topology` — o
//! equipamento acima de um aparelho (uplink) e os vizinhos, com as portas dos
//! dois lados.

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use netmonitor_core::{
    ai::{
        args::ToolArgs,
        lookup::{device_not_found, find_device},
        tools::{AiToolHandler, ToolGroup, ToolOutput},
    },
    areas::Area,
};
use serde_json::{json, Value};

use super::service::{get_topology, TopologyEdge};
use crate::services::{ai_tools::platform::MAX_LIMIT, shared::errors::AppResult};

pub struct Topology;

/// Uma ligação vista de `device_id`: o vizinho e as portas dos dois lados.
fn neighbor_row(edge: &TopologyEdge, device_id: i64) -> Value {
    let outgoing = edge.source_device_id == device_id;
    let (local, remote, remote_name) = if outgoing {
        (
            &edge.source_interface_name,
            &edge.target_interface_name,
            &edge.target_device_name,
        )
    } else {
        (
            &edge.target_interface_name,
            &edge.source_interface_name,
            &edge.source_device_name,
        )
    };
    json!({
        "neighbor": remote_name,
        "local_port": local,
        "remote_port": remote,
        "status": edge.status,
        "traffic": edge.traffic_label,
        "type": edge.link_type,
        "confirmed": edge.confirmed,
    })
}

#[async_trait]
impl AiToolHandler for Topology {
    fn area(&self) -> Area {
        Area::Network
    }

    fn name(&self) -> &'static str {
        "get_topology"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Platform
    }

    fn description(&self) -> &'static str {
        "Topologia física/lógica. Com 'device': o equipamento acima dele (uplink) e os vizinhos, com portas dos dois lados, estado e tráfego do enlace. \
Sem 'device': totais e os enlaces que não estão 'up'."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": { "type": "string", "description": "Nome, IP ou id do dispositivo (opcional)" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let graph = get_topology(&ctx.db, None).await?;
        let Some(identifier) = args.text("device") else {
            let degraded: Vec<Value> = graph
                .edges
                .iter()
                .filter(|edge| !matches!(edge.status.as_str(), "up" | "online"))
                .take(MAX_LIMIT as usize)
                .map(|edge| {
                    json!({
                        "from": edge.source_device_name,
                        "to": edge.target_device_name,
                        "status": edge.status,
                        "ports": [edge.source_interface_name, edge.target_interface_name],
                    })
                })
                .collect();
            return Ok(ToolOutput::data(json!({
                "devices": graph.nodes.len(),
                "links": graph.edges.len(),
                "links_not_up": degraded,
            })));
        };
        let Some(device) = find_device(&ctx.db, &identifier).await? else {
            return Ok(device_not_found(&identifier));
        };
        let node = graph.nodes.iter().find(|node| node.id == device.id);
        let parent = node
            .and_then(|node| node.parent_id)
            .and_then(|parent| graph.nodes.iter().find(|candidate| candidate.id == parent))
            .map(|parent| json!({ "name": parent.name, "status": parent.status }));
        let neighbors: Vec<Value> = graph
            .edges
            .iter()
            .filter(|edge| edge.source_device_id == device.id || edge.target_device_id == device.id)
            .map(|edge| neighbor_row(edge, device.id))
            .collect();
        Ok(ToolOutput::data(json!({
            "device": device.name,
            "uplink": parent,
            "neighbors": neighbors,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge() -> TopologyEdge {
        TopologyEdge {
            id: 1,
            source: 1,
            target: 2,
            source_device_id: 1,
            target_device_id: 2,
            source_device_name: Some("core".into()),
            target_device_name: Some("acesso-1".into()),
            source_interface_name: Some("ge-0/0/1".into()),
            target_interface_name: Some("eth0".into()),
            traffic_label: Some("12.0 Mbps".into()),
            link_type: "ethernet".into(),
            discovery_method: "lldp".into(),
            confidence: 100,
            confirmed: true,
            status: "up".into(),
            ..TopologyEdge::default()
        }
    }

    #[test]
    fn vizinho_e_visto_do_lado_de_quem_pergunta() {
        let ligacao = edge();
        let do_core = neighbor_row(&ligacao, 1);
        assert_eq!(do_core["neighbor"], "acesso-1");
        assert_eq!(do_core["local_port"], "ge-0/0/1");
        assert_eq!(do_core["remote_port"], "eth0");

        let do_acesso = neighbor_row(&ligacao, 2);
        assert_eq!(do_acesso["neighbor"], "core");
        assert_eq!(do_acesso["local_port"], "eth0");
    }
}
