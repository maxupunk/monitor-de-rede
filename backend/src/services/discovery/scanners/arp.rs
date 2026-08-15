//! Leitura das tabelas ARP/NDP no Linux após o sweep.

use crate::services::discovery::merger::DiscoveredHost;
#[cfg(target_os = "linux")]
use futures::stream::{self, StreamExt};
#[cfg(target_os = "linux")]
use std::collections::BTreeSet;
use std::net::IpAddr;
#[cfg(target_os = "linux")]
use tokio::io::AsyncReadExt;
#[cfg(target_os = "linux")]
use tokio::{net::TcpStream, process::Command, time};

#[cfg(target_os = "linux")]
const ARP_PRIME_PORTS: [u16; 2] = [80, 443];
#[cfg(target_os = "linux")]
const ARP_PRIME_CONCURRENCY: usize = 64;
#[cfg(target_os = "linux")]
const ARP_PRIME_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(300);

pub async fn scan(allowed: &[IpAddr]) -> Vec<DiscoveredHost> {
    #[cfg(target_os = "linux")]
    {
        prime_neighbor_cache(allowed).await;
        let allowed: BTreeSet<_> = allowed.iter().map(ToString::to_string).collect();
        let mut discovered = Vec::new();
        if let Ok(mut file) = tokio::fs::File::open("/proc/net/arp").await {
            let mut content = String::new();
            if file.read_to_string(&mut content).await.is_ok() {
                discovered.extend(parse_arp(&content, &allowed));
            }
        }
        if let Ok(output) = Command::new("ip")
            .args(["-6", "neigh", "show"])
            .output()
            .await
        {
            discovered.extend(parse_ndp(
                &String::from_utf8_lossy(&output.stdout),
                &allowed,
            ));
        }
        return discovered;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = allowed;
        vec![]
    }
}

/// Dispara conexões curtas para que o kernel possa resolver os vizinhos antes
/// de lermos o cache ARP. A conexão não precisa completar: a tentativa TCP já
/// é suficiente para provocar ARP e cada alvo continua estritamente limitado.
#[cfg(target_os = "linux")]
async fn prime_neighbor_cache(allowed: &[IpAddr]) {
    stream::iter(
        allowed
            .iter()
            .copied()
            .flat_map(|ip| ARP_PRIME_PORTS.map(move |port| (ip, port))),
    )
    .for_each_concurrent(Some(ARP_PRIME_CONCURRENCY), |(ip, port)| async move {
        let _ = time::timeout(ARP_PRIME_TIMEOUT, TcpStream::connect((ip, port))).await;
    })
    .await;
}

#[cfg(target_os = "linux")]
fn valid_mac(value: &str) -> bool {
    value != "00:00:00:00:00:00" && !value.starts_with("ff:") && value.split(':').count() == 6
}

#[cfg(target_os = "linux")]
fn parse_arp(content: &str, allowed: &BTreeSet<String>) -> Vec<DiscoveredHost> {
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.len() >= 4 && allowed.contains(fields[0]) && valid_mac(fields[3])).then(|| {
                DiscoveredHost {
                    ip_address: fields[0].into(),
                    mac_address: Some(fields[3].to_ascii_lowercase()),
                    confidence: 80,
                    data: serde_json::json!({ "scanner": "arp" }),
                    ..Default::default()
                }
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn parse_ndp(content: &str, allowed: &BTreeSet<String>) -> Vec<DiscoveredHost> {
    content
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            let ip = fields.first()?;
            let mac_index = fields.iter().position(|field| *field == "lladdr")?;
            let mac = *fields.get(mac_index + 1)?;
            (allowed.contains(*ip) && valid_mac(mac)).then(|| DiscoveredHost {
                ip_address: (*ip).to_string(),
                mac_address: Some(mac.to_ascii_lowercase()),
                confidence: 80,
                data: serde_json::json!({ "scanner": "ndp" }),
                ..Default::default()
            })
        })
        .collect()
}
