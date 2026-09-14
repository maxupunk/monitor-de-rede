//! Leitura e agrupamento da tabela de vizinhos (ARP/NDP) da rede local.

use std::collections::{BTreeMap, BTreeSet};

/// Entrada de vizinho resolvida na rede local.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeighborEntry {
    pub ip_address: String,
    pub mac_address: String,
    pub interface: Option<String>,
    pub state: Option<String>,
}

/// Normaliza um endereço MAC no formato padrão `aa:bb:cc:dd:ee:ff`.
#[must_use]
pub fn normalize_mac(mac: &str) -> Option<String> {
    let clean = mac.trim().to_ascii_lowercase().replace('-', ":");
    let parts: Vec<&str> = clean.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    for part in &parts {
        if part.len() != 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
    }
    if clean == "00:00:00:00:00:00" || clean.starts_with("ff:ff:ff") {
        return None;
    }
    Some(clean)
}

/// Valida se uma string é um MAC válido.
#[must_use]
pub fn is_valid_mac(mac: &str) -> bool {
    normalize_mac(mac).is_some()
}

/// Lê as entradas de vizinhos do sistema operacional.
pub async fn read_system_neighbors() -> Vec<NeighborEntry> {
    #[cfg(target_os = "linux")]
    {
        let mut entries = Vec::new();
        if let Ok(content) = tokio::fs::read_to_string("/proc/net/arp").await {
            entries.extend(parse_proc_arp(&content));
        }
        if let Ok(output) = tokio::process::Command::new("ip")
            .args(["neigh", "show"])
            .output()
            .await
        {
            let text = String::from_utf8_lossy(&output.stdout);
            entries.extend(parse_ip_neigh(&text));
        }
        dedup_neighbors(entries)
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Em ambientes de desenvolvimento como Windows/macOS sem Linux nativo,
        // retorna lista vazia por padrão.
        Vec::new()
    }
}

/// Faz a leitura do arquivo `/proc/net/arp`.
#[must_use]
pub fn parse_proc_arp(content: &str) -> Vec<NeighborEntry> {
    content
        .lines()
        .skip(1) // cabeçalho
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 6 {
                return None;
            }
            let ip = fields[0];
            let flags = fields[2];
            // Flag 0x0 indica entrada incompleta/sem resposta
            if flags == "0x0" {
                return None;
            }
            let mac = normalize_mac(fields[3])?;
            let interface = Some(fields[5].to_string());
            Some(NeighborEntry {
                ip_address: ip.to_string(),
                mac_address: mac,
                interface,
                state: Some("REACHABLE".to_string()),
            })
        })
        .collect()
}

/// Analisa a saída do comando `ip neigh show`.
#[must_use]
pub fn parse_ip_neigh(content: &str) -> Vec<NeighborEntry> {
    content
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.is_empty() {
                return None;
            }
            let ip = fields[0];
            // Ignora entradas que falharam
            if fields.iter().any(|f| *f == "FAILED" || *f == "INCOMPLETE") {
                return None;
            }
            let dev_idx = fields.iter().position(|f| *f == "dev");
            let interface = dev_idx
                .and_then(|idx| fields.get(idx + 1))
                .map(ToString::to_string);

            let lladdr_idx = fields.iter().position(|f| *f == "lladdr")?;
            let raw_mac = fields.get(lladdr_idx + 1)?;
            let mac = normalize_mac(raw_mac)?;

            let state = fields.last().map(ToString::to_string);

            Some(NeighborEntry {
                ip_address: ip.to_string(),
                mac_address: mac,
                interface,
                state,
            })
        })
        .collect()
}

/// Remove duplicatas preservando a primeira observação válida de cada par (IP, MAC).
#[must_use]
pub fn dedup_neighbors(entries: Vec<NeighborEntry>) -> Vec<NeighborEntry> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for entry in entries {
        let key = (entry.ip_address.clone(), entry.mac_address.clone());
        if seen.insert(key) {
            result.push(entry);
        }
    }
    result
}

/// Agrupa as entradas pelo endereço IP para identificar colisões/IPs clonados
/// (mais de um MAC diferente reclamando o mesmo IP).
#[must_use]
pub fn find_ip_collisions(entries: &[NeighborEntry]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in entries {
        map.entry(entry.ip_address.clone())
            .or_default()
            .insert(entry.mac_address.clone());
    }
    map.into_iter()
        .filter(|(_, macs)| macs.len() > 1)
        .map(|(ip, macs)| (ip, macs.into_iter().collect()))
        .collect()
}

/// Agrupa as entradas pelo endereço MAC para identificar clonagem/duplicação
/// (o mesmo MAC presente em múltiplos IPs simultaneamente).
#[must_use]
pub fn find_mac_duplicates(entries: &[NeighborEntry]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in entries {
        map.entry(entry.mac_address.clone())
            .or_default()
            .insert(entry.ip_address.clone());
    }
    map.into_iter()
        .filter(|(_, ips)| ips.len() > 1)
        .map(|(mac, ips)| (mac, ips.into_iter().collect()))
        .collect()
}

/// Busca o endereço IP atual de um MAC específico na lista de vizinhos.
#[must_use]
pub fn find_ips_for_mac(entries: &[NeighborEntry], mac: &str) -> Vec<String> {
    let Some(target_mac) = normalize_mac(mac) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|entry| entry.mac_address == target_mac)
        .map(|entry| entry.ip_address.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizacao_de_mac() {
        assert_eq!(
            normalize_mac("48-8F-5A-12-34-56"),
            Some("48:8f:5a:12:34:56".into())
        );
        assert_eq!(
            normalize_mac(" 48:8f:5a:12:34:56 "),
            Some("48:8f:5a:12:34:56".into())
        );
        assert_eq!(normalize_mac("00:00:00:00:00:00"), None);
        assert_eq!(normalize_mac("invalido"), None);
        assert_eq!(normalize_mac("00:11:22:33:44"), None);
    }

    #[test]
    fn parse_proc_arp_valido() {
        let sample = "\
IP address       HW type     Flags       HW address            Mask     Device
10.0.0.1         0x1         0x2         00:11:22:33:44:55     *        eth0
10.0.0.108       0x1         0x2         48:8f:5a:aa:bb:cc     *        eth0
10.0.0.200       0x1         0x0         00:00:00:00:00:00     *        eth0
";
        let parsed = parse_proc_arp(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].ip_address, "10.0.0.1");
        assert_eq!(parsed[0].mac_address, "00:11:22:33:44:55");
        assert_eq!(parsed[1].ip_address, "10.0.0.108");
        assert_eq!(parsed[1].mac_address, "48:8f:5a:aa:bb:cc");
    }

    #[test]
    fn parse_ip_neigh_valido() {
        let sample = "\
10.0.0.1 dev eth0 lladdr 00:11:22:33:44:55 REACHABLE
10.0.0.119 dev eth0 lladdr 48:8f:5a:aa:bb:cc STALE
10.0.0.50 dev eth0 FAILED
";
        let parsed = parse_ip_neigh(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].ip_address, "10.0.0.1");
        assert_eq!(parsed[1].ip_address, "10.0.0.119");
        assert_eq!(parsed[1].mac_address, "48:8f:5a:aa:bb:cc");
    }

    #[test]
    fn detecta_colisao_de_ip() {
        let entries = vec![
            NeighborEntry {
                ip_address: "10.0.0.50".into(),
                mac_address: "00:11:22:33:44:55".into(),
                interface: Some("eth0".into()),
                state: None,
            },
            NeighborEntry {
                ip_address: "10.0.0.50".into(),
                mac_address: "66:77:88:99:aa:bb".into(),
                interface: Some("eth0".into()),
                state: None,
            },
            NeighborEntry {
                ip_address: "10.0.0.1".into(),
                mac_address: "aa:bb:cc:dd:ee:ff".into(),
                interface: Some("eth0".into()),
                state: None,
            },
        ];
        let collisions = find_ip_collisions(&entries);
        assert_eq!(collisions.len(), 1);
        assert!(collisions.contains_key("10.0.0.50"));
        assert_eq!(collisions["10.0.0.50"].len(), 2);
    }

    #[test]
    fn detecta_mac_duplicado_ou_migrado() {
        let entries = vec![
            NeighborEntry {
                ip_address: "10.0.0.108".into(),
                mac_address: "48:8f:5a:aa:bb:cc".into(),
                interface: Some("eth0".into()),
                state: None,
            },
            NeighborEntry {
                ip_address: "10.0.0.119".into(),
                mac_address: "48:8f:5a:aa:bb:cc".into(),
                interface: Some("eth0".into()),
                state: None,
            },
        ];
        let duplicates = find_mac_duplicates(&entries);
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates["48:8f:5a:aa:bb:cc"].len(), 2);
    }
}
