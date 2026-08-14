//! SSDP M-SEARCH sem depender de processo externo.

use crate::services::discovery::merger::DiscoveredHost;
use std::time::Duration;
use tokio::net::UdpSocket;

pub async fn scan() -> Vec<DiscoveredHost> {
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return vec![];
    };
    let message = b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: ssdp:all\r\n\r\n";
    if socket
        .send_to(message, "239.255.255.250:1900")
        .await
        .is_err()
    {
        return vec![];
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut found = Vec::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(Ok((read, source))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
    {
        let response = String::from_utf8_lossy(&buffer[..read]);
        let server = header(&response, "server");
        let location = header(&response, "location");
        let vendor = server.as_deref().and_then(vendor);
        found.push(DiscoveredHost { ip_address: source.ip().to_string(), vendor: vendor.map(str::to_string), confidence: 60, data: serde_json::json!({ "scanner":"ssdp", "server":server, "location":location, "usn":header(&response,"usn"), "st":header(&response,"st") }), ..Default::default() });
    }
    found
}
fn header(response: &str, name: &str) -> Option<String> {
    response.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().to_string())
    })
}
fn vendor(server: &str) -> Option<&'static str> {
    let server = server.to_ascii_lowercase();
    [
        ("upnp", "UPnP"),
        ("d-link", "D-Link"),
        ("tp-link", "TP-Link"),
        ("sony", "Sony"),
        ("samsung", "Samsung"),
    ]
    .iter()
    .find_map(|(needle, vendor)| server.contains(needle).then_some(*vendor))
}
