//! A página inicial de quem tem porta web aberta.
//!
//! Roteador, câmera, impressora e nobreak têm painel web, e o painel se
//! apresenta: `Server: Hikvision-Webs`, `<title>RouterOS router configuration
//! page</title>`, `WWW-Authenticate: Basic realm="TP-LINK Archer C6"`, um
//! redirecionamento para `/cgi-bin/luci`. É evidência que nenhuma porta dá.
//!
//! Só o próprio IP é visitado: redirecionamento para outro host é anotado,
//! nunca seguido. O cliente é um só por varredura (ver [`client`]).

use std::{collections::BTreeMap, net::IpAddr, sync::LazyLock, time::Duration};

use futures::{stream, StreamExt};
use regex::Regex;
use reqwest::{header, redirect::Policy, Client, Url};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::services::{
    discovery::{fingerprints::WEB_PORTS, merger::DiscoveredHost},
    shared::errors::{AppError, AppResult},
};

/// Hosts visitados ao mesmo tempo.
const CONCURRENCY: usize = 32;
/// O suficiente para chegar ao `<title>` de qualquer painel.
const MAX_BODY: usize = 64 * 1024;
const MAX_TEXT: usize = 160;
/// Redirecionamentos para o mesmo host seguidos até achar a página de login.
const MAX_REDIRECTS: usize = 2;

static TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("regex do título"));
static REALM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)realm="([^"]*)""#).expect("regex do realm"));
static SCRIPT_REDIRECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:location(?:\.href)?\s*=\s*|url=)['"]?([^'";\s>]+)"#)
        .expect("regex do redirecionamento")
});

/// Cliente da descoberta: aceita o certificado autoassinado de todo painel de
/// equipamento e não segue redirecionamento sozinho.
///
/// # Errors
///
/// Falha só se o TLS do processo não inicializa.
pub fn client() -> AppResult<Client> {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(Policy::none())
        .connect_timeout(Duration::from_millis(1_200))
        .timeout(Duration::from_millis(2_500))
        .user_agent("NetMonitor-Discovery/1.0")
        .build()
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpFingerprint {
    pub port: u16,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

impl HttpFingerprint {
    fn says_something(&self) -> bool {
        self.server.is_some() || self.title.is_some() || self.realm.is_some()
    }
}

/// Visita a página de cada host com porta web aberta.
pub async fn fingerprint_all(
    client: &Client,
    open_ports: &BTreeMap<IpAddr, Vec<u16>>,
    cancel: &CancellationToken,
) -> Vec<DiscoveredHost> {
    let jobs: Vec<(IpAddr, Vec<u16>)> = open_ports
        .iter()
        .map(|(ip, ports)| (*ip, ports.clone()))
        .collect();
    let client = client.clone();
    let cancel = cancel.clone();
    stream::iter(jobs)
        .map(move |(ip, ports)| {
            let client = client.clone();
            let cancel = cancel.clone();
            async move {
                if cancel.is_cancelled() {
                    return None;
                }
                let fingerprint = fingerprint(&client, ip, &ports).await?;
                Some(DiscoveredHost {
                    ip_address: ip.to_string(),
                    data: serde_json::json!({ "scanner": "http", "http": fingerprint }),
                    ..Default::default()
                })
            }
        })
        .buffer_unordered(CONCURRENCY)
        .filter_map(std::future::ready)
        .collect()
        .await
}

async fn fingerprint(client: &Client, ip: IpAddr, open: &[u16]) -> Option<HttpFingerprint> {
    let host = match ip {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    let mut fallback = None;
    for (port, scheme) in WEB_PORTS.iter().filter(|(port, _)| open.contains(port)) {
        let Ok(url) = Url::parse(&format!("{scheme}://{host}:{port}/")) else {
            continue;
        };
        if let Some(found) = visit(client, url, *port).await {
            if found.says_something() {
                return Some(found);
            }
            fallback.get_or_insert(found);
        }
    }
    fallback
}

/// Segue até [`MAX_REDIRECTS`] redirecionamentos dentro do mesmo host.
async fn visit(client: &Client, mut url: Url, port: u16) -> Option<HttpFingerprint> {
    let origin = url.host_str()?.to_string();
    let mut found = HttpFingerprint {
        port,
        ..HttpFingerprint::default()
    };
    for _ in 0..=MAX_REDIRECTS {
        let mut response = client.get(url.clone()).send().await.ok()?;
        found.status = response.status().as_u16();
        let headers = response.headers();
        found.server = found
            .server
            .or_else(|| header_text(headers, header::SERVER));
        found.realm = found.realm.or_else(|| {
            header_text(headers, header::WWW_AUTHENTICATE).and_then(|value| capture(&REALM, &value))
        });
        let redirect = header_text(headers, header::LOCATION);

        let mut body = Vec::new();
        while body.len() < MAX_BODY {
            match response.chunk().await {
                Ok(Some(chunk)) => body.extend_from_slice(&chunk),
                _ => break,
            }
        }
        let html = String::from_utf8_lossy(&body);
        found.title = found.title.or_else(|| capture(&TITLE, &html));
        let redirect = redirect.or_else(|| capture(&SCRIPT_REDIRECT, &html));
        let Some(target) = redirect else {
            break;
        };
        found.location = Some(clean(&target));
        match url.join(&target) {
            Ok(next) if next.host_str() == Some(origin.as_str()) && next != url => url = next,
            _ => break,
        }
        if found.title.is_some() {
            break;
        }
    }
    Some(found)
}

fn header_text(headers: &header::HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(clean)
        .filter(|value| !value.is_empty())
}

fn capture(pattern: &Regex, text: &str) -> Option<String> {
    pattern
        .captures(text)
        .and_then(|captures| captures.get(1))
        .map(|value| clean(value.as_str()))
        .filter(|value| !value.is_empty())
}

/// Entidades comuns decodificadas, espaços colapsados e tamanho limitado.
fn clean(text: &str) -> String {
    let decoded = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    let collapsed = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(MAX_TEXT).collect()
}

#[cfg(test)]
mod tests {
    use tokio::{io::AsyncWriteExt, net::TcpListener};

    use super::*;

    /// Um painel falso em 127.0.0.1 que responde sempre o mesmo HTTP.
    async fn panel(response: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0_u8; 1_024];
                let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buffer).await;
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        port
    }

    #[tokio::test]
    async fn le_servidor_titulo_e_realm() {
        let port = panel(
            "HTTP/1.1 401 Unauthorized\r\nServer: Hikvision-Webs\r\nWWW-Authenticate: Basic realm=\"DS-2CD2143\"\r\nConnection: close\r\n\r\n<html><title> Login &amp; Cam</title>",
        )
        .await;
        let url = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let found =
            tokio::time::timeout(Duration::from_secs(5), visit(&client().unwrap(), url, 80))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(found.status, 401);
        assert_eq!(found.server.as_deref(), Some("Hikvision-Webs"));
        assert_eq!(found.realm.as_deref(), Some("DS-2CD2143"));
        assert_eq!(found.title.as_deref(), Some("Login & Cam"));
    }

    #[tokio::test]
    async fn redirecionamento_para_outro_host_nao_e_seguido() {
        let port = panel(
            "HTTP/1.1 302 Found\r\nLocation: http://example.com/login\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .await;
        let url = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let found =
            tokio::time::timeout(Duration::from_secs(5), visit(&client().unwrap(), url, 80))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(found.location.as_deref(), Some("http://example.com/login"));
        assert!(found.title.is_none());
    }

    #[test]
    fn redirecionamento_por_script_e_capturado() {
        assert_eq!(
            capture(
                &SCRIPT_REDIRECT,
                "<script>window.location.href='/cgi-bin/luci';</script>"
            ),
            Some("/cgi-bin/luci".into())
        );
    }
}
