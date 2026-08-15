//! Identifica agentes SNMP autorizados durante discovery. Falhas são esperadas.

use crate::services::{
    discovery::{merger::DiscoveredHost, progress::ScanReporter},
    snmp::service::detect_connection,
};
use futures::{stream, StreamExt};
use tokio_util::sync::CancellationToken;

pub async fn enrich(
    hosts: Vec<DiscoveredHost>,
    cancel: CancellationToken,
    reporter: &ScanReporter,
) -> Vec<DiscoveredHost> {
    let total = hosts.len();
    let mut queried = stream::iter(hosts)
        .map(|mut host| {
            let cancel = cancel.clone();
            async move {
                if cancel.is_cancelled() {
                    return host;
                }
                if let Ok(result) = detect_connection(&host.ip_address, 161, None).await {
                    if result.detected {
                        host.confidence = host.confidence.max(95);
                        if let Some(details) = result.result {
                            if host.vendor.is_none() {
                                host.vendor = details.system.sys_descr;
                            }
                        }
                        host.data["snmp"] = serde_json::json!({
                            "detected": true,
                            "protocol": "udp",
                            "port": 161,
                            "version": result.version,
                        });
                    }
                }
                host
            }
        })
        .buffer_unordered(32);

    let mut enriched = Vec::with_capacity(total);
    while let Some(host) = queried.next().await {
        enriched.push(host);
        reporter.progress("snmp", enriched.len(), total);
    }
    enriched
}
