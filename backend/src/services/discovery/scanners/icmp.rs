use std::{net::IpAddr, time::Duration};

use futures::{stream, StreamExt};
use surge_ping::{PingIdentifier, PingSequence};
use tokio_util::sync::CancellationToken;

use crate::services::{
    discovery::{merger::DiscoveredHost, progress::ScanReporter},
    monitoring::checkers::ping::PingClient,
    shared::errors::AppResult,
};

/// Sweep ICMP usando o socket DGRAM compartilhado; não abre raw socket por host.
///
/// O varrimento é concorrente, mas o resultado sai pelo `reporter` à medida que
/// cada host responde: quem está olhando a tela vê a lista crescer durante o
/// ping, em vez de esperar a faixa inteira terminar para ver tudo de uma vez.
pub async fn scan(
    client: &PingClient,
    hosts: &[IpAddr],
    cancel: CancellationToken,
    reporter: &ScanReporter,
) -> AppResult<Vec<DiscoveredHost>> {
    let total = hosts.len();
    let mut attempts = stream::iter(hosts.iter().copied())
        .map(|ip| {
            let client = client.clone();
            let cancel = cancel.clone();
            async move {
                let client = client.for_ip(ip)?;
                let mut pinger = client.pinger(ip, PingIdentifier(rand::random())).await;
                pinger.timeout(Duration::from_millis(1_500));
                let result = tokio::select! {
                    () = cancel.cancelled() => return None,
                    result = pinger.ping(PingSequence(0), &[]) => result,
                };
                match result {
                    Ok(_) => Some(DiscoveredHost {
                        ip_address: ip.to_string(),
                        confidence: 50,
                        data: serde_json::json!({ "scanner": "icmp" }),
                        ..Default::default()
                    }),
                    Err(_) => None,
                }
            }
        })
        .buffer_unordered(64);

    let mut discovered = Vec::new();
    let mut tested = 0;
    while let Some(outcome) = attempts.next().await {
        tested += 1;
        if let Some(host) = outcome {
            discovered.push(host);
            reporter.hosts(&discovered);
        }
        reporter.progress("icmp", tested, total);
    }
    Ok(discovered)
}
