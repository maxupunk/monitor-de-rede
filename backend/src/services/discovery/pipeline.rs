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
    merger::{merge_with_gateway, DiscoveredHost},
    progress::{phase, ScanReporter},
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
    const fn split(&self, from_percent: usize, to_percent: usize) -> (usize, usize) {
        let start = self.offset + self.len * from_percent / 100;
        let end = self.offset + self.len * to_percent / 100;
        (start, end - start)
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

        let alive = sweep(ping, &addresses, &cancel, reporter, &batch).await?;
        let mut found = vec![alive];
        if let Some(task) = multicast.take() {
            found.push(task.await.unwrap_or_default());
        }
        merged = merge_with_gateway(std::iter::once(merged).chain(found), gateway.as_deref());
        reporter.hosts(&merged);

        ensure_running(&cancel)?;
        let in_batch: BTreeSet<String> = addresses.iter().map(ToString::to_string).collect();
        let living: Vec<&DiscoveredHost> = merged
            .iter()
            .filter(|host| in_batch.contains(&host.ip_address))
            .collect();
        reporter.log(format!(
            "{} host(s) ativo(s) entre {} endereço(s); identificando…",
            living.len(),
            addresses.len()
        ));
        let identified =
            identify(&living, &client, target.gateway, &cancel, reporter, &batch).await;
        merged = merge_with_gateway([merged, identified], gateway.as_deref());
        reporter.hosts(&merged);

        offset = offset.saturating_add(addresses.len() as u32);
    }
    ensure_running(&cancel)?;
    Ok(merged)
}

/// Estágio 1: quem responde ICMP, ARP ou TCP.
async fn sweep(
    ping: &PingClient,
    addresses: &[IpAddr],
    cancel: &CancellationToken,
    reporter: &ScanReporter,
    batch: &Batch,
) -> AppResult<Vec<DiscoveredHost>> {
    let (start, span) = batch.split(0, 30);
    let stage = reporter.stage(phase::SWEEP, start, span, batch.total);
    let icmp_hosts = icmp::scan(ping, addresses, cancel.clone(), &stage).await?;
    let pinged: BTreeSet<String> = icmp_hosts
        .iter()
        .map(|host| host.ip_address.clone())
        .collect();

    // O ping obriga o kernel a resolver o MAC de cada endereço: quem bloqueia
    // ICMP ainda aparece na tabela de vizinhos.
    let neighbors = arp::scan(addresses).await;
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
    let (start, span) = batch.split(30, 50);
    let tcp_hosts = ports::liveness(
        &silent,
        cancel,
        &reporter.stage(phase::SWEEP, start, span, batch.total),
    )
    .await;
    let proven: BTreeSet<&str> = tcp_hosts
        .iter()
        .map(|host| host.ip_address.as_str())
        .collect();
    let stale_alive: Vec<DiscoveredHost> = stale
        .iter()
        .filter(|host| proven.contains(host.ip_address.as_str()))
        .cloned()
        .collect();

    Ok([icmp_hosts, confirmed, stale_alive, tcp_hosts].concat())
}

/// Estágio 2: o que cada host vivo é.
async fn identify(
    hosts: &[&DiscoveredHost],
    client: &Client,
    gateway: Option<IpAddr>,
    cancel: &CancellationToken,
    reporter: &ScanReporter,
    batch: &Batch,
) -> Vec<DiscoveredHost> {
    let (start, span) = batch.split(50, 100);
    let stage = reporter.stage(phase::IDENTIFY, start, span, batch.total);
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
        ports::scan(&ips, cancel, &stage),
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
        let (sweep_start, sweep_span) = batch.split(0, 50);
        let (identify_start, identify_span) = batch.split(50, 100);
        assert_eq!(sweep_start, 1_024);
        assert_eq!(sweep_start + sweep_span, identify_start);
        assert_eq!(identify_start + identify_span, 1_024 + 254);
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
