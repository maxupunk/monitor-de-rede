//! Ciclo de vida do servidor WireGuard (§8.10.4).
//!
//! Configuração, chaves e sincronização do `wg0.conf`. A v1 opera com um único
//! servidor (uma interface) — a tabela aceita mais de uma linha para não travar
//! uma evolução futura, mas todo o §7.13 fala no singular.

use std::net::Ipv4Addr;

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::{
    models::{devices, networks, sites, vpn_peers, vpn_servers},
    services::{
        shared::errors::{AppError, AppResult},
        vpn::{
            cidr::{first_usable_address, parse_cidr},
            config_builder::{PeerEntryInput, ServerInterfaceInput},
            config_writer::{write_server_config, FileConfigSink},
            key_generator::generate_key_pair,
            peer_status,
        },
    },
};

pub const DEFAULT_VPN_CIDR: &str = "10.8.0.0/24";
pub const DEFAULT_LISTEN_PORT: i32 = 51_820;
pub const DEFAULT_MTU: i32 = 1_420;
pub const DEFAULT_INTERFACE: &str = "wg0";

/// Payload de `PUT /api/vpn/server`.
#[derive(Debug, Clone, Default)]
pub struct VpnServerPayload {
    pub cidr: Option<String>,
    pub site_id: Option<i64>,
    pub network_id: Option<i64>,
    pub listen_port: Option<i32>,
    pub public_endpoint: Option<String>,
    pub mtu: Option<i32>,
    pub dns_servers: Option<String>,
    pub allow_peer_to_peer: Option<bool>,
    pub active: Option<bool>,
}

/// Estado agregado exibido no painel.
#[derive(Debug, Clone)]
pub struct VpnServerState {
    pub server: Option<vpn_servers::Model>,
    pub cidr: Option<String>,
    pub server_address: Option<String>,
    pub peers_total: usize,
    pub peers_connected: usize,
    pub bytes_rx: i64,
    pub bytes_tx: i64,
}

/// Servidor VPN configurado (v1: instância única).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn find(db: &sea_orm::DatabaseConnection) -> AppResult<Option<vpn_servers::Model>> {
    Ok(vpn_servers::Entity::find()
        .order_by_asc(vpn_servers::Column::Id)
        .one(db)
        .await?)
}

/// # Errors
///
/// Devolve 400 quando a VPN ainda não foi configurada — é o que o wizard exibe.
pub async fn find_or_fail(db: &sea_orm::DatabaseConnection) -> AppResult<vpn_servers::Model> {
    find(db)
        .await?
        .ok_or_else(|| AppError::business_rule("Servidor VPN ainda não foi configurado"))
}

/// Rede (e portanto CIDR) do servidor.
///
/// # Errors
///
/// Propaga erro do banco; 400 se a rede sumiu.
pub async fn network_of(
    db: &sea_orm::DatabaseConnection,
    server: &vpn_servers::Model,
) -> AppResult<networks::Model> {
    networks::Entity::find_by_id(server.network_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::business_rule("A rede da VPN não existe mais"))
}

/// Endereço do NetMonitor dentro do túnel (primeiro IP utilizável do CIDR).
///
/// # Errors
///
/// Falha quando o CIDR da rede é inválido.
pub fn server_address(network: &networks::Model) -> AppResult<Ipv4Addr> {
    first_usable_address(&network.cidr)
}

fn sink() -> FileConfigSink {
    FileConfigSink::default()
}

/// Traz o `wg show dump` publicado pelo container para dentro do banco.
///
/// Precisa acontecer **antes** de qualquer leitura que exiba status ao
/// operador: o scheduler sincroniza em background, mas quem acabou de abrir a
/// tela não pode depender do próximo ciclo dele para ver o túnel que subiu
/// agora. Falha na leitura do arquivo não derruba a resposta — os dados
/// persistidos seguem válidos, só ficam um ciclo atrasados.
pub async fn sync_telemetry(db: &sea_orm::DatabaseConnection) {
    let Ok(Some(server)) = find(db).await else {
        return;
    };
    if let Err(error) =
        peer_status::sync_peers(db, &sink(), &server.interface_name, server.id).await
    {
        tracing::warn!(%error, "falha ao sincronizar a telemetria dos túneis");
    }
}

/// Cria a rede da VPN quando ainda não existe.
///
/// `networks.site_id` é opcional: usamos o Site informado ou o primeiro
/// cadastrado, e seguimos sem vínculo quando não há nenhum — inventar um Site
/// "Matriz" só para satisfazer a FK poluía a lista de locais de quem nunca
/// cadastrou um.
async fn resolve_network(
    db: &sea_orm::DatabaseConnection,
    payload: &VpnServerPayload,
) -> AppResult<networks::Model> {
    let cidr = payload
        .cidr
        .clone()
        .unwrap_or_else(|| DEFAULT_VPN_CIDR.to_string());
    // Valida cedo, antes de qualquer escrita.
    parse_cidr(&cidr)?;

    if let Some(network_id) = payload.network_id {
        let existing = networks::Entity::find_by_id(network_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::not_found("Rede não encontrada"))?;
        let mut active: networks::ActiveModel = existing.into();
        active.cidr = Set(cidr);
        return Ok(active.update(db).await?);
    }

    let site_id = match payload.site_id {
        Some(site_id) => Some(site_id),
        None => sites::Entity::find()
            .order_by_asc(sites::Column::Id)
            .one(db)
            .await?
            .map(|site| site.id),
    };

    Ok(networks::ActiveModel {
        site_id: Set(site_id),
        name: Set("VPN WireGuard".into()),
        cidr: Set(cidr.clone()),
        gateway: Set(Some(first_usable_address(&cidr)?.to_string())),
        scan_enabled: Set(false),
        scan_interval: Set(3_600),
        active: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Cria (com par de chaves novo) ou atualiza o servidor e reescreve o
/// `wg0.conf` — o watcher aplica com `syncconf`, sem derrubar túneis.
///
/// # Errors
///
fn validate_vpn_server_payload(payload: &VpnServerPayload) -> AppResult<()> {
    if let Some(port) = payload.listen_port {
        if !(1..=65535).contains(&port) {
            return Err(AppError::validation(
                "A porta de escuta deve estar entre 1 e 65535.",
            ));
        }
    }
    if let Some(mtu) = payload.mtu {
        if !(576..=9000).contains(&mtu) {
            return Err(AppError::validation(
                "O MTU deve estar entre 576 e 9000 bytes.",
            ));
        }
    }
    if let Some(endpoint) = payload
        .public_endpoint
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        let trimmed = endpoint.trim();
        if trimmed.contains('\n') || trimmed.contains('\r') || trimmed.contains(' ') {
            return Err(AppError::validation(
                "O endpoint público contém caracteres inválidos.",
            ));
        }
    }
    if let Some(dns) = payload
        .dns_servers
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        for ip_str in dns.split(',') {
            let ip_str = ip_str.trim();
            if !ip_str.is_empty() && ip_str.parse::<std::net::IpAddr>().is_err() {
                return Err(AppError::validation(format!(
                    "Endereço DNS inválido: '{ip_str}'."
                )));
            }
        }
    }
    Ok(())
}

pub async fn create_or_update(
    db: &sea_orm::DatabaseConnection,
    payload: &VpnServerPayload,
) -> AppResult<(vpn_servers::Model, networks::Model)> {
    validate_vpn_server_payload(payload)?;
    let server = match find(db).await? {
        None => {
            let network = resolve_network(db, payload).await?;
            let key_pair = generate_key_pair();
            let mut active = vpn_servers::ActiveModel {
                network_id: Set(network.id),
                interface_name: Set(DEFAULT_INTERFACE.into()),
                listen_port: Set(payload.listen_port.unwrap_or(DEFAULT_LISTEN_PORT)),
                public_endpoint: Set(payload.public_endpoint.clone()),
                public_key: Set(key_pair.public_key),
                allow_peer_to_peer: Set(payload.allow_peer_to_peer.unwrap_or(false)),
                mtu: Set(payload.mtu.unwrap_or(DEFAULT_MTU)),
                dns_servers: Set(payload.dns_servers.clone()),
                active: Set(payload.active.unwrap_or(true)),
                ..Default::default()
            };
            active.set_private_key(&key_pair.private_key)?;
            active.insert(db).await?
        }
        Some(current) => {
            // Trocar o CIDR mexe na rede, não no servidor: é a rede que carrega
            // a faixa, e o gateway precisa acompanhar.
            if let Some(cidr) = payload.cidr.as_deref() {
                let network = network_of(db, &current).await?;
                if network.cidr != cidr {
                    parse_cidr(cidr)?;
                    let mut active: networks::ActiveModel = network.into();
                    active.cidr = Set(cidr.to_string());
                    active.gateway = Set(Some(first_usable_address(cidr)?.to_string()));
                    active.update(db).await?;
                }
            }
            let mut active: vpn_servers::ActiveModel = current.clone().into();
            active.listen_port = Set(payload.listen_port.unwrap_or(current.listen_port));
            active.public_endpoint =
                Set(payload.public_endpoint.clone().or(current.public_endpoint));
            active.mtu = Set(payload.mtu.unwrap_or(current.mtu));
            active.dns_servers = Set(payload.dns_servers.clone().or(current.dns_servers));
            active.allow_peer_to_peer = Set(payload
                .allow_peer_to_peer
                .unwrap_or(current.allow_peer_to_peer));
            active.active = Set(payload.active.unwrap_or(current.active));
            active.update(db).await?
        }
    };

    let network = network_of(db, &server).await?;
    apply_configuration(db, &server, &network).await?;
    Ok((server, network))
}

/// Reescreve o arquivo de configuração com todos os peers habilitados.
///
/// # Errors
///
/// Propaga erro de cifra, de banco e de escrita no volume.
pub async fn apply_configuration(
    db: &sea_orm::DatabaseConnection,
    server: &vpn_servers::Model,
    network: &networks::Model,
) -> AppResult<String> {
    let peers = vpn_peers::Entity::find_enabled_for_server(server.id)
        .all(db)
        .await?;

    let mut entries = Vec::with_capacity(peers.len());
    for peer in peers {
        let device = devices::Entity::find_by_id(peer.device_id).one(db).await?;
        entries.push(PeerEntryInput {
            name: device
                .as_ref()
                .map_or_else(|| format!("peer-{}", peer.id), |device| device.name.clone()),
            public_key: peer.public_key.clone(),
            preshared_key: peer.preshared_key()?,
            ip_address: device
                .and_then(|device| device.ip_address)
                .unwrap_or_default(),
            enabled: peer.enabled,
        });
    }

    let contents = write_server_config(
        &sink(),
        &ServerInterfaceInput {
            interface_name: server.interface_name.clone(),
            address: server_address(network)?.to_string(),
            cidr: network.cidr.clone(),
            listen_port: server.listen_port,
            private_key: server.private_key()?,
            mtu: server.mtu,
            allow_peer_to_peer: server.allow_peer_to_peer,
        },
        &entries,
    )
    .await?;

    let mut active: vpn_servers::ActiveModel = server.clone().into();
    active.last_synced_at = Set(Some(Utc::now().into()));
    active.update(db).await?;

    Ok(contents)
}

/// Estado agregado do painel, sincronizando a telemetria antes de contar.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn get_state(db: &sea_orm::DatabaseConnection) -> AppResult<VpnServerState> {
    let Some(server) = find(db).await? else {
        return Ok(VpnServerState {
            server: None,
            cidr: None,
            server_address: None,
            peers_total: 0,
            peers_connected: 0,
            bytes_rx: 0,
            bytes_tx: 0,
        });
    };

    sync_telemetry(db).await;

    let network = network_of(db, &server).await?;
    let peers = vpn_peers::Entity::find()
        .filter(vpn_peers::Column::VpnServerId.eq(server.id))
        .all(db)
        .await?;

    Ok(VpnServerState {
        cidr: Some(network.cidr.clone()),
        server_address: Some(server_address(&network)?.to_string()),
        peers_total: peers.len(),
        peers_connected: peers
            .iter()
            .filter(|peer| {
                peer.connection_status()
                    == crate::models::vpn_peers::VpnPeerConnectionStatus::Connected
            })
            .count(),
        bytes_rx: peers.iter().map(|peer| peer.bytes_rx).sum(),
        bytes_tx: peers.iter().map(|peer| peer.bytes_tx).sum(),
        server: Some(server),
    })
}
