//! Identifica agentes SNMPv2c públicos durante discovery. Falhas são esperadas.

use crate::services::{
    discovery::{merger::DiscoveredHost, progress::ScanReporter},
    snmp::{client::SnmpConfig, service::test_connection},
};
use futures::{stream, StreamExt};

pub async fn enrich(hosts: Vec<DiscoveredHost>, reporter: &ScanReporter) -> Vec<DiscoveredHost> {
    let total = hosts.len();
    let mut queried = stream::iter(hosts)
        .map(|mut host| async move {
            if let Ok(result) =
                test_connection(SnmpConfig::v2c(host.ip_address.clone(), "public", 161)).await
            {
                if result.success {
                    host.confidence = host.confidence.max(95);
                    if host.vendor.is_none() {
                        host.vendor = result.system.sys_descr;
                    }
                    host.data["snmp"] = serde_json::json!(true);
                }
            }
            host
        })
        .buffer_unordered(32);

    let mut enriched = Vec::with_capacity(total);
    while let Some(host) = queried.next().await {
        enriched.push(host);
        reporter.progress("snmp", enriched.len(), total);
    }
    enriched
}
