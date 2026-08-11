//! Discovery de portas comuns reutilizando a mesma estratégia de varredura.

use crate::services::{
    discovery::merger::DiscoveredHost,
    network_tools::port_scanner::{self, PortProtocol, PortScanEvent, ScanStrategy},
};
use std::net::IpAddr;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const COMMON_PORTS: &[u16] = &[80, 443, 22, 445, 8080, 8000, 3389, 161];
pub async fn enrich(
    mut hosts: Vec<DiscoveredHost>,
    cancel: CancellationToken,
) -> Vec<DiscoveredHost> {
    for host in &mut hosts {
        if cancel.is_cancelled() {
            break;
        }
        let Ok(ip) = host.ip_address.parse::<IpAddr>() else {
            continue;
        };
        let (sender, mut receiver) = mpsc::channel(16);
        let worker_cancel = cancel.child_token();
        let ports = COMMON_PORTS.to_vec();
        let worker = tokio::spawn(async move {
            port_scanner::scan(
                ip,
                &ports,
                PortProtocol::Tcp,
                ScanStrategy::with_timeout(800),
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
        host.confidence =
            (host.confidence + if host.open_ports.is_empty() { 0 } else { 20 }).min(100);
    }
    hosts
}
