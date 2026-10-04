//! Reconcilia as observações heterogêneas por endereço IP.
//!
//! Cada scanner devolve o que viu de um host (`data.scanner` diz quem viu);
//! aqui elas viram um host só: campos preenchidos por quem os tem, portas
//! somadas, blocos de `data` fundidos em profundidade e `data.sources` com
//! todos que o enxergaram. No fim, o [`classify`] decide o tipo.

use super::{
    device_identifier::classify,
    fingerprints,
    oui_lookup::{is_locally_administered, lookup_vendor},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredHost {
    pub ip_address: String,
    pub mac_address: Option<String>,
    pub hostname: Option<String>,
    pub mdns_name: Option<String>,
    pub vendor: Option<String>,
    pub device_type: Option<String>,
    #[serde(default)]
    pub open_ports: Vec<u16>,
    pub confidence: i32,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Funde e classifica sem saber o gateway da rede.
#[must_use]
pub fn merge_hosts(lists: impl IntoIterator<Item = Vec<DiscoveredHost>>) -> Vec<DiscoveredHost> {
    merge_with_gateway(lists, None)
}

/// Funde as listas e classifica cada host; `gateway` é o gateway cadastrado
/// da rede, que entra como evidência de roteador.
#[must_use]
pub fn merge_with_gateway(
    lists: impl IntoIterator<Item = Vec<DiscoveredHost>>,
    gateway: Option<&str>,
) -> Vec<DiscoveredHost> {
    let mut by_ip = BTreeMap::<String, DiscoveredHost>::new();
    for mut host in lists.into_iter().flatten() {
        record_source(&mut host.data);
        match by_ip.get_mut(&host.ip_address) {
            Some(current) => absorb(current, host),
            None => {
                by_ip.insert(host.ip_address.clone(), host);
            }
        }
    }
    for host in by_ip.values_mut() {
        finalize(host, gateway);
    }
    by_ip.into_values().collect()
}

fn absorb(current: &mut DiscoveredHost, host: DiscoveredHost) {
    fill(&mut current.mac_address, host.mac_address);
    fill(&mut current.hostname, host.hostname);
    fill(&mut current.mdns_name, host.mdns_name);
    fill(&mut current.vendor, host.vendor);
    current.confidence = current.confidence.max(host.confidence);
    let ports: BTreeSet<_> = current
        .open_ports
        .iter()
        .chain(host.open_ports.iter())
        .copied()
        .collect();
    current.open_ports = ports.into_iter().collect();
    merge_json(&mut current.data, host.data);
}

fn fill(target: &mut Option<String>, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        *target = Some(value);
    }
}

/// `data.scanner` (quem viu agora) vira uma entrada de `data.sources`.
fn record_source(data: &mut Value) {
    if !data.is_object() {
        *data = serde_json::json!({});
    }
    let Some(object) = data.as_object_mut() else {
        return;
    };
    let Some(Value::String(scanner)) = object.remove("scanner") else {
        return;
    };
    let sources = object
        .entry("sources")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(items) = sources {
        if !items.iter().any(|item| item.as_str() == Some(&scanner)) {
            items.push(Value::String(scanner));
        }
    }
}

/// Fusão em profundidade: objetos se juntam chave a chave, listas de valores
/// simples viram a união, e o resto é substituído pelo mais recente.
fn merge_json(base: &mut Value, next: Value) {
    match (base, next) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, value) in right {
                match left.get_mut(&key) {
                    Some(existing) => merge_json(existing, value),
                    None => {
                        left.insert(key, value);
                    }
                }
            }
        }
        (Value::Array(left), Value::Array(right))
            if left
                .iter()
                .chain(right.iter())
                .all(|item| !item.is_object() && !item.is_array()) =>
        {
            for item in right {
                if !left.contains(&item) {
                    left.push(item);
                }
            }
        }
        (base, next) => {
            if !next.is_null() {
                *base = next;
            }
        }
    }
}

fn text(data: &Value, path: &[&str]) -> Option<String> {
    path.iter()
        .try_fold(data, |current, key| current.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn finalize(host: &mut DiscoveredHost, gateway: Option<&str>) {
    if host.vendor.is_none() {
        host.vendor = host
            .mac_address
            .as_deref()
            .and_then(lookup_vendor)
            .map(str::to_string)
            .or_else(|| text(&host.data, &["identity", "hardwareVendor"]))
            .or_else(|| text(&host.data, &["ssdp", "manufacturer"]));
    }
    if let Some(object) = host.data.as_object_mut() {
        let services: serde_json::Map<String, Value> = host
            .open_ports
            .iter()
            .filter_map(|port| {
                fingerprints::port_label(*port)
                    .map(|label| (port.to_string(), Value::String(label.into())))
            })
            .collect();
        object.insert("services".into(), Value::Object(services));
        let private = host
            .mac_address
            .as_deref()
            .is_some_and(is_locally_administered);
        object.insert("macPrivate".into(), Value::Bool(private));
    }
    let classification = classify(host, gateway);
    host.device_type = Some(classification.device_type.into());
    host.confidence = classification.confidence;
    host.data["classification"] = serde_json::to_value(&classification).unwrap_or(Value::Null);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn seen(ip: &str, scanner: &str, data: Value) -> DiscoveredHost {
        let mut data = data;
        data["scanner"] = json!(scanner);
        DiscoveredHost {
            ip_address: ip.into(),
            data,
            ..DiscoveredHost::default()
        }
    }

    #[test]
    fn junta_fontes_e_blocos_em_profundidade() {
        let merged = merge_hosts([
            vec![seen("10.0.0.5", "icmp", json!({}))],
            vec![seen(
                "10.0.0.5",
                "mdns",
                json!({ "mdns": { "services": ["_ipp._tcp"] } }),
            )],
            vec![seen(
                "10.0.0.5",
                "mdns",
                json!({ "mdns": { "services": ["_http._tcp"], "model": "EcoTank" } }),
            )],
        ]);
        assert_eq!(merged.len(), 1);
        let data = &merged[0].data;
        assert_eq!(data["sources"], json!(["icmp", "mdns"]));
        assert_eq!(data["mdns"]["services"], json!(["_ipp._tcp", "_http._tcp"]));
        assert_eq!(data["mdns"]["model"], "EcoTank");
        assert_eq!(merged[0].device_type.as_deref(), Some("printer"));
        assert!(data["classification"]["reasons"].is_array());
    }

    #[test]
    fn fabricante_vem_do_mac_ou_do_upnp_e_portas_ganham_nome() {
        let mut arp = seen("10.0.0.6", "arp", json!({}));
        arp.mac_address = Some("b8:27:eb:00:00:01".into());
        let mut ports = seen("10.0.0.6", "tcp", json!({}));
        ports.open_ports = vec![22, 80];
        let upnp = seen(
            "10.0.0.7",
            "ssdp",
            json!({ "ssdp": { "manufacturer": "Sonos, Inc." } }),
        );
        let merged = merge_hosts([vec![arp, ports, upnp]]);
        assert_eq!(merged[0].vendor.as_deref(), Some("Raspberry Pi"));
        assert_eq!(merged[0].open_ports, vec![22, 80]);
        assert_eq!(merged[0].data["services"]["22"], "SSH");
        assert_eq!(merged[1].vendor.as_deref(), Some("Sonos, Inc."));
        assert_eq!(merged[1].device_type.as_deref(), Some("media"));
    }

    #[test]
    fn valor_vazio_nao_apaga_o_que_ja_se_sabia() {
        let mut named = seen("10.0.0.8", "snmp", json!({}));
        named.hostname = Some("core-sw".into());
        let mut blank = seen("10.0.0.8", "dns", json!({}));
        blank.hostname = Some(" ".into());
        let merged = merge_hosts([vec![named, blank]]);
        assert_eq!(merged[0].hostname.as_deref(), Some("core-sw"));
    }
}
