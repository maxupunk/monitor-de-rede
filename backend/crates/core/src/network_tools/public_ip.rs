//! O IP público visto pela internet — o pré-voo da VPN e o assistente
//! inicial perguntam a mesma coisa.

use std::{net::Ipv4Addr, time::Duration};

/// Serviços públicos usados para descobrir o IP visto pela internet.
const PUBLIC_IP_ENDPOINTS: [&str; 2] = [
    "https://api.ipify.org?format=json",
    "https://ifconfig.co/json",
];

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// IP público visto pela internet.
///
/// Dois provedores em sequência: o primeiro que responder decide. Falha de rede
/// devolve `None` — não é erro, é "não deu para saber".
pub async fn detect_public_ip() -> Option<Ipv4Addr> {
    #[derive(serde::Deserialize)]
    struct IpPayload {
        ip: Option<String>,
    }
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()?;

    for endpoint in PUBLIC_IP_ENDPOINTS {
        let Ok(response) = client.get(endpoint).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        if let Ok(payload) = response.json::<IpPayload>().await {
            if let Some(ip) = payload.ip.and_then(|ip| ip.trim().parse::<Ipv4Addr>().ok()) {
                return Some(ip);
            }
        }
    }
    None
}
