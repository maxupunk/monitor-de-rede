//! A varredura em dois estágios, lote a lote do CIDR.
//!
//! 1. **Quem está vivo** (`sweep`): ping em todo endereço; a tabela de
//!    vizinhos revela quem bloqueia ICMP mas responde ARP. Só quando a faixa
//!    não é do mesmo enlace (rede roteada, Docker em bridge) os mudos passam
//!    por um punhado de portas TCP — um `RST` já prova o host.
//! 2. **O que cada vivo é** (`identify`): portas da tabela de assinaturas,
//!    SNMP, nomes (PTR/NetBIOS) e a página web, em paralelo e só nos vivos.
//!
//! Antes, portas e SNMP rodavam em **todos** os endereços, oito hosts por vez:
//! um /24 vazio levava mais de um minuto esperando timeout. Agora o custo
//! acompanha o número de hosts que existem, não o tamanho da faixa.
//!
//! mDNS e SSDP pertencem à interface, não ao lote: rodam uma vez, em paralelo
//! com o primeiro sweep.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use reqwest::Client;
use tokio_util::sync::CancellationToken;

use super::{
    cidr_range::{expand_cidr_batch, parse_cidr_range, MAX_SCAN_HOSTS},
    fingerprints,
    merger::{merge_with_gateway, DiscoveredHost},
    progress::{phase, ScanReporter, Stage},
    scanners::{arp, http, icmp, mdns, names, ports, snmp, ssdp},
};
use crate::services::{
    monitoring::checkers::ping::PingClient,
    shared::{
        cidr::is_ip_in_cidr,
        errors::{AppError, AppResult},
    },
};

/// O que varrer: a faixa e, quando a rede tem, o gateway cadastrado.
#[derive(Debug, Clone)]
pub struct ScanTarget {
    pub cidr: String,
    pub gateway: Option<IpAddr>,
}

impl ScanTarget {
    #[must_use]
    pub fn new(cidr: impl Into<String>) -> Self {
        Self {
            cidr: cidr.into(),
            gateway: None,
        }
    }

    #[must_use]
    pub fn with_gateway(mut self, gateway: Option<&str>) -> Self {
        self.gateway = gateway.and_then(|value| value.trim().parse().ok());
        self
    }
}

fn ensure_running(cancel: &CancellationToken) -> AppResult<()> {
    if cancel.is_cancelled() {
        Err(AppError::BusinessRule("Varredura cancelada.".into()))
    } else {
        Ok(())
    }
}

/// Partes da barra de um lote: o sweep ocupa a primeira metade, a
/// identificação a segunda. A barra só anda para frente.
struct Batch {
    offset: usize,
    len: usize,
    total: usize,
}

impl Batch {
    /// Posição na barra a `percent`% do lote.
    const fn at(&self, percent: usize) -> usize {
        self.offset + self.len * percent / 100
    }
}

pub async fn scan(
    ping: &PingClient,
    target: &ScanTarget,
    cancel: CancellationToken,
    reporter: &ScanReporter,
) -> AppResult<Vec<DiscoveredHost>> {
    let range = parse_cidr_range(&target.cidr)?;
    let total = range.usable_hosts as usize;
    let gateway = target.gateway.map(|ip| ip.to_string());
    let client = http::client()?;
    reporter.phase(phase::SWEEP, 0, total);

    let mut multicast = Some(tokio::spawn(multicast(client.clone(), target.cidr.clone())));
    let mut merged: Vec<DiscoveredHost> = Vec::new();
    let mut offset = 0_u32;
    while offset < range.usable_hosts {
        ensure_running(&cancel)?;
        let addresses = expand_cidr_batch(&target.cidr, offset, MAX_SCAN_HOSTS as usize)?;
        if addresses.is_empty() {
            break;
        }
        let batch = Batch {
            offset: offset as usize,
            len: addresses.len(),
            total,
        };

        let presence = presence(ping, &addresses, &cancel, reporter, &batch).await?;
        let mut found = vec![presence.alive.clone()];
        if let Some(task) = multicast.take() {
            found.push(task.await.unwrap_or_default());
        }
        merged = merge_with_gateway(std::iter::once(merged).chain(found), gateway.as_deref());
        reporter.hosts(&merged);
        reporter.log(presence.summary(addresses.len()));

        // Primeiro quem já respondeu: a prova de vida por TCP dos mudos é uma
        // rajada de SYN para endereços que não existem, e um proxy de rede
        // (Docker Desktop, rootless) leva segundos para se recuperar dela —
        // feita antes, ela zerava as portas de todo mundo.
        ensure_running(&cancel)?;
        let in_batch: BTreeSet<String> = addresses.iter().map(ToString::to_string).collect();
        let living: Vec<&DiscoveredHost> = merged
            .iter()
            .filter(|host| in_batch.contains(&host.ip_address))
            .collect();
        let known: BTreeSet<String> = living.iter().map(|host| host.ip_address.clone()).collect();
        let identified = identify(
            &living,
            &client,
            target.gateway,
            &cancel,
            &reporter.stage(
                phase::IDENTIFY,
                batch.at(25),
                batch.at(80) - batch.at(25),
                total,
            ),
        )
        .await;
        merged = merge_with_gateway([merged, identified], gateway.as_deref());
        reporter.hosts(&merged);

        ensure_running(&cancel)?;
        let proven = prove_silent(&presence, &cancel, reporter, &batch).await;
        let newcomers: BTreeSet<String> = proven
            .iter()
            .map(|host| host.ip_address.clone())
            .filter(|ip| !known.contains(ip))
            .collect();
        if !proven.is_empty() {
            merged = merge_with_gateway([merged, proven], gateway.as_deref());
        }
        if !newcomers.is_empty() {
            let hosts: Vec<&DiscoveredHost> = merged
                .iter()
                .filter(|host| newcomers.contains(&host.ip_address))
                .collect();
            let identified = identify(
                &hosts,
                &client,
                target.gateway,
                &cancel,
                &reporter.stage(
                    phase::IDENTIFY,
                    batch.at(90),
                    batch.at(100) - batch.at(90),
                    total,
                ),
            )
            .await;
            merged = merge_with_gateway([merged, identified], gateway.as_deref());
        }
        reporter.hosts(&merged);
        reporter.log(identification_summary(
            merged
                .iter()
                .filter(|host| in_batch.contains(&host.ip_address)),
            newcomers.len(),
        ));

        offset = offset.saturating_add(addresses.len() as u32);
    }
    ensure_running(&cancel)?;
    Ok(merged)
}

/// O que o ping e a tabela de vizinhos deixam: quem já está provado e quem
/// ainda precisa de prova por TCP.
struct Presence {
    /// Responderam ao ping ou estão confirmados na tabela de vizinhos.
    alive: Vec<DiscoveredHost>,
    /// Mudos a provar por TCP.
    silent: Vec<IpAddr>,
    /// Vizinhos lembrados (com MAC), que só valem se o TCP provar.
    stale: Vec<DiscoveredHost>,
    pinged: usize,
    neighbors: usize,
}

impl Presence {
    fn summary(&self, addresses: usize) -> String {
        let mut line = format!(
            "{} de {addresses} endereço(s) responderam ao ping; {} na tabela de vizinhos (ARP).",
            self.pinged, self.neighbors
        );
        if self.neighbors == 0 && self.pinged > 0 {
            line.push_str(
                " Sem MAC visível nesta faixa (contêiner em bridge ou rede roteada): o fabricante \
                 pelo MAC fica indisponível — use o docker-compose.host.yml para ver a camada 2.",
            );
        }
        line
    }
}

/// Estágio 1: quem responde ICMP ou ARP.
async fn presence(
    ping: &PingClient,
    addresses: &[IpAddr],
    cancel: &CancellationToken,
    reporter: &ScanReporter,
    batch: &Batch,
) -> AppResult<Presence> {
    let stage = reporter.stage(
        phase::SWEEP,
        batch.at(0),
        batch.at(25) - batch.at(0),
        batch.total,
    );
    let icmp_hosts = icmp::scan(ping, addresses, cancel.clone(), &stage).await?;
    let pinged: BTreeSet<String> = icmp_hosts
        .iter()
        .map(|host| host.ip_address.clone())
        .collect();

    // O ping obriga o kernel a resolver o MAC de cada endereço: quem bloqueia
    // ICMP ainda aparece na tabela de vizinhos.
    let neighbors = arp::scan(addresses).await;
    let neighbor_count = neighbors.len();
    let on_link = neighbors.iter().any(arp::is_confirmed);
    let (confirmed, stale): (Vec<_>, Vec<_>) = neighbors
        .into_iter()
        .partition(|host| arp::is_confirmed(host) || pinged.contains(&host.ip_address));
    let known: BTreeSet<String> = pinged
        .iter()
        .cloned()
        .chain(confirmed.iter().map(|host| host.ip_address.clone()))
        .collect();

    // No mesmo enlace, quem não respondeu ARP não existe: só as lembranças
    // velhas da tabela precisam de prova. Fora dele, todo mudo é suspeito.
    let silent: Vec<IpAddr> = if on_link {
        stale
            .iter()
            .filter_map(|host| host.ip_address.parse().ok())
            .collect()
    } else {
        addresses
            .iter()
            .filter(|ip| !known.contains(&ip.to_string()))
            .copied()
            .collect()
    };
    Ok(Presence {
        pinged: pinged.len(),
        neighbors: neighbor_count,
        alive: [icmp_hosts, confirmed].concat(),
        silent,
        stale,
    })
}

/// Prova de vida por TCP dos mudos. Se a rede responde ping, quem não
/// respondeu quase sempre é Windows com firewall ou um painel web: poucas
/// portas bastam e a rajada fica pequena. Sem nenhum ping, a lista é a
/// completa.
async fn prove_silent(
    presence: &Presence,
    cancel: &CancellationToken,
    reporter: &ScanReporter,
    batch: &Batch,
) -> Vec<DiscoveredHost> {
    if presence.silent.is_empty() {
        return Vec::new();
    }
    let ports = if presence.pinged > 0 {
        fingerprints::LIVENESS_PORTS_LIGHT
    } else {
        fingerprints::LIVENESS_PORTS
    };
    let stage = reporter.stage(
        phase::SWEEP,
        batch.at(80),
        batch.at(90) - batch.at(80),
        batch.total,
    );
    let tcp_hosts = ports::liveness(&presence.silent, ports, cancel, &stage).await;
    let proven: BTreeSet<&str> = tcp_hosts
        .iter()
        .map(|host| host.ip_address.as_str())
        .collect();
    let stale_alive: Vec<DiscoveredHost> = presence
        .stale
        .iter()
        .filter(|host| proven.contains(host.ip_address.as_str()))
        .cloned()
        .collect();
    [stale_alive, tcp_hosts].concat()
}

/// Uma linha para o registro: o que a identificação conseguiu de cada fonte.
fn identification_summary<'a>(
    hosts: impl Iterator<Item = &'a DiscoveredHost>,
    proven_by_tcp: usize,
) -> String {
    let (mut total, mut with_ports, mut ports, mut snmp, mut named, mut web, mut mac) =
        (0, 0, 0, 0, 0, 0, 0);
    for host in hosts {
        total += 1;
        if !host.open_ports.is_empty() {
            with_ports += 1;
            ports += host.open_ports.len();
        }
        snmp += usize::from(host.data.get("snmp").is_some());
        named += usize::from(host.hostname.is_some() || host.mdns_name.is_some());
        web += usize::from(host.data.get("http").is_some());
        mac += usize::from(host.mac_address.is_some());
    }
    let tcp = if proven_by_tcp > 0 {
        format!(" {proven_by_tcp} achado(s) só por TCP (bloqueiam ping).")
    } else {
        String::new()
    };
    format!(
        "Identificação: {ports} porta(s) aberta(s) em {with_ports} de {total} host(s); \
         SNMP em {snmp}; nome em {named}; página web em {web}; MAC em {mac}.{tcp}"
    )
}

/// Estágio 2: o que cada host vivo é.
async fn identify(
    hosts: &[&DiscoveredHost],
    client: &Client,
    gateway: Option<IpAddr>,
    cancel: &CancellationToken,
    stage: &Stage,
) -> Vec<DiscoveredHost> {
    stage.begin();
    let ips: Vec<IpAddr> = hosts
        .iter()
        .filter_map(|host| host.ip_address.parse().ok())
        .collect();
    if ips.is_empty() {
        stage.advance(1, 1);
        return Vec::new();
    }
    let (port_hosts, snmp_hosts, name_hosts) = tokio::join!(
        ports::scan(&ips, cancel, stage),
        snmp::enrich(&ips, cancel),
        names::resolve(&ips, gateway),
    );

    // A página web depende das portas abertas — as de agora e as que o
    // sweep já tinha visto.
    let mut open = BTreeMap::<IpAddr, Vec<u16>>::new();
    for host in hosts.iter().copied().chain(port_hosts.iter()) {
        if let Ok(ip) = host.ip_address.parse() {
            open.entry(ip).or_default().extend(&host.open_ports);
        }
    }
    let http_hosts = http::fingerprint_all(client, &open, cancel).await;
    [port_hosts, snmp_hosts, name_hosts, http_hosts].concat()
}

/// mDNS e SSDP, filtrados para a faixa varrida — a interface ouve a rede
/// toda, e um aparelho de outra sub-rede não pertence a esta varredura.
async fn multicast(client: Client, cidr: String) -> Vec<DiscoveredHost> {
    let (mdns_hosts, ssdp_hosts) = tokio::join!(mdns::scan(), ssdp::scan(&client));
    mdns_hosts
        .into_iter()
        .chain(ssdp_hosts)
        .filter(|host| {
            host.ip_address
                .parse()
                .ok()
                .and_then(|ip| is_ip_in_cidr(ip, &cidr).ok())
                .unwrap_or(false)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lote_divide_a_barra_sem_sobrepor() {
        let batch = Batch {
            offset: 1_024,
            len: 254,
            total: 2_048,
        };
        assert_eq!(batch.at(0), 1_024);
        assert!(batch.at(25) < batch.at(80));
        assert_eq!(batch.at(100), 1_024 + 254);
    }

    #[test]
    fn resumo_diz_o_que_cada_fonte_achou_e_explica_a_falta_de_mac() {
        let presence = Presence {
            alive: Vec::new(),
            silent: Vec::new(),
            stale: Vec::new(),
            pinged: 48,
            neighbors: 0,
        };
        assert!(presence.summary(254).contains("Sem MAC visível"));
        let host = DiscoveredHost {
            ip_address: "10.0.0.1".into(),
            hostname: Some("gw".into()),
            open_ports: vec![22, 80],
            data: serde_json::json!({ "snmp": {}, "http": {} }),
            ..DiscoveredHost::default()
        };
        let line = identification_summary([&host].into_iter(), 2);
        assert!(line.contains("2 porta(s) aberta(s) em 1 de 1"), "{line}");
        assert!(
            line.contains("SNMP em 1; nome em 1; página web em 1; MAC em 0"),
            "{line}"
        );
        assert!(line.contains("2 achado(s) só por TCP"), "{line}");
    }

    #[test]
    fn alvo_aceita_gateway_valido_e_ignora_lixo() {
        let target = ScanTarget::new("10.0.0.0/24").with_gateway(Some(" 10.0.0.1 "));
        assert_eq!(target.gateway, Some("10.0.0.1".parse().unwrap()));
        assert!(ScanTarget::new("10.0.0.0/24")
            .with_gateway(Some("roteador"))
            .gateway
            .is_none());
    }
}
