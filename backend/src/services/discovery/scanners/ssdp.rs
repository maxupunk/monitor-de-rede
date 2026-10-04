//! SSDP M-SEARCH sem depender de processo externo.
//!
//! A resposta diz o tipo UPnP (`InternetGatewayDevice`, `MediaRenderer`…) e
//! aponta o XML de descrição, onde o aparelho conta fabricante, modelo e o
//! nome amigável. O XML só é buscado no próprio IP que respondeu.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::{IpAddr, Ipv4Addr},
    sync::LazyLock,
    time::Duration,
};

use futures::{stream, StreamExt};
use regex::Regex;
use reqwest::{Client, Url};
use tokio::net::UdpSocket;

use crate::services::discovery::merger::DiscoveredHost;

const GROUP: &str = "239.255.255.250:1900";
const WINDOW: Duration = Duration::from_secs(3);
const SEARCH_TARGETS: &[&str] = &["ssdp:all", "upnp:rootdevice"];
const MAX_DESCRIPTION: usize = 64 * 1024;
const DESCRIPTION_CONCURRENCY: usize = 16;
const DESCRIPTION_FIELDS: &[&str] = &[
    "friendlyName",
    "manufacturer",
    "modelName",
    "modelDescription",
    "deviceType",
];

static XML_FIELDS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    DESCRIPTION_FIELDS
        .iter()
        .map(|field| {
            (
                *field,
                Regex::new(&format!(r"(?is)<{field}>\s*(.*?)\s*</{field}>"))
                    .expect("regex do campo UPnP"),
            )
        })
        .collect()
});

#[derive(Default)]
struct Seen {
    server: Option<String>,
    location: Option<String>,
    types: BTreeSet<String>,
}

pub async fn scan(client: &Client) -> Vec<DiscoveredHost> {
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return vec![];
    };
    for target in SEARCH_TARGETS {
        let message = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: {GROUP}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {target}\r\n\r\n"
        );
        if socket.send_to(message.as_bytes(), GROUP).await.is_err() {
            return vec![];
        }
    }
    let deadline = tokio::time::Instant::now() + WINDOW;
    let mut seen = BTreeMap::<Ipv4Addr, Seen>::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(Ok((read, source))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
    {
        let IpAddr::V4(ip) = source.ip() else {
            continue;
        };
        let response = String::from_utf8_lossy(&buffer[..read]);
        let entry = seen.entry(ip).or_default();
        entry.server = entry.server.take().or_else(|| header(&response, "server"));
        entry.location = entry
            .location
            .take()
            .or_else(|| header(&response, "location"));
        for name in ["st", "nt", "usn"] {
            if let Some(value) = header(&response, name) {
                entry.types.extend(upnp_types(&value));
            }
        }
    }

    let client = client.clone();
    stream::iter(seen)
        .map(move |(ip, seen)| {
            let client = client.clone();
            async move {
                let description = match &seen.location {
                    Some(location) => describe(&client, ip, location).await,
                    None => BTreeMap::new(),
                };
                into_host(ip, seen, description)
            }
        })
        .buffer_unordered(DESCRIPTION_CONCURRENCY)
        .collect()
        .await
}

fn into_host(ip: Ipv4Addr, seen: Seen, description: BTreeMap<&str, String>) -> DiscoveredHost {
    let mut ssdp = serde_json::json!({
        "server": seen.server,
        "location": seen.location,
        "types": seen.types,
    });
    for (field, value) in description {
        ssdp[field] = serde_json::Value::String(value);
    }
    DiscoveredHost {
        ip_address: ip.to_string(),
        confidence: 60,
        // `server` no topo é o contrato antigo que o Laya ainda lê.
        data: serde_json::json!({
            "scanner": "ssdp",
            "server": seen.server,
            "ssdp": ssdp,
        }),
        ..Default::default()
    }
}

fn header(response: &str, name: &str) -> Option<String> {
    response.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(key, _)| key.trim().eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

/// Os `urn:...:device:<Tipo>:n` e `urn:...:service:<Tipo>:n` de um ST/NT/USN.
fn upnp_types(value: &str) -> Vec<String> {
    value
        .split("::")
        .map(str::trim)
        .filter(|part| part.starts_with("urn:"))
        .map(str::to_string)
        .collect()
}

/// O XML de descrição — só se `location` aponta para o próprio IP.
async fn describe(client: &Client, ip: Ipv4Addr, location: &str) -> BTreeMap<&'static str, String> {
    let Ok(url) = Url::parse(location) else {
        return BTreeMap::new();
    };
    if url.scheme() != "http" || url.host_str() != Some(ip.to_string().as_str()) {
        return BTreeMap::new();
    }
    let Ok(mut response) = client.get(url).send().await else {
        return BTreeMap::new();
    };
    let mut body = Vec::new();
    while body.len() < MAX_DESCRIPTION {
        match response.chunk().await {
            Ok(Some(chunk)) => body.extend_from_slice(&chunk),
            _ => break,
        }
    }
    parse_description(&String::from_utf8_lossy(&body))
}

fn parse_description(xml: &str) -> BTreeMap<&'static str, String> {
    XML_FIELDS
        .iter()
        .filter_map(|(field, pattern)| {
            let value = pattern.captures(xml)?.get(1)?.as_str().trim();
            (!value.is_empty()).then(|| (*field, value.chars().take(160).collect()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrai_tipos_do_usn() {
        assert_eq!(
            upnp_types("uuid:abc::urn:schemas-upnp-org:device:InternetGatewayDevice:1"),
            vec!["urn:schemas-upnp-org:device:InternetGatewayDevice:1".to_string()]
        );
        assert!(upnp_types("upnp:rootdevice").is_empty());
    }

    #[test]
    fn le_a_descricao_do_dispositivo() {
        let xml =
            "<root><device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>\
                   <friendlyName>TV Sala</friendlyName><manufacturer>LG Electronics</manufacturer>\
                   <modelName> OLED55 </modelName></device></root>";
        let fields = parse_description(xml);
        assert_eq!(fields["friendlyName"], "TV Sala");
        assert_eq!(fields["modelName"], "OLED55");
        assert!(fields["deviceType"].contains("MediaRenderer"));
        assert!(!fields.contains_key("modelDescription"));
    }

    #[test]
    fn cabecalho_ignora_caixa_e_valor_vazio() {
        let response = "HTTP/1.1 200 OK\r\nSERVER: Linux UPnP/1.0\r\nLOCATION:\r\n";
        assert_eq!(
            header(response, "server").as_deref(),
            Some("Linux UPnP/1.0")
        );
        assert!(header(response, "location").is_none());
    }
}
