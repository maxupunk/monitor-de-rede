//! O próprio NetMonitor como objeto de diagnóstico: agentes remotos, VPN,
//! descoberta, redes, janelas de manutenção e notificações. A trilha de
//! auditoria tem grupo próprio ([`super::audit`]); a topologia é da área Rede
//! (`services::topology::ai_tools`). VPN e descoberta são seções que as áreas
//! contribuem ([`PlatformSection`]).
//!
//! Uma ferramenta com `area` em vez de uma por domínio: cada schema vai em
//! toda rodada depois de carregado, e sete contratos quase iguais custariam
//! sete vezes mais para a IA escolher entre eles. Tudo é leitura, e nada que
//! seja segredo sai daqui (token, chave privada, senha de canal).

use netmonitor_core::areas::Area;
use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

/// O teto das listas e os nomes das redes moram no núcleo (a topologia e a
/// seção da descoberta, da área Rede, também os usam).
pub use netmonitor_core::ai::platform::{network_names, MAX_LIMIT};
use netmonitor_core::ai::platform::{rfc3339, PlatformSection};

use super::{lookup::device_names, AiToolHandler, ToolArgs, ToolGroup, ToolOutput};
use crate::{
    models::_entities::{dns_servers, networks, notification_outbox, sites},
    services::{
        agents::{
            hub::AgentHub,
            service::{to_view, AgentService},
        },
        maintenance_windows,
        shared::errors::AppResult,
    },
};

const AREAS: [&str; 7] = [
    "agents",
    "vpn",
    "wifi",
    "discovery",
    "networks",
    "maintenance",
    "notifications",
];
const DEFAULT_LIMIT: i64 = 20;
/// O estado do próprio NetMonitor. As seções `vpn`, `wifi` e `discovery` são
/// contribuídas pelas áreas ([`PlatformSection`]), no ponto de montagem.
pub struct PlatformStatus {
    sections: Vec<Box<dyn PlatformSection>>,
}

impl PlatformStatus {
    #[must_use]
    pub fn new(sections: Vec<Box<dyn PlatformSection>>) -> Self {
        Self { sections }
    }

    fn section(&self, area: &str) -> Option<&dyn PlatformSection> {
        self.sections
            .iter()
            .find(|section| section.key() == area)
            .map(AsRef::as_ref)
    }
}

#[async_trait]
impl AiToolHandler for PlatformStatus {
    fn area(&self) -> Area {
        Area::Core
    }

    fn name(&self) -> &'static str {
        "get_platform_status"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Platform
    }

    fn description(&self) -> &'static str {
        "Estado do próprio NetMonitor por área: 'agents' (agentes remotos: conexão, versão, host), 'vpn' (servidor WireGuard e peers: handshake, tráfego), \
'wifi' (roteadores/APs e os aparelhos conectados: sinal, SNR, retransmissões, saúde e vigiados; os com problema primeiro), \
'discovery' (varreduras recentes e em andamento), 'networks' (redes/CIDR, sites e servidores DNS), 'maintenance' (janelas ativas e futuras), \
e 'notifications' (envios recentes, pendentes e suprimidos, com hours)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "area": { "type": "string", "enum": AREAS },
                "hours": { "type": "integer", "description": "Janela para notifications e discovery (1 a 720, padrão 24)" },
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
            "networks" => networks_overview(ctx, limit).await?,
            "maintenance" => maintenance(ctx, limit).await?,
            "notifications" => notifications(ctx, since, limit).await?,
            other => match self.section(other) {
                Some(section) => section.report(ctx, since, limit).await?,
                None => {
                    return Ok(ToolOutput::not_found(format!(
                        "Área '{area}' desconhecida; use {}",
                        AREAS.join(", ")
                    )))
                }
            },
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
