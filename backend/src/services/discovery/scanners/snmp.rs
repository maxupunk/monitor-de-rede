//! Identifica agentes SNMP autorizados durante discovery. Falhas são esperadas.

use crate::services::{
    devices::systems,
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
                            let identity = systems::detect(&systems::Evidence {
                                sys_object_id: details.system.sys_object_id.as_deref(),
                                sys_descr: details.system.sys_descr.as_deref(),
                                ..systems::Evidence::default()
                            });
                            if host.hostname.as_deref().unwrap_or("").trim().is_empty() {
                                if let Some(sys_name) = &details.system.sys_name {
                                    let trimmed = sys_name.trim();
                                    if !trimmed.is_empty() {
                                        host.hostname = Some(trimmed.to_string());
                                    }
                                }
                            }
                            if host.vendor.is_none() {
                                host.vendor =
                                    details.system.hardware_vendor.clone().or_else(|| {
                                        infer_snmp_vendor(
                                            details.system.sys_object_id.as_deref(),
                                            details.system.sys_descr.as_deref(),
                                            details.system.sys_name.as_deref(),
                                        )
                                    });
                            }
                            host.data["identity"] = serde_json::json!({
                                "operatingSystem": identity.system.id,
                                "label": identity.system.label,
                                "source": identity.source,
                                "reason": identity.reason,
                                "sysDescr": details.system.sys_descr,
                                "sysObjectId": details.system.sys_object_id,
                                "sysName": details.system.sys_name,
                                "hardwareVendor": details.system.hardware_vendor,
                                "hardwareModel": details.system.hardware_model,
                            });
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

fn infer_snmp_vendor(
    sys_object_id: Option<&str>,
    sys_descr: Option<&str>,
    sys_name: Option<&str>,
) -> Option<String> {
    if let Some(oid) = sys_object_id {
        let clean = oid.trim().trim_start_matches('.');
        if clean.starts_with("1.3.6.1.4.1.17095") {
            return Some("Volt Tecnologia".to_string());
        }
        if clean.starts_with("1.3.6.1.4.1.14988") {
            return Some("MikroTik".to_string());
        }
        if clean.starts_with("1.3.6.1.4.1.9.") {
            return Some("Cisco".to_string());
        }
        if clean.starts_with("1.3.6.1.4.1.41112.") {
            return Some("Ubiquiti".to_string());
        }
        if clean.starts_with("1.3.6.1.4.1.4881.") {
            return Some("Intelbras".to_string());
        }
        if clean.starts_with("1.3.6.1.4.1.311.") {
            return Some("Microsoft".to_string());
        }
    }

    let context = format!(
        "{} {}",
        sys_descr.unwrap_or_default(),
        sys_name.unwrap_or_default()
    )
    .to_ascii_lowercase();

    if context.contains("mikrotik")
        || context.contains("routeros")
        || context.contains("routerboard")
        || context.contains("rb9")
        || context.contains("rb7")
        || context.contains("rb4")
        || context.contains("rb3")
        || context.contains("rb1")
        || context.contains("ccr")
        || context.contains("crs")
    {
        return Some("MikroTik".to_string());
    }
    if context.contains("ubiquiti") || context.contains("unifi") || context.contains("edgerouter") {
        return Some("Ubiquiti".to_string());
    }
    if context.contains("cisco") {
        return Some("Cisco".to_string());
    }
    if context.contains("intelbras") {
        return Some("Intelbras".to_string());
    }
    if context.contains("volt")
        || context.contains("mppt")
        || context.contains("controlador de carga")
    {
        return Some("Volt Tecnologia".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infere_fabricante_por_oid() {
        assert_eq!(
            infer_snmp_vendor(Some("1.3.6.1.4.1.17095.1"), None, None),
            Some("Volt Tecnologia".to_string())
        );
        assert_eq!(
            infer_snmp_vendor(Some(".1.3.6.1.4.1.14988.1"), None, None),
            Some("MikroTik".to_string())
        );
    }

    #[test]
    fn infere_fabricante_por_descr_e_nome() {
        assert_eq!(
            infer_snmp_vendor(
                None,
                Some("Controlador de Carga MPPT 12V/24V/48V-30A"),
                Some("Volt")
            ),
            Some("Volt Tecnologia".to_string())
        );
        assert_eq!(
            infer_snmp_vendor(
                None,
                Some("Linux RB922-terraco 6.12.94 #0 Mon Jun 29 12:59:20 2026 mips"),
                Some("HeartOfGold")
            ),
            Some("MikroTik".to_string())
        );
    }
}
