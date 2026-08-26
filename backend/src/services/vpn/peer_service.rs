//! Regras de negócio dos peers (§8.10.4).
//!
//! Criação com provisionamento completo, rotação de chaves, revogação e geração
//! dos artefatos por perfil. A chave privada do cliente fica **apenas** em
//! memória, entregue uma única vez.

use std::net::Ipv4Addr;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};

use crate::{
    models::{devices, monitors, networks, vpn_peers, vpn_servers},
    services::{
        maintenance::resource_cleanup::ResourceCleanupService,
        shared::errors::{AppError, AppResult},
        vpn::{
            ip_allocator,
            key_generator::{generate_key_pair, generate_preshared_key},
            monitor_provisioner::{self, MonitorProvisioningOptions},
            peer_hints::{compute_peer_hints, PeerHints},
            profiles::{
                contract::PeerConfigContext, registry, GeneratedArtifact,
                PERSISTENT_KEEPALIVE_SECONDS, PRIVATE_KEY_UNAVAILABLE,
            },
            secret_store::{client_key_store, secret_key},
            server_service,
        },
    },
};

/// Endpoint exibido quando o operador ainda não configurou o endereço público.
const ENDPOINT_PLACEHOLDER: &str = "ENDERECO-PUBLICO-NAO-CONFIGURADO";

/// O sistema do catálogo que corresponde a um perfil do assistente.
///
/// `None` para perfil sem sistema declarado — que hoje não existe, e no dia em
/// que existir é melhor deixar o campo vazio do que gravar um palpite.
fn sistema_do_perfil(perfil: &str) -> Option<String> {
    crate::services::devices::systems::catalog()
        .iter()
        .find(|sistema| sistema.vpn_profile == Some(perfil))
        .map(|sistema| sistema.id.to_owned())
}

#[derive(Debug, Clone, Default)]
pub struct CreatePeerPayload {
    pub name: String,
    pub profile: String,
    pub ip_address: Option<String>,
    pub site_id: Option<i64>,
    pub snmp_enabled: bool,
    pub snmp_community: Option<String>,
    pub snmp_version: Option<String>,
    pub description: Option<String>,
}

pub struct PeerListItem {
    pub peer: vpn_peers::Model,
    pub device: Option<devices::Model>,
    pub hints: PeerHints,
}

/// Carrega peer + servidor + rede + dispositivo de uma vez.
struct PeerBundle {
    peer: vpn_peers::Model,
    device: Option<devices::Model>,
    server: vpn_servers::Model,
    network: networks::Model,
}

async fn load_peer(db: &DatabaseConnection, peer_id: i64) -> AppResult<PeerBundle> {
    let peer = vpn_peers::Entity::find_by_id(peer_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Peer da VPN não encontrado"))?;
    let server = vpn_servers::Entity::find_by_id(peer.vpn_server_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::business_rule("Servidor VPN ainda não foi configurado"))?;
    let network = server_service::network_of(db, &server).await?;
    let device = devices::Entity::find_by_id(peer.device_id).one(db).await?;
    Ok(PeerBundle {
        peer,
        device,
        server,
        network,
    })
}

/// Monta o contexto consumido pelos geradores por perfil.
fn build_context(
    bundle: &PeerBundle,
    client_private_key: Option<&str>,
) -> AppResult<PeerConfigContext> {
    let device = bundle.device.as_ref();
    Ok(PeerConfigContext {
        peer_name: device.map_or_else(
            || format!("peer-{}", bundle.peer.id),
            |device| device.name.clone(),
        ),
        peer_ip_address: device
            .and_then(|device| device.ip_address.clone())
            .unwrap_or_default(),
        vpn_cidr: bundle.network.cidr.clone(),
        server_vpn_address: server_service::server_address(&bundle.network)?.to_string(),
        client_private_key: client_private_key
            .unwrap_or(PRIVATE_KEY_UNAVAILABLE)
            .to_string(),
        server_public_key: bundle.server.public_key.clone(),
        preshared_key: bundle.peer.preshared_key()?,
        endpoint_host: bundle
            .server
            .public_endpoint
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| ENDPOINT_PLACEHOLDER.to_string()),
        endpoint_port: bundle.server.listen_port,
        mtu: bundle.server.mtu,
        dns_servers: bundle.server.dns_servers.clone(),
        snmp_enabled: device.is_some_and(|device| device.snmp_enabled),
        snmp_community: device.and_then(|device| device.snmp_community.clone()),
    })
}

fn generate_artifact(
    bundle: &PeerBundle,
    client_private_key: Option<&str>,
) -> AppResult<GeneratedArtifact> {
    let generator = registry::resolve(&bundle.peer.device_profile)?;
    Ok(generator.generate(&build_context(bundle, client_private_key)?))
}

/// Lista os peers com telemetria fresca e os avisos de diagnóstico.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn list(db: &DatabaseConnection) -> AppResult<Vec<PeerListItem>> {
    // Sem isto a lista mostra o que o scheduler gravou no ciclo anterior: o
    // operador que acabou de conectar o túnel precisaria recarregar a tela até
    // o background alcançá-lo.
    server_service::sync_telemetry(db).await;

    let peers = vpn_peers::Entity::find()
        .order_by_asc(vpn_peers::Column::Id)
        .all(db)
        .await?;
    if peers.is_empty() {
        return Ok(Vec::new());
    }

    let device_ids: Vec<i64> = peers.iter().map(|peer| peer.device_id).collect();
    let devices_by_id = devices::Entity::find()
        .filter(devices::Column::Id.is_in(device_ids.clone()))
        .all(db)
        .await?;
    let ping_monitors = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.is_in(device_ids))
        .filter(monitors::Column::Type.eq("ping"))
        .all(db)
        .await?;

    // Uma leitura só da topologia para a lista inteira.
    let probe_external = super::probe_is_external();
    Ok(peers
        .into_iter()
        .map(|peer| {
            let device = devices_by_id
                .iter()
                .find(|device| device.id == peer.device_id)
                .cloned();
            let monitor = ping_monitors
                .iter()
                .find(|monitor| monitor.device_id == Some(peer.device_id));
            let hints = compute_peer_hints(&peer, monitor, probe_external);
            PeerListItem {
                peer,
                device,
                hints,
            }
        })
        .collect())
}

/// Cria dispositivo, peer e monitores em uma única transação e só então
/// reescreve o `wg0.conf`.
///
/// # Errors
///
/// Propaga validação de perfil/IP e erro do banco.
pub async fn create(
    db: &DatabaseConnection,
    payload: &CreatePeerPayload,
) -> AppResult<(vpn_peers::Model, GeneratedArtifact)> {
    if !registry::has(&payload.profile) {
        return Err(AppError::business_rule(format!(
            "Perfil de equipamento não suportado: {}",
            payload.profile
        )));
    }
    let server = server_service::find_or_fail(db).await?;
    let network = server_service::network_of(db, &server).await?;

    let requested_ip = match payload.ip_address.as_deref().filter(|v| !v.is_empty()) {
        Some(raw) => {
            let ip: Ipv4Addr = raw
                .parse()
                .map_err(|_| AppError::business_rule(format!("Endereço IPv4 inválido: {raw}")))?;
            ip_allocator::assert_available(db, server.network_id, &network.cidr, ip).await?;
            Some(ip)
        }
        None => None,
    };

    let key_pair = generate_key_pair();
    let preshared_key = generate_preshared_key();

    let provision = |ip_address: Ipv4Addr| {
        let payload = payload.clone();
        let server = server.clone();
        let network = network.clone();
        let key_pair = key_pair.clone();
        let preshared_key = preshared_key.clone();
        async move {
            let txn = db.begin().await?;
            let device = devices::ActiveModel {
                site_id: Set(payload.site_id.or(network.site_id)),
                network_id: Set(Some(server.network_id)),
                ip_address: Set(Some(ip_address.to_string())),
                name: Set(payload.name.clone()),
                r#type: Set(
                    if payload.profile == "mikrotik" || payload.profile == "openwrt" {
                        "router".into()
                    } else {
                        "host".into()
                    },
                ),
                description: Set(Some(payload.description.clone().unwrap_or_else(|| {
                    "Dispositivo conectado via VPN WireGuard".to_string()
                }))),
                status: Set("unknown".into()),
                is_monitored: Set(true),
                // Aqui a forma de acesso não é dedução: o dispositivo está
                // nascendo **por causa** do túnel. Declará-la agora poupa a
                // pergunta em toda tela que depois precisar do endereço deste
                // servidor — a ativação de log, principalmente.
                access_mode: Set(Some("vpn".into())),
                // O sistema também já está respondido: é o perfil que o
                // operador escolheu para gerar a configuração. A tradução do
                // nome do gerador (`mikrotik`) para o do sistema (`routeros`)
                // mora no catálogo, e não numa segunda tabela aqui.
                operating_system: Set(sistema_do_perfil(&payload.profile)),
                snmp_enabled: Set(payload.snmp_enabled),
                snmp_community: Set(payload.snmp_enabled.then(|| {
                    payload
                        .snmp_community
                        .clone()
                        .unwrap_or_else(|| "public".into())
                })),
                snmp_version: Set(payload
                    .snmp_enabled
                    .then(|| payload.snmp_version.clone().unwrap_or_else(|| "v2c".into()))),
                ..Default::default()
            }
            .insert(&txn)
            .await?;

            let mut peer = vpn_peers::ActiveModel {
                vpn_server_id: Set(server.id),
                device_id: Set(device.id),
                public_key: Set(key_pair.public_key.clone()),
                device_profile: Set(payload.profile.clone()),
                persistent_keepalive: Set(PERSISTENT_KEEPALIVE_SECONDS),
                enabled: Set(true),
                bytes_rx: Set(0),
                bytes_tx: Set(0),
                ..Default::default()
            };
            peer.set_preshared_key(Some(&preshared_key))?;
            let peer = peer.insert(&txn).await?;

            monitor_provisioner::provision(
                &txn,
                &device,
                &MonitorProvisioningOptions {
                    snmp_enabled: payload.snmp_enabled,
                    snmp_community: device.snmp_community.clone(),
                    snmp_version: device.snmp_version.clone(),
                    interval_seconds: None,
                },
            )
            .await?;

            txn.commit().await?;
            Ok(peer)
        }
    };

    let peer = match requested_ip {
        Some(ip) => provision(ip).await?,
        None => {
            ip_allocator::allocate(db, server.network_id, &network.cidr, &[], provision).await?
        }
    };

    server_service::apply_configuration(db, &server, &network).await?;

    client_key_store().put(secret_key(peer.id), key_pair.private_key.clone());
    let bundle = load_peer(db, peer.id).await?;
    let artifact = generate_artifact(&bundle, Some(&key_pair.private_key))?;

    Ok((bundle.peer, artifact))
}

/// Renomeia o dispositivo do peer.
///
/// Existe separado do `PUT /api/devices/:id` de propósito: aquele endpoint
/// sincroniza "o primeiro monitor" do dispositivo, e um peer da VPN tem dois
/// (ping e SNMP) — o SNMP perderia community e versão se caísse ali.
///
/// # Errors
///
/// Propaga validação de nome e erro do banco.
pub async fn rename(
    db: &DatabaseConnection,
    peer_id: i64,
    name: &str,
) -> AppResult<(vpn_peers::Model, Option<devices::Model>)> {
    let new_name = name.trim();
    if new_name.is_empty() {
        return Err(AppError::business_rule("Informe o nome do dispositivo"));
    }

    let bundle = load_peer(db, peer_id).await?;
    let Some(device) = bundle.device.clone() else {
        return Ok((bundle.peer, None));
    };
    let previous_name = device.name.clone();
    if previous_name == new_name {
        return Ok((bundle.peer, Some(device)));
    }

    let txn = db.begin().await?;
    let mut active: devices::ActiveModel = device.into();
    active.name = Set(new_name.to_string());
    let device = active.update(&txn).await?;

    // Só acompanha os monitores que ainda usam o nome gerado no
    // provisionamento — um monitor renomeado à mão continua como está.
    let monitors_of_device = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .all(&txn)
        .await?;
    for monitor in monitors_of_device {
        let Some(prefix) = monitor_provisioner::generated_name_prefix(&monitor.r#type) else {
            continue;
        };
        if monitor.name != format!("{prefix} {previous_name}") {
            continue;
        }
        let mut active: monitors::ActiveModel = monitor.into();
        active.name = Set(format!("{prefix} {new_name}"));
        active.update(&txn).await?;
    }
    txn.commit().await?;

    // O `wg0.conf` traz o nome como comentário de cada peer.
    server_service::apply_configuration(db, &bundle.server, &bundle.network).await?;

    Ok((bundle.peer, Some(device)))
}

/// Gera novo par de chaves e PSK, invalidando imediatamente os anteriores.
///
/// # Errors
///
/// Propaga erro de cifra e de banco.
pub async fn rotate_keys(
    db: &DatabaseConnection,
    peer_id: i64,
) -> AppResult<(vpn_peers::Model, GeneratedArtifact)> {
    let bundle = load_peer(db, peer_id).await?;
    let key_pair = generate_key_pair();

    let mut active: vpn_peers::ActiveModel = bundle.peer.clone().into();
    active.public_key = Set(key_pair.public_key.clone());
    active.set_preshared_key(Some(&generate_preshared_key()))?;
    let peer = active.update(db).await?;

    server_service::apply_configuration(db, &bundle.server, &bundle.network).await?;

    client_key_store().put(secret_key(peer.id), key_pair.private_key.clone());
    let bundle = load_peer(db, peer.id).await?;
    let artifact = generate_artifact(&bundle, Some(&key_pair.private_key))?;

    Ok((bundle.peer, artifact))
}

/// Artefato de configuração do peer.
///
/// A chave privada só aparece na primeira leitura após criação/rotação; depois
/// vem o placeholder (matriz de paridade #33).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn build_artifact(db: &DatabaseConnection, peer_id: i64) -> AppResult<GeneratedArtifact> {
    let bundle = load_peer(db, peer_id).await?;
    let private_key = client_key_store().consume(&secret_key(peer_id));
    generate_artifact(&bundle, private_key.as_deref())
}

/// Regras de firewall do perfil — usadas no diagnóstico "não responde ao ping".
///
/// # Errors
///
/// Propaga erro do banco e perfil desconhecido.
pub async fn firewall_hints(
    db: &DatabaseConnection,
    peer_id: i64,
) -> AppResult<(String, &'static str, String)> {
    let bundle = load_peer(db, peer_id).await?;
    let generator = registry::resolve(&bundle.peer.device_profile)?;
    let context = build_context(&bundle, None)?;
    Ok((
        bundle.peer.device_profile.clone(),
        generator.label(),
        generator.firewall_hints(&context),
    ))
}

/// Revoga o peer, remove o dispositivo (liberando o IP) e reescreve o
/// `wg0.conf`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn revoke(db: &DatabaseConnection, peer_id: i64) -> AppResult<()> {
    let bundle = load_peer(db, peer_id).await?;
    let device_id = bundle.peer.device_id;

    vpn_peers::Entity::delete_by_id(peer_id).exec(db).await?;
    // Apagar o dispositivo é o que libera o IP (matriz de paridade #41): o
    // alocador olha `devices.ip_address`, não a tabela de peers.
    ResourceCleanupService::delete_device(db, device_id).await?;

    client_key_store().consume(&secret_key(peer_id));
    server_service::apply_configuration(db, &bundle.server, &bundle.network).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn bundle(endpoint: Option<&str>, device: Option<devices::Model>) -> PeerBundle {
        let now = Utc::now();
        PeerBundle {
            peer: vpn_peers::Model {
                id: 1,
                vpn_server_id: 1,
                device_id: 1,
                public_key: "PUB".into(),
                preshared_key_encrypted: None,
                device_profile: "linux".into(),
                persistent_keepalive: 25,
                last_handshake_at: None,
                last_seen_at: None,
                bytes_rx: 0,
                bytes_tx: 0,
                enabled: true,
                last_connection_status: None,
                created_at: now.into(),
                updated_at: now.into(),
            },
            device,
            server: vpn_servers::Model {
                id: 1,
                network_id: 1,
                interface_name: "wg0".into(),
                listen_port: 51_820,
                public_endpoint: endpoint.map(ToString::to_string),
                public_key: "SERVER-PUB".into(),
                private_key_encrypted: String::new(),
                allow_peer_to_peer: false,
                mtu: 1_420,
                dns_servers: None,
                active: true,
                last_synced_at: None,
                created_at: now.into(),
                updated_at: now.into(),
            },
            network: networks::Model {
                id: 1,
                site_id: None,
                probe_id: None,
                name: "VPN WireGuard".into(),
                cidr: "10.8.0.0/24".into(),
                gateway: Some("10.8.0.1".into()),
                vlan: None,
                dns_servers: None,
                scan_enabled: false,
                scan_interval: 3_600,
                active: true,
                last_scan_at: None,
                next_scan_at: None,
                created_at: now.into(),
                updated_at: now.into(),
            },
        }
    }

    fn device(name: &str, snmp: bool) -> devices::Model {
        let now = Utc::now();
        devices::Model {
            id: 1,
            site_id: None,
            network_id: Some(1),
            parent_id: None,
            ip_address: Some("10.8.0.11".into()),
            name: name.into(),
            r#type: "host".into(),
            vendor: None,
            model: None,
            serial_number: None,
            description: None,
            is_monitored: true,
            snmp_enabled: snmp,
            snmp_community: snmp.then(|| "netmon".to_string()),
            snmp_version: snmp.then(|| "v2c".to_string()),
            snmp_poll_interval_seconds: 60,
            access_mode: Some("vpn".into()),
            operating_system: Some("routeros".into()),
            syslog_server_address: None,
            system_key: None,
            status: "unknown".into(),
            last_seen_at: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    #[test]
    fn sem_chave_em_memoria_o_contexto_traz_o_placeholder() {
        let context = build_context(
            &bundle(Some("vpn.exemplo"), Some(device("filial", false))),
            None,
        )
        .unwrap();
        assert_eq!(context.client_private_key, PRIVATE_KEY_UNAVAILABLE);
        assert!(context.private_key_consumed());
    }

    #[test]
    fn endpoint_ausente_vira_placebo_legivel_e_nao_string_vazia() {
        // Um `Endpoint = :51820` no `.conf` faz o cliente falhar sem explicar.
        let context = build_context(&bundle(None, Some(device("filial", false))), None).unwrap();
        assert_eq!(context.endpoint_host, ENDPOINT_PLACEHOLDER);
        let vazio = build_context(&bundle(Some(""), Some(device("filial", false))), None).unwrap();
        assert_eq!(vazio.endpoint_host, ENDPOINT_PLACEHOLDER);
    }

    #[test]
    fn o_contexto_carrega_o_snmp_do_dispositivo() {
        let context = build_context(
            &bundle(Some("vpn.exemplo"), Some(device("filial", true))),
            None,
        )
        .unwrap();
        assert!(context.snmp_enabled);
        assert_eq!(context.community(), "netmon");
    }

    #[test]
    fn peer_sem_dispositivo_ainda_gera_contexto_utilizavel() {
        // O device pode ter sumido por uma limpeza manual: o artefato precisa
        // continuar montável para o operador conseguir revogar o peer.
        let context = build_context(&bundle(Some("vpn.exemplo"), None), None).unwrap();
        assert_eq!(context.peer_name, "peer-1");
        assert_eq!(context.peer_ip_address, "");
        assert!(!context.snmp_enabled);
    }
}
