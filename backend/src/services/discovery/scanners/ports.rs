//! Discovery de portas comuns reutilizando a mesma estratégia de varredura.

use crate::services::{
    discovery::{merger::DiscoveredHost, progress::ScanReporter},
    network_tools::port_scanner::{self, PortProtocol, PortScanEvent, ScanProfile, ScanStrategy},
};
use std::net::IpAddr;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const COMMON_PORTS: &[u16] = &[
    21, 22, 23, 53, 80, 139, 443, 445, 554, 3389, 8000, 8080, 8291, 8443, 9100,
];

/// Hosts sondados ao mesmo tempo.
///
/// Cada host custa ~800 ms de timeout, e em série uma faixa com 40 aparelhos
/// deixava a fase de portas mais de meio minuto sem sinal de vida. Oito de cada
/// vez são 64 conexões TCP simultâneas — folga confortável em qualquer sistema.
const CONCURRENT_HOSTS: usize = 8;

pub async fn enrich(
    hosts: Vec<DiscoveredHost>,
    cancel: CancellationToken,
    reporter: &ScanReporter,
) -> Vec<DiscoveredHost> {
    let total = hosts.len();
    let mut scanned = Vec::with_capacity(total);
    let mut done = 0;
    reporter.phase("ports", 0, total);

    // Os lotes preservam a ordem de entrada e mantêm o relatório previsível:
    // cada rodada publica o que já foi sondado até ali.
    for batch in hosts.chunks(CONCURRENT_HOSTS) {
        if cancel.is_cancelled() {
            scanned.extend_from_slice(batch);
            done += batch.len();
            continue;
        }
        let results = futures::future::join_all(
            batch
                .iter()
                .cloned()
                .map(|host| scan_host(host, cancel.clone())),
        )
        .await;
        done += results.len();
        scanned.extend(results);
        reporter.progress("ports", done, total);
        reporter.hosts(&scanned);
    }
    scanned
}

async fn scan_host(mut host: DiscoveredHost, cancel: CancellationToken) -> DiscoveredHost {
    let Ok(ip) = host.ip_address.parse::<IpAddr>() else {
        return host;
    };
    let (sender, mut receiver) = mpsc::channel(16);
    let worker_cancel = cancel.child_token();
    let ports = COMMON_PORTS.to_vec();
    let worker = tokio::spawn(async move {
        port_scanner::scan(
            ip,
            &ports,
            PortProtocol::Tcp,
            ScanStrategy::for_profile(ScanProfile::Reliable, 1_200),
            sender,
            worker_cancel,
        )
        .await;
    });
    while let Some(event) = receiver.recv().await {
        if let PortScanEvent::Result(item) = event {
            if item.status == "open" {
                host.open_ports.push(item.port);
            }
        }
    }
    let _ = worker.await;
    host.open_ports.sort_unstable();
    host.confidence = (host.confidence + if host.open_ports.is_empty() { 0 } else { 20 }).min(100);
    host
}
