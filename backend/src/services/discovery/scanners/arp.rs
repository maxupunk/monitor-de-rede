//! Vizinhos (ARP/NDP) da faixa varrida, lidos depois do ping.
//!
//! O próprio ping obriga o kernel a resolver o MAC de cada endereço: quem
//! bloqueia ICMP ainda responde ARP e aparece aqui. A leitura é a de
//! [`neighbor_cache`], a mesma da auditoria de conflitos.

use std::{collections::BTreeSet, net::IpAddr};

use crate::services::{discovery::merger::DiscoveredHost, network_tools::neighbor_cache};

/// Estados em que a entrada foi confirmada há pouco. `STALE`/`DELAY` são
/// lembranças: o host pode ter saído da rede desde então.
const CONFIRMED_STATES: &[&str] = &["REACHABLE", "PERMANENT", "NOARP"];

pub async fn scan(allowed: &[IpAddr]) -> Vec<DiscoveredHost> {
    let allowed: BTreeSet<_> = allowed.iter().map(ToString::to_string).collect();
    neighbor_cache::read_system_neighbors()
        .await
        .into_iter()
        .filter(|entry| allowed.contains(&entry.ip_address))
        .map(|entry| DiscoveredHost {
            ip_address: entry.ip_address,
            mac_address: Some(entry.mac_address),
            confidence: 80,
            data: serde_json::json!({
                "scanner": "arp",
                "neighborState": entry.state,
            }),
            ..Default::default()
        })
        .collect()
}

/// A entrada prova sozinha que o host está na rede agora?
#[must_use]
pub fn is_confirmed(host: &DiscoveredHost) -> bool {
    host.data
        .get("neighborState")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|state| CONFIRMED_STATES.contains(&state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_entrada_recente_prova_presenca() {
        let with_state = |state: &str| DiscoveredHost {
            data: serde_json::json!({ "neighborState": state }),
            ..Default::default()
        };
        assert!(is_confirmed(&with_state("REACHABLE")));
        assert!(!is_confirmed(&with_state("STALE")));
        assert!(!is_confirmed(&with_state("DELAY")));
        assert!(is_confirmed(&DiscoveredHost::default()));
    }
}
