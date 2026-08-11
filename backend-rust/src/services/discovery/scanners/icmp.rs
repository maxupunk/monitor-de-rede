use std::{net::Ipv4Addr, time::Duration};

use futures::{stream, StreamExt};
use surge_ping::{PingIdentifier, PingSequence};

use crate::services::{
    discovery::merger::DiscoveredHost, monitoring::checkers::ping::PingClient,
    shared::errors::AppResult,
};

/// Sweep ICMP usando o socket DGRAM compartilhado; não abre raw socket por host.
pub async fn scan(client: &PingClient, hosts: &[Ipv4Addr]) -> AppResult<Vec<DiscoveredHost>> {
    let discovered = stream::iter(hosts.iter().copied())
        .map(|ip| {
            let client = client.clone();
            async move {
                let mut pinger = client
                    .0
                    .pinger(ip.into(), PingIdentifier(rand::random()))
                    .await;
                pinger.timeout(Duration::from_millis(1_500));
                match pinger.ping(PingSequence(0), &[]).await {
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
        .buffer_unordered(64)
        .filter_map(std::future::ready)
        .collect()
        .await;
    Ok(discovered)
}
