//! O próprio NetMonitor como objeto de diagnóstico: agentes remotos, VPN,
//! topologia, descoberta, redes, janelas de manutenção, auditoria e
//! notificações.
//!
//! Uma ferramenta com `area` em vez de uma por domínio: cada schema vai em
//! toda rodada depois de carregado, e oito contratos quase iguais custariam
//! oito vezes mais para a IA escolher entre eles. Tudo é leitura, e nada que
//! seja segredo sai daqui (token, chave privada, senha de canal).

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use super::{
    lookup::{device_names, find_device},
    AiToolHandler, ToolArgs, ToolGroup, ToolOutput,
};
use crate::{
    models::_entities::{
        discovery_results, discovery_runs, dns_servers, networks, notification_outbox, sites, users,
    },
    services::{
        agents::{
            hub::AgentHub,
            service::{to_view, AgentService},
        },
        audit::{AuditFilters, AuditService},
        discovery::service::ScanSessionService,
        maintenance_windows,
        shared::errors::AppResult,
        topology::service::{get_topology, TopologyEdge},
        vpn::{peer_service, server_service},
    },
};

const AREAS: [&str; 7] = [
    "agents",
    "vpn",
    "discovery",
    "networks",
    "maintenance",
    "audit",
    "notifications",
];
const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 50;
const MIB: f64 = 1024.0 * 1024.0;

#[allow(clippy::cast_precision_loss)]
fn mib(bytes: i64) -> f64 {
    (bytes as f64 / MIB * 100.0).round() / 100.0
}

fn rfc3339<Tz: chrono::TimeZone>(at: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    at.with_timezone(&Utc).to_rfc3339()
}

pub struct PlatformStatus;

#[async_trait]
impl AiToolHandler for PlatformStatus {
    fn name(&self) -> &'static str {
        "get_platform_status"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Platform
    }

    fn description(&self) -> &'static str {
        "Estado do próprio NetMonitor por área: 'agents' (agentes remotos: conexão, versão, host), 'vpn' (servidor WireGuard e peers: handshake, tráfego), \
'discovery' (varreduras recentes e em andamento), 'networks' (redes/CIDR, sites e servidores DNS), 'maintenance' (janelas ativas e futuras), \
'audit' (quem mudou o quê, com hours) e 'notifications' (envios recentes, pendentes e suprimidos, com hours)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "area": { "type": "string", "enum": AREAS },
                "hours": { "type": "integer", "description": "Janela para audit, notifications e discovery (1 a 720, padrão 24)" },
                "limit": { "type": "integer", "description": "Itens na lista (1 a 50, padrão 20)" }
            },
            "required": ["area"]
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let hours = args.integer_in("hours", 24, 1, 720);
        let since = Utc::now() - Duration::hours(hours);
        let limit =
            usize::try_from(args.integer_in("limit", DEFAULT_LIMIT, 1, MAX_LIMIT)).unwrap_or(20);
        let area = args.text("area").unwrap_or_default().to_lowercase();
        let data = match area.as_str() {
            "agents" => agents(ctx, limit).await?,
            "vpn" => vpn(ctx, limit).await?,
            "discovery" => discovery(ctx, since, limit).await?,
            "networks" => networks_overview(ctx, limit).await?,
            "maintenance" => maintenance(ctx, limit).await?,
            "audit" => audit(ctx, since, limit).await?,
            "notifications" => notifications(ctx, since, limit).await?,
            _ => {
                return Ok(ToolOutput::not_found(format!(
                    "Área '{area}' desconhecida; use {}",
                    AREAS.join(", ")
                )))
            }
        };
        Ok(ToolOutput::data(data))
    }
}

async fn agents(ctx: &AppContext, limit: usize) -> AppResult<Value> {
    let service = AgentService::new(&ctx.db);
    let listed = service.list().await?;
    let devices: HashMap<i64, _> = service
        .devices_of(&listed)
        .await?
        .into_iter()
        .map(|device| (device.id, device))
        .collect();
    // Sem o hub (processo sem canal de agentes), ninguém está conectado.
    let hub = AgentHub::from_context(ctx).ok();
    let views: Vec<_> = listed
        .iter()
        .map(|probe| {
            let device = probe.device_id.and_then(|id| devices.get(&id));
            let connected = hub.as_ref().is_some_and(|hub| hub.get(probe.id).is_some());
            to_view(probe, device, connected)
        })
        .collect();
    let connected = views.iter().filter(|view| view.connected).count();
    Ok(json!({
        "total": views.len(),
        "connected": connected,
        "agents": views.iter().take(limit).map(|view| json!({
            "name": view.name,
            "connected": view.connected,
            "status": view.status,
            "version": view.version,
            "last_seen_at": view.last_seen_at,
            "device": view.device_name,
            "hostname": view.host.hostname,
            "os": view.host.os,
            "docker": view.host.docker.available,
            "policy": view.host.policy,
        })).collect::<Vec<_>>(),
        "omitted": views.len().saturating_sub(limit),
    }))
}

async fn vpn(ctx: &AppContext, limit: usize) -> AppResult<Value> {
    let state = server_service::get_state(&ctx.db).await?;
    let Some(server) = state.server.as_ref() else {
        return Ok(json!({ "configured": false }));
    };
    let mut peers = peer_service::list(&ctx.db).await?;
    // Quem precisa de atenção primeiro.
    peers.sort_by_key(|item| {
        matches!(
            item.peer.connection_status(),
            crate::models::vpn_peers::VpnPeerConnectionStatus::Connected
        )
    });
    Ok(json!({
        "configured": true,
        "interface": server.interface_name,
        "listen_port": server.listen_port,
        "endpoint": server.public_endpoint,
        "active": server.active,
        "cidr": state.cidr,
        "peers_total": state.peers_total,
        "peers_connected": state.peers_connected,
        "rx_mib": mib(state.bytes_rx),
        "tx_mib": mib(state.bytes_tx),
        "peers": peers.iter().take(limit).map(|item| json!({
            "device": item.device.as_ref().map(|device| device.name.clone()),
            "status": item.peer.connection_status(),
            "enabled": item.peer.enabled,
            "last_handshake_at": item.peer.last_handshake_at.as_ref().map(rfc3339),
            "rx_mib": mib(item.peer.bytes_rx),
            "tx_mib": mib(item.peer.bytes_tx),
            "needs_firewall_rule": item.hints.needs_firewall_hint,
        })).collect::<Vec<_>>(),
        "omitted": peers.len().saturating_sub(limit),
    }))
}

async fn discovery(ctx: &AppContext, since: DateTime<Utc>, limit: usize) -> AppResult<Value> {
    let runs = discovery_runs::Entity::find()
        .filter(discovery_runs::Column::StartedAt.gte(since))
        .order_by_desc(discovery_runs::Column::StartedAt)
        .limit(limit as u64)
        .all(&ctx.db)
        .await?;
    let run_ids: Vec<i64> = runs.iter().map(|run| run.id).collect();
    let mut found: HashMap<i64, usize> = HashMap::new();
    for run_id in discovery_results::Entity::find()
        .select_only()
        .column(discovery_results::Column::DiscoveryRunId)
        .filter(discovery_results::Column::DiscoveryRunId.is_in(run_ids))
        .into_tuple::<i64>()
        .all(&ctx.db)
        .await?
    {
        *found.entry(run_id).or_default() += 1;
    }
    let network_names = network_names(ctx).await?;
    let live = match ScanSessionService::from_context(ctx) {
        Ok(service) => {
            let state = service.state().await;
            state.run_id.map(|run_id| {
                json!({
                    "run_id": run_id,
                    "status": state.status,
                    "phase": state.phase,
                    "progress": format!("{}/{}", state.progress_current, state.progress_total),
                    "hosts_found": state.hosts.len(),
                    "error": state.error,
                })
            })
        }
        Err(_) => None,
    };
    Ok(json!({
        "current": live,
        "runs": runs.iter().map(|run| json!({
            "id": run.id,
            "network": network_names.get(&run.network_id),
            "status": run.status,
            "started_at": rfc3339(&run.started_at),
            "finished_at": run.finished_at.as_ref().map(rfc3339),
            "hosts_found": found.get(&run.id).copied().unwrap_or(0),
            "error": run.error,
        })).collect::<Vec<_>>(),
    }))
}

async fn network_names(ctx: &AppContext) -> AppResult<HashMap<i64, String>> {
    Ok(networks::Entity::find()
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|network| (network.id, format!("{} ({})", network.name, network.cidr)))
        .collect())
}

async fn networks_overview(ctx: &AppContext, limit: usize) -> AppResult<Value> {
    let (networks, sites, dns) = tokio::try_join!(
        networks::Entity::find().all(&ctx.db),
        sites::Entity::find().all(&ctx.db),
        dns_servers::Entity::find().all(&ctx.db),
    )?;
    let site_names: HashMap<i64, &str> = sites
        .iter()
        .map(|site| (site.id, site.name.as_str()))
        .collect();
    Ok(json!({
        "networks": networks.iter().take(limit).map(|network| json!({
            "name": network.name,
            "cidr": network.cidr,
            "gateway": network.gateway,
            "vlan": network.vlan,
            "site": network.site_id.and_then(|id| site_names.get(&id)),
            "scan_enabled": network.scan_enabled,
            "last_scan_at": network.last_scan_at.as_ref().map(rfc3339),
            "active": network.active,
        })).collect::<Vec<_>>(),
        "networks_omitted": networks.len().saturating_sub(limit),
        "sites": sites.iter().take(limit).map(|site| json!({
            "name": site.name,
            "location": site.location,
            "active": site.active,
        })).collect::<Vec<_>>(),
        "dns_servers": dns.iter().map(|server| json!({
            "name": server.name,
            "address": server.address,
            "protocol": server.protocol,
            "default": server.is_default,
        })).collect::<Vec<_>>(),
    }))
}

async fn maintenance(ctx: &AppContext, limit: usize) -> AppResult<Value> {
    let now = Utc::now();
    let windows: Vec<_> = maintenance_windows::list(&ctx.db)
        .await?
        .into_iter()
        .filter(|window| window.ends_at.with_timezone(&Utc) > now)
        .collect();
    let names = device_names(&ctx.db, windows.iter().filter_map(|w| w.device_id)).await?;
    let row = |window: &&crate::models::_entities::maintenance_windows::Model| {
        json!({
            "name": window.name,
            "starts_at": rfc3339(&window.starts_at),
            "ends_at": rfc3339(&window.ends_at),
            "device": window.device_id.and_then(|id| names.get(&id)),
            "site_id": window.site_id,
        })
    };
    let (active, upcoming): (Vec<_>, Vec<_>) = windows
        .iter()
        .partition(|window| window.starts_at.with_timezone(&Utc) <= now);
    Ok(json!({
        "active": active.iter().take(limit).map(row).collect::<Vec<_>>(),
        "upcoming": upcoming.iter().take(limit).map(row).collect::<Vec<_>>(),
    }))
}

async fn audit(ctx: &AppContext, since: DateTime<Utc>, limit: usize) -> AppResult<Value> {
    let page = AuditService::new(&ctx.db)
        .list(
            AuditFilters {
                from: Some(since),
                ..AuditFilters::default()
            },
            Some(1),
            Some(limit as u64),
        )
        .await?;
    let user_ids: Vec<i64> = page.data.iter().filter_map(|entry| entry.user_id).collect();
    let user_names: HashMap<i64, String> = users::Entity::find()
        .filter(users::Column::Id.is_in(user_ids))
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|user| (user.id, user.name))
        .collect();
    Ok(json!({
        "total": page.meta.total,
        "entries": page.data.iter().map(|entry| json!({
            "at": rfc3339(&entry.created_at),
            "user": entry.user_id.and_then(|id| user_names.get(&id)),
            "action": entry.action,
            "resource": entry.resource_type,
            "target": entry.resource_label,
            "description": entry.description,
        })).collect::<Vec<_>>(),
    }))
}

async fn notifications(ctx: &AppContext, since: DateTime<Utc>, limit: usize) -> AppResult<Value> {
    let rows = notification_outbox::Entity::find()
        .filter(notification_outbox::Column::CreatedAt.gte(since))
        .order_by_desc(notification_outbox::Column::CreatedAt)
        .all(&ctx.db)
        .await?;
    let mut by_status: HashMap<&str, usize> = HashMap::new();
    for row in &rows {
        *by_status.entry(row.status.as_str()).or_default() += 1;
    }
    Ok(json!({
        "by_status": by_status,
        "recent": rows.iter().take(limit).map(|row| json!({
            "at": rfc3339(&row.created_at),
            "status": row.status,
            "kind": row.kind,
            "severity": row.severity,
            "title": row.title,
            "suppress_reason": row.suppress_reason,
            "sent_at": row.sent_at.as_ref().map(rfc3339),
        })).collect::<Vec<_>>(),
        "note": "Falhas de entrega não ficam gravadas; para elas, grep source='logs' com pattern do canal (telegram, email, webhook).",
    }))
}

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
                .filter(|edge| edge.status != "up")
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
            return Ok(ToolOutput::not_found(format!(
                "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
            )));
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
            source_interface_id: None,
            target_interface_id: None,
            source_interface_name: Some("ge-0/0/1".into()),
            target_interface_name: Some("eth0".into()),
            source_interface_speed: None,
            target_interface_speed: None,
            source_interface_status: None,
            target_interface_status: None,
            in_bps: None,
            out_bps: None,
            traffic_bps: None,
            traffic_label: Some("12.0 Mbps".into()),
            link_type: "ethernet".into(),
            discovery_method: "lldp".into(),
            confidence: 100,
            confirmed: true,
            status: "up".into(),
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

    #[test]
    fn trafego_da_vpn_em_mib() {
        assert!((mib(3 * 1024 * 1024) - 3.0).abs() < f64::EPSILON);
        assert!((mib(0)).abs() < f64::EPSILON);
    }
}
