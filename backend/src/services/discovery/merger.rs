//! Reconcilia as observações heterogêneas por endereço IP.

use super::{device_identifier::identify_device_type, oui_lookup::lookup_vendor};
use serde::{Deserialize, Serialize};
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

#[must_use]
pub fn merge_hosts(lists: impl IntoIterator<Item = Vec<DiscoveredHost>>) -> Vec<DiscoveredHost> {
    let mut by_ip = BTreeMap::<String, DiscoveredHost>::new();
    for host in lists.into_iter().flatten() {
        let current = by_ip
            .entry(host.ip_address.clone())
            .or_insert_with(|| host.clone());
        if host.mac_address.is_some() {
            current.mac_address = host.mac_address.clone();
        }
        if host.hostname.is_some() {
            current.hostname = host.hostname.clone();
        }
        if host.mdns_name.is_some() {
            current.mdns_name = host.mdns_name.clone();
        }
        if host.vendor.is_some() {
            current.vendor = host.vendor.clone();
        }
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
    for host in by_ip.values_mut() {
        if host.vendor.is_none() {
            host.vendor = host
                .mac_address
                .as_deref()
                .and_then(lookup_vendor)
                .map(str::to_string);
        }
        host.device_type = Some(
            identify_device_type(
                host.hostname.as_deref().or(host.mdns_name.as_deref()),
                host.vendor.as_deref(),
                &host.open_ports,
            )
            .into(),
        );
    }
    by_ip.into_values().collect()
}
fn merge_json(base: &mut serde_json::Value, next: serde_json::Value) {
    if let (serde_json::Value::Object(left), serde_json::Value::Object(right)) = (base, next) {
        left.extend(right);
    }
}
