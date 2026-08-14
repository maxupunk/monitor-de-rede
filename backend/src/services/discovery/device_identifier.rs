//! Heurística determinística de classificação descoberta.

#[must_use]
pub fn identify_device_type(
    hostname: Option<&str>,
    vendor: Option<&str>,
    open_ports: &[u16],
) -> &'static str {
    let context = format!(
        "{} {}",
        hostname.unwrap_or_default(),
        vendor.unwrap_or_default()
    )
    .to_ascii_lowercase();
    // A ordem é significativa: roteadores também expõem HTTP/SSH e não devem
    // cair na classificação genérica de servidor.
    if contains_any(
        &context,
        &["router", "gateway", "mikrotik", "pfsense", "firewall"],
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
    } else if contains_any(&context, &["server", "nas", "vmware", "windows"])
        || open_ports
            .iter()
            .any(|port| matches!(port, 445 | 1433 | 3306 | 5432))
    {
        "server"
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
