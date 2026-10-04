use std::{net::IpAddr, time::Duration};

use futures::{stream, StreamExt};
use surge_ping::{PingIdentifier, PingSequence};
use tokio_util::sync::CancellationToken;

use crate::services::{
    discovery::{merger::DiscoveredHost, progress::Stage},
    monitoring::checkers::ping::PingClient,
    shared::errors::AppResult,
};

/// Pings em voo ao mesmo tempo. O socket DGRAM é um só; o custo de cada eco a
/// mais é um pacote, e o tempo da faixa é o timeout dividido por este número.
const CONCURRENCY: usize = 128;
/// Numa LAN a resposta vem em milissegundos; um segundo cobre enlaces de rádio
/// e ainda mantém um /24 abaixo de três segundos.
const TIMEOUT: Duration = Duration::from_millis(1_000);

/// Sweep ICMP usando o socket DGRAM compartilhado; não abre raw socket por host.
pub async fn scan(
    client: &PingClient,
    hosts: &[IpAddr],
    cancel: CancellationToken,
    stage: &Stage,
) -> AppResult<Vec<DiscoveredHost>> {
    let total = hosts.len();
    stage.begin();
    let mut attempts = stream::iter(hosts.iter().copied())
        .map(|ip| {
            let client = client.clone();
            let cancel = cancel.clone();
            async move {
                let client = client.for_ip(ip)?;
                let mut pinger = client.pinger(ip, PingIdentifier(rand::random())).await;
                pinger.timeout(TIMEOUT);
                let result = tokio::select! {
                    () = cancel.cancelled() => return None,
                    result = pinger.ping(PingSequence(0), &[]) => result,
                };
                result.ok().map(|(_, rtt)| DiscoveredHost {
                    ip_address: ip.to_string(),
                    confidence: 50,
                    data: serde_json::json!({
                        "scanner": "icmp",
                        "latencyMs": crate::services::network_tools::icmp_probe::duration_to_ms(rtt),
                    }),
                    ..Default::default()
                })
            }
        })
        .buffer_unordered(CONCURRENCY);

    let mut discovered = Vec::new();
    let mut tested = 0;
    while let Some(outcome) = attempts.next().await {
        tested += 1;
        if let Some(host) = outcome {
            discovered.push(host);
        }
        stage.advance(tested, total);
    }
    Ok(discovered)
}
