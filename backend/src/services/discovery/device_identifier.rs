//! Heurística determinística de classificação descoberta.

use crate::services::devices::adapters::registry;

#[must_use]
pub fn identify_device_type(
    hostname: Option<&str>,
    vendor: Option<&str>,
    open_ports: &[u16],
) -> &'static str {
    identify_device_type_with_context(hostname, vendor, open_ports, None)
}

#[must_use]
pub fn identify_device_type_with_context(
    hostname: Option<&str>,
    vendor: Option<&str>,
    open_ports: &[u16],
    extra_evidence: Option<&str>,
) -> &'static str {
    let context = format!(
        "{} {} {}",
        hostname.unwrap_or_default(),
        vendor.unwrap_or_default(),
        extra_evidence.unwrap_or_default()
    )
    .to_ascii_lowercase();
    // Conhecimento de plataforma pertence aos adapters. A heurística abaixo
    // fica responsável apenas por papéis e protocolos genéricos.
    if let Some(kind) = registry::all()
        .iter()
        .find_map(|adapter| adapter.device_type_hint(&context))
    {
        kind
    } else if contains_any(
        &context,
        &[
            "router",
            "gateway",
            "pfsense",
            "firewall",
            "routerboard",
            "rb9",
            "rb7",
            "rb4",
            "rb3",
            "rb1",
            "ccr",
            "crs",
        ],
    ) {
        "router"
    } else if contains_any(&context, &["switch", "catalyst", "procurve"]) {
        "switch"
    } else if contains_any(&context, &["access point", "wifi", "unifi", "wireless"]) {
        "access_point"
    } else if contains_any(&context, &["printer", "epson", "hp laser", "brother"])
        || open_ports.contains(&9100)
    {
        "printer"
    } else if contains_any(&context, &["camera", "hikvision", "dahua", "onvif"])
        || open_ports.contains(&554)
    {
        "camera"
    } else if contains_any(&context, &["server", "nas", "vmware"])
        || open_ports
            .iter()
            .any(|port| matches!(port, 445 | 1433 | 3306 | 5432))
    {
        "server"
    } else if contains_any(
        &context,
        &[
            "controlador de carga",
            "mppt",
            "nobreak",
            "ups",
            "pdu",
            "sistema embarcado",
            "firmware embarcado",
            "embedded",
        ],
    ) {
        "other"
    } else if open_ports
        .iter()
        .any(|port| matches!(port, 80 | 443 | 8080 | 8000))
    {
        "web_device"
    } else {
        "unknown"
    }
}
fn contains_any(input: &str, terms: &[&str]) -> bool {
    terms.iter().any(|term| input.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plataformas_sao_classificadas_pelos_adapters() {
        assert_eq!(identify_device_type(None, Some("MikroTik"), &[]), "router");
        assert_eq!(
            identify_device_type(Some("OpenWrt AP"), None, &[]),
            "router"
        );
        assert_eq!(
            identify_device_type(Some("UniFi AP"), None, &[]),
            "access_point"
        );
        assert_eq!(
            identify_device_type(None, Some("Microsoft Windows"), &[]),
            "server"
        );
    }

    #[test]
    fn classifica_com_evidencias_extras() {
        assert_eq!(
            identify_device_type_with_context(
                Some("Volt"),
                Some("Volt Tecnologia"),
                &[],
                Some("Controlador de Carga MPPT 12V/24V/48V-30A embedded")
            ),
            "other"
        );
        assert_eq!(
            identify_device_type_with_context(
                Some("HeartOfGold"),
                Some("MikroTik"),
                &[22, 80, 443],
                Some("Linux RB922-terraco 6.12.94 #0 Mon Jun 29 12:59:20 2026 mips")
            ),
            "router"
        );
        assert_eq!(
            identify_device_type_with_context(None, None, &[80, 554], None),
            "camera"
        );
    }
}
