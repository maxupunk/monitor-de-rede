//! Identifica agentes SNMP autorizados durante discovery. Falhas são esperadas.

use std::net::IpAddr;

use crate::services::{
    devices::systems,
    discovery::{fingerprints, merger::DiscoveredHost},
    snmp::{
        client::{SnmpClient, SnmpConfig, SnmpVersion},
        collectors,
        service::detect_connection,
    },
};
use futures::{stream, StreamExt};
use tokio_util::sync::CancellationToken;

/// Agentes consultados ao mesmo tempo. Cada consulta testa as comunidades e
/// versões configuradas em paralelo, então o custo real é este número vezes
/// as combinações.
const CONCURRENCY: usize = 48;

/// Consulta SNMP nos hosts vivos e devolve só os que responderam.
pub async fn enrich(ips: &[IpAddr], cancel: &CancellationToken) -> Vec<DiscoveredHost> {
    let cancel = cancel.clone();
    stream::iter(ips.to_vec())
        .map(move |ip| {
            let cancel = cancel.clone();
            async move {
                if cancel.is_cancelled() {
                    return Vec::new();
                }
                query(ip).await
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect()
}

async fn query(ip: IpAddr) -> Vec<DiscoveredHost> {
    let ip_address = ip.to_string();
    let Ok(result) = detect_connection(&ip_address, 161, None).await else {
        return Vec::new();
    };
    if !result.detected {
        return Vec::new();
    }
    let mut host = DiscoveredHost {
        ip_address: ip_address.clone(),
        confidence: 95,
        data: serde_json::json!({ "scanner": "snmp" }),
        ..Default::default()
    };
    if let Some(details) = result.result {
        let system = &details.system;
        let identity = systems::detect(&systems::Evidence {
            sys_object_id: system.sys_object_id.as_deref(),
            sys_descr: system.sys_descr.as_deref(),
            ..systems::Evidence::default()
        });
        host.hostname = system
            .sys_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        host.vendor = system.hardware_vendor.clone().or_else(|| {
            infer_snmp_vendor(
                system.sys_object_id.as_deref(),
                system.sys_descr.as_deref(),
                system.sys_name.as_deref(),
            )
        });
        host.data["identity"] = serde_json::json!({
            "operatingSystem": identity.system.id,
            "label": identity.system.label,
            "source": identity.source,
            "reason": identity.reason,
            "sysDescr": system.sys_descr,
            "sysObjectId": system.sys_object_id,
            "sysName": system.sys_name,
            "hardwareVendor": system.hardware_vendor,
            "hardwareModel": system.hardware_model,
        });
    }
    host.data["snmp"] = serde_json::json!({
        "detected": true,
        "protocol": "udp",
        "port": 161,
        "version": result.version,
    });

    let version = result
        .version
        .as_deref()
        .and_then(SnmpVersion::parse)
        .unwrap_or(SnmpVersion::V2c);
    let community = result.community.unwrap_or_else(|| "public".to_string());
    let mut config = SnmpConfig::v2c(&ip_address, community, 161);
    config.version = version;
    config.timeout_ms = 1_000;
    let client = SnmpClient::new(config);

    if let Ok(macs) = collectors::collect_interface_macs(&client).await {
        if let Some(mac) = macs.into_iter().next() {
            host.mac_address = Some(mac);
        }
    }

    let mut found = vec![host];

    if let Ok(arp_entries) = collectors::collect_arp_table(&client).await {
        for entry in arp_entries {
            found.push(DiscoveredHost {
                ip_address: entry.ip_address,
                mac_address: Some(entry.mac_address),
                confidence: 80,
                data: serde_json::json!({
                    "scanner": "snmp_arp",
                    "sourceRouter": ip_address,
                }),
                ..Default::default()
            });
        }
    }

    found
}

fn infer_snmp_vendor(
    sys_object_id: Option<&str>,
    sys_descr: Option<&str>,
    sys_name: Option<&str>,
) -> Option<String> {
    let enterprise_vendor = sys_object_id
        .and_then(fingerprints::enterprise_for)
        .map(|entry| entry.vendor);

    // Se o enterprise for específico e não for apenas o daemon Net-SNMP, usa ele
    if let Some(vendor) = enterprise_vendor {
        if vendor != "Net-SNMP" {
            return Some(vendor.to_string());
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
    if context.contains("bpi-r") || context.contains("bananapi") || context.contains("banana pi") {
        return Some("Sinovoip (Banana Pi)".to_string());
    }
    if context.contains("openwrt") {
        return Some("OpenWrt".to_string());
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

    enterprise_vendor.map(ToString::to_string)
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
        // Quando OID é Net-SNMP mas o hardware está identificado na descrição
        assert_eq!(
            infer_snmp_vendor(
                Some("1.3.6.1.4.1.8072"),
                Some("Linux RB922-terraco 6.12.94 #0 Mon Jun 29 12:59:20 2026 mips"),
                Some("HeartOfGold")
            ),
            Some("MikroTik".to_string())
        );
        assert_eq!(
            infer_snmp_vendor(
                Some("1.3.6.1.4.1.8072"),
                Some("Linux bpi-r3-borda 6.12.94 #0 SMP Mon Jun 29 12:59:20 2026 aarch64"),
                Some("HeartOfGold")
            ),
            Some("Sinovoip (Banana Pi)".to_string())
        );
    }
}
