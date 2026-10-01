//! Reconhecimento do equipamento antes de escrever (ou escolher) um plugin.
//!
//! Sem credencial e sem alterar nada: portas de gerência abertas, a linha de
//! identificação do SSH e o título/`Server` da página web. Com isso o sistema
//! deduz a plataforma (o mesmo `systems::detect` do cadastro, agora com o
//! banner) e aponta os plugins instalados que casam — inclusive pelo
//! `httpFingerprint` do manifesto, para equipamento que só tem interface web.
//!
//! Parte sempre da central. Equipamento só alcançável por agente remoto
//! aparece com as portas fechadas — o resultado diz de onde partiu.

use std::{net::IpAddr, time::Duration};

use futures::future::join_all;
use regex::Regex;
use sea_orm::ConnectionTrait;
use serde::Serialize;
use tokio::{io::AsyncReadExt, net::TcpStream, time::timeout};

use super::{
    compat::{self, DeviceFacts},
    service,
    transport::{
        local::LocalTransport, DeviceIoCall, DeviceIoReply, DeviceTransport, Endpoint, HttpRequest,
    },
};
use crate::{
    models::devices,
    services::{
        devices::systems,
        network_tools::tcp_probe::{probe_tcp, TcpProbeState},
        shared::errors::{AppError, AppResult},
    },
};

/// Portas de gerência mais comuns: SSH, Telnet, HTTP(S) e as alternativas, e
/// o Winbox da MikroTik.
pub const MANAGEMENT_PORTS: [u16; 7] = [22, 23, 80, 443, 8080, 8443, 8291];
const PROBE_TIMEOUT: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone, Serialize)]
pub struct HttpFingerprint {
    pub port: u16,
    pub https: bool,
    pub status: u16,
    pub server: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestedPlugin {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub status: String,
    pub compat: compat::Compat,
    pub http_fingerprint_match: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fingerprint {
    pub ip: String,
    pub probed_from: &'static str,
    pub open_ports: Vec<u16>,
    pub ssh_banner: Option<String>,
    pub http: Option<HttpFingerprint>,
    pub platform: String,
    pub platform_source: String,
    pub platform_reason: String,
    pub suggested_plugins: Vec<SuggestedPlugin>,
}

async fn ssh_banner(ip: IpAddr) -> Option<String> {
    let mut stream = timeout(PROBE_TIMEOUT, TcpStream::connect((ip, 22)))
        .await
        .ok()?
        .ok()?;
    let mut buffer = [0_u8; 256];
    let read = timeout(Duration::from_secs(3), stream.read(&mut buffer))
        .await
        .ok()?
        .ok()?;
    let line = String::from_utf8_lossy(&buffer[..read]);
    line.lines()
        .next()
        .map(str::trim)
        .filter(|line| line.starts_with("SSH-"))
        .map(str::to_owned)
}

async fn http_page(ip: IpAddr, port: u16, https: bool) -> Option<HttpFingerprint> {
    let transport = LocalTransport::new("central").ok()?;
    let reply = transport
        .execute(&DeviceIoCall::HttpRequest {
            endpoint: Endpoint {
                host: ip.to_string(),
                port,
            },
            https,
            request: HttpRequest {
                method: "GET".into(),
                path: "/".into(),
                headers: vec![],
                body: None,
                basic_auth: None,
            },
            timeout_ms: 5_000,
        })
        .await
        .ok()?;
    let DeviceIoReply::Http {
        status,
        headers,
        body,
    } = reply
    else {
        return None;
    };
    Some(HttpFingerprint {
        port,
        https,
        status,
        server: headers
            .iter()
            .find(|(name, _)| name == "server")
            .map(|(_, value)| value.clone()),
        title: page_title(&body),
    })
}

/// O `<title>` da página, sem espaços sobrando.
#[must_use]
pub fn page_title(body: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<title[^>]*>\s*(.*?)\s*</title>").ok()?;
    re.captures(body)
        .and_then(|captures| captures.get(1))
        .map(|m| m.as_str().split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|title| !title.is_empty())
}

/// # Errors
///
/// Equipamento sem IP válido ou erro do banco.
pub async fn fingerprint<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
) -> AppResult<Fingerprint> {
    let ip: IpAddr = device
        .ip_address
        .as_deref()
        .and_then(|ip| ip.trim().parse().ok())
        .ok_or_else(|| AppError::business_rule("O equipamento não tem IP válido cadastrado."))?;

    let probes = MANAGEMENT_PORTS.map(|port| async move {
        let observation = probe_tcp((ip, port), PROBE_TIMEOUT).await;
        (port, observation.state == TcpProbeState::Open)
    });
    let open_ports: Vec<u16> = join_all(probes)
        .await
        .into_iter()
        .filter_map(|(port, open)| open.then_some(port))
        .collect();

    let banner = if open_ports.contains(&22) {
        ssh_banner(ip).await
    } else {
        None
    };
    let mut http = None;
    for (port, https) in [(80, false), (443, true), (8080, false), (8443, true)] {
        if open_ports.contains(&port) {
            http = http_page(ip, port, https).await;
            if http.is_some() {
                break;
            }
        }
    }

    let detection = systems::detect(&systems::Evidence {
        declared: device.operating_system.as_deref(),
        ssh_banner: banner.as_deref(),
        name: Some(&device.name),
        vendor: device.vendor.as_deref(),
        model: device.model.as_deref(),
        ..systems::Evidence::default()
    });
    let mut facts = DeviceFacts::from_device(device);
    facts.platform = detection.system.id.to_string();
    facts.platform_known = detection.source != systems::source::DEFAULT;

    let page_text = http
        .as_ref()
        .map(|page| {
            format!(
                "{} {}",
                page.title.as_deref().unwrap_or_default(),
                page.server.as_deref().unwrap_or_default()
            )
        })
        .unwrap_or_default();
    let mut suggested = Vec::new();
    for item in service::for_device(db, device).await? {
        let model = service::find(db, item.plugin.id).await?;
        let package = service::package_of(&model)?;
        let verdict = compat::evaluate(&package.manifest, &package.compatibility, &facts);
        let http_match = package
            .manifest
            .matcher
            .http_fingerprint
            .as_deref()
            .is_some_and(|pattern| {
                !page_text.trim().is_empty()
                    && Regex::new(&format!("(?i){pattern}")).is_ok_and(|re| re.is_match(&page_text))
            });
        if verdict.level > compat::Compat::Incompatible || http_match {
            suggested.push(SuggestedPlugin {
                id: model.id,
                slug: model.slug,
                name: model.name,
                status: model.status,
                compat: verdict.level,
                http_fingerprint_match: http_match,
            });
        }
    }

    Ok(Fingerprint {
        ip: ip.to_string(),
        probed_from: "central",
        open_ports,
        ssh_banner: banner,
        http,
        platform: detection.system.id.to_string(),
        platform_source: detection.source.to_string(),
        platform_reason: detection.reason,
        suggested_plugins: suggested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulo_da_pagina_sai_limpo() {
        assert_eq!(
            page_title("<html><TITLE>\n  RT-AX58U  \n</TITLE>").as_deref(),
            Some("RT-AX58U")
        );
        assert_eq!(page_title("<title></title>"), None);
        assert_eq!(page_title("sem título"), None);
    }
}
