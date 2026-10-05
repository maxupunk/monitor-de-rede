//! Portas TCP da descoberta, em duas medidas:
//!
//! * [`liveness`] — poucas portas em todo endereço que ficou mudo no ping e
//!   no ARP: um `RST` já prova que o host existe (o `-Pn` do nmap).
//! * [`scan`] — a tabela inteira de [`fingerprints::PORT_RULES`], só nos hosts
//!   vivos, para dizer o que cada um é.
//!
//! As duas jogam todos os pares host×porta num único fluxo com teto global de
//! conexões: rodar host por host deixava a fase esperando o host mais lento.

use std::{collections::BTreeMap, net::IpAddr, time::Duration};

use futures::{stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::services::{
    discovery::{fingerprints, merger::DiscoveredHost, progress::Stage},
    network_tools::tcp_probe::{probe_tcp, TcpProbeState},
};

/// Conexões TCP abertas ao mesmo tempo, somando todos os hosts.
const CONCURRENCY: usize = 256;
/// Prova de vida: quem existe responde (SYN-ACK ou RST) em milissegundos.
const LIVENESS_TIMEOUT: Duration = Duration::from_millis(600);
/// Porta filtrada por firewall só responde com silêncio; esperar mais que
/// isso não muda a conclusão, só atrasa a fase.
const PORT_TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Default)]
struct Observed {
    alive: bool,
    open: Vec<u16>,
}

async fn sweep(
    ips: &[IpAddr],
    ports: &[u16],
    timeout: Duration,
    cancel: &CancellationToken,
    stage: &Stage,
) -> BTreeMap<IpAddr, Observed> {
    let pairs: Vec<(IpAddr, u16)> = ips
        .iter()
        .flat_map(|ip| ports.iter().map(move |port| (*ip, *port)))
        .collect();
    let total = pairs.len();
    let cancel = cancel.clone();
    let mut probes = stream::iter(pairs)
        .map(move |(ip, port)| {
            let cancel = cancel.clone();
            async move {
                if cancel.is_cancelled() {
                    return (ip, port, TcpProbeState::Error);
                }
                (ip, port, probe_tcp((ip, port), timeout).await.state)
            }
        })
        .buffer_unordered(CONCURRENCY);

    let mut observed = BTreeMap::<IpAddr, Observed>::new();
    let mut done = 0;
    while let Some((ip, port, state)) = probes.next().await {
        done += 1;
        if state.proves_reachability() {
            let entry = observed.entry(ip).or_default();
            entry.alive = true;
            if state == TcpProbeState::Open {
                entry.open.push(port);
            }
        }
        stage.advance(done, total);
    }
    observed
}

fn into_hosts(observed: BTreeMap<IpAddr, Observed>, scanner: &str) -> Vec<DiscoveredHost> {
    observed
        .into_iter()
        .filter(|(_, seen)| seen.alive)
        .map(|(ip, mut seen)| {
            seen.open.sort_unstable();
            DiscoveredHost {
                ip_address: ip.to_string(),
                open_ports: seen.open,
                confidence: 60,
                data: serde_json::json!({ "scanner": scanner }),
                ..Default::default()
            }
        })
        .collect()
}

/// Hosts que provaram existir por TCP, com as portas que já vieram abertas.
pub async fn liveness(
    addresses: &[IpAddr],
    ports: &[u16],
    cancel: &CancellationToken,
    stage: &Stage,
) -> Vec<DiscoveredHost> {
    stage.begin();
    let observed = sweep(addresses, ports, LIVENESS_TIMEOUT, cancel, stage).await;
    into_hosts(observed, "tcp")
}

/// Portas abertas de cada host vivo, pela tabela de assinaturas.
pub async fn scan(
    ips: &[IpAddr],
    cancel: &CancellationToken,
    stage: &Stage,
) -> Vec<DiscoveredHost> {
    let ports: Vec<u16> = fingerprints::PORT_RULES
        .iter()
        .map(|rule| rule.port)
        .collect();
    let observed = sweep(ips, &ports, PORT_TIMEOUT, cancel, stage).await;
    into_hosts(observed, "tcp")
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    #[tokio::test]
    async fn porta_aberta_e_porta_fechada_provam_o_host() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let open = listener.local_addr().unwrap().port();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let observed = sweep(
            &[ip],
            &[open, 1],
            Duration::from_secs(5),
            &CancellationToken::new(),
            &Stage::silent(),
        )
        .await;
        let hosts = into_hosts(observed, "tcp");
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].open_ports, vec![open]);
    }

    #[tokio::test]
    async fn cancelado_nao_abre_conexao() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let hosts = liveness(
            &[ip],
            fingerprints::LIVENESS_PORTS,
            &cancel,
            &Stage::silent(),
        )
        .await;
        assert!(hosts.is_empty());
    }
}
