//! Reconcilia as observações heterogêneas por endereço IP.
//!
//! Cada scanner devolve o que viu de um host (`data.scanner` diz quem viu);
//! aqui elas viram um host só: campos preenchidos por quem os tem, portas
//! somadas, blocos de `data` fundidos em profundidade e `data.sources` com
//! todos que o enxergaram. No fim, o [`classify`] decide o tipo.

use super::{device_identifier::classify, fingerprints};
use crate::services::vendors::{self, builtin::is_locally_administered};
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
            .and_then(vendors::vendor_name)
            .or_else(|| text(&host.data, &["identity", "hardwareVendor"]))
            .or_else(|| text(&host.data, &["ssdp", "manufacturer"]))
            .or_else(|| infer_vendor_from_evidence(host));
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

fn infer_vendor_from_evidence(host: &DiscoveredHost) -> Option<String> {
    // 1. Portas proprietárias / exclusivas de fabricantes conhecidos
    for port in &host.open_ports {
        match *port {
            8291 | 8728 => return Some("MikroTik".into()),
            34567 => return Some("Xiongmai".into()),
            37777 => return Some("Dahua".into()),
            6053 => return Some("Espressif".into()),
            6668 => return Some("Tuya".into()),
            5001 => return Some("Synology".into()),
            8001 => return Some("Samsung".into()),
            8060 => return Some("Roku".into()),
            1400 => return Some("Sonos".into()),
            _ => {}
        }
    }

    // 2. HTTP Server / Realm / Title
    if let Some(server) = text(&host.data, &["http", "server"]) {
        let s = server.to_ascii_lowercase();
        if s.contains("mikrotik") || s.contains("routeros") {
            return Some("MikroTik".into());
        }
        if s.contains("hikvision") {
            return Some("Hikvision".into());
        }
        if s.contains("dahua") {
            return Some("Dahua".into());
        }
        if s.contains("openwrt") || s.contains("luci") {
            return Some("OpenWrt".into());
        }
        if s.contains("rompager") || s.contains("tp-link") {
            return Some("TP-Link".into());
        }
        if s.contains("intelbras") {
            return Some("Intelbras".into());
        }
        if s.contains("cisco") {
            return Some("Cisco".into());
        }
        if s.contains("ubiquiti") || s.contains("unifi") || s.contains("airos") {
            return Some("Ubiquiti".into());
        }
    }

    if let Some(realm) = text(&host.data, &["http", "realm"]) {
        let r = realm.to_ascii_lowercase();
        if r.contains("mikrotik") || r.contains("routeros") {
            return Some("MikroTik".into());
        }
        if r.contains("tp-link") {
            return Some("TP-Link".into());
        }
        if r.contains("hikvision") {
            return Some("Hikvision".into());
        }
        if r.contains("dahua") {
            return Some("Dahua".into());
        }
        if r.contains("intelbras") {
            return Some("Intelbras".into());
        }
        if r.contains("zte") {
            return Some("ZTE".into());
        }
        if r.contains("huawei") {
            return Some("Huawei".into());
        }
    }

    if let Some(title) = text(&host.data, &["http", "title"]) {
        let t = title.to_ascii_lowercase();
        if t.contains("netsurveillance") {
            return Some("Xiongmai".into());
        }
        if t.contains("mikrotik") || t.contains("routeros") {
            return Some("MikroTik".into());
        }
        if t.contains("hikvision") {
            return Some("Hikvision".into());
        }
        if t.contains("dahua") {
            return Some("Dahua".into());
        }
        if t.contains("tp-link") {
            return Some("TP-Link".into());
        }
        if t.contains("intelbras") {
            return Some("Intelbras".into());
        }
        if t.contains("synology") {
            return Some("Synology".into());
        }
        if t.contains("qnap") {
            return Some("QNAP".into());
        }
    }

    // 3. Hostname / mDNS / NetBIOS
    for name in [
        host.hostname.as_deref(),
        host.mdns_name.as_deref(),
        text(&host.data, &["netbios", "name"]).as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let n = name.to_ascii_lowercase();
        if n.starts_with("mikrotik") {
            return Some("MikroTik".into());
        }
        if n.starts_with("shelly") {
            return Some("Shelly".into());
        }
        if n.starts_with("sonoff") {
            return Some("Sonoff".into());
        }
        if n.starts_with("tasmota") {
            return Some("Tasmota".into());
        }
        if n.starts_with("esphome") {
            return Some("Espressif".into());
        }
        if n.starts_with("tuya") {
            return Some("Tuya".into());
        }
    }

    None
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

    #[test]
    fn infere_fabricante_por_portas_e_evidencias_web() {
        // Porta 8291 (Winbox) infere MikroTik mesmo sem MAC
        let mut router = seen("10.0.0.4", "tcp", json!({}));
        router.open_ports = vec![8291];
        let merged = merge_hosts([vec![router]]);
        assert_eq!(merged[0].vendor.as_deref(), Some("MikroTik"));

        // Porta 34567 (DVR) infere Xiongmai
        let mut dvr = seen("10.0.0.10", "tcp", json!({}));
        dvr.open_ports = vec![34567];
        let merged = merge_hosts([vec![dvr]]);
        assert_eq!(merged[0].vendor.as_deref(), Some("Xiongmai"));

        // Porta 6053 (ESPHome) infere Espressif
        let mut esp = seen("10.0.0.106", "tcp", json!({}));
        esp.open_ports = vec![6053];
        let merged = merge_hosts([vec![esp]]);
        assert_eq!(merged[0].vendor.as_deref(), Some("Espressif"));

        // Porta 6668 infere Tuya
        let mut tuya = seen("10.0.0.107", "tcp", json!({}));
        tuya.open_ports = vec![6668];
        let merged = merge_hosts([vec![tuya]]);
        assert_eq!(merged[0].vendor.as_deref(), Some("Tuya"));

        // Título web com netsurveillance infere Xiongmai
        let web_cam = seen(
            "10.0.0.11",
            "http",
            json!({ "http": { "title": "NETSurveillance WEB" } }),
        );
        let merged = merge_hosts([vec![web_cam]]);
        assert_eq!(merged[0].vendor.as_deref(), Some("Xiongmai"));
    }
}
