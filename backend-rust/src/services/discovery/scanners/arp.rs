//! Leitura de ARP no Linux após o sweep, sem parsear a saída localizada de CLI.

use crate::services::discovery::merger::DiscoveredHost;
#[cfg(target_os = "linux")]
use std::collections::BTreeSet;
use std::net::Ipv4Addr;
#[cfg(target_os = "linux")]
use tokio::io::AsyncReadExt;

pub async fn scan(allowed: &[Ipv4Addr]) -> Vec<DiscoveredHost> {
    #[cfg(target_os = "linux")]
    {
        let Ok(mut file) = tokio::fs::File::open("/proc/net/arp").await else {
            return vec![];
        };
        let mut content = String::new();
        if file.read_to_string(&mut content).await.is_err() {
            return vec![];
        }
        let allowed: BTreeSet<_> = allowed.iter().map(ToString::to_string).collect();
        return content
            .lines()
            .skip(1)
            .filter_map(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                (fields.len() >= 4 && allowed.contains(fields[0]) && valid_mac(fields[3])).then(
                    || DiscoveredHost {
                        ip_address: fields[0].into(),
                        mac_address: Some(fields[3].to_ascii_lowercase()),
                        confidence: 80,
                        data: serde_json::json!({ "scanner": "arp" }),
                        ..Default::default()
                    },
                )
            })
            .collect();
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = allowed;
        vec![]
    }
}
fn valid_mac(value: &str) -> bool {
    value != "00:00:00:00:00:00" && !value.starts_with("ff:") && value.split(':').count() == 6
}
