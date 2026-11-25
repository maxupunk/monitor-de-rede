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

use std::{net::IpAddr, sync::OnceLock, time::Duration};

use futures::future::join_all;
use regex::Regex;
use sea_orm::ConnectionTrait;
use serde::Serialize;
use tokio::{io::AsyncReadExt, net::TcpStream, time::timeout};

use super::{
    compat::{self, DeviceFacts},
    runtime, service,
    transport::{
        local::LocalTransport, DeviceIoCall, DeviceIoReply, DeviceTransport, Endpoint, HttpRequest,
    },
};
use netmonitor_core::{
    devices::systems,
    models::devices,
    network_tools::tcp_probe::{probe_tcp, TcpProbeState},
    shared::errors::{AppError, AppResult},
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
    let transport = LocalTransport::shared("central").ok()?;
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
    static TITLE_RE: OnceLock<Regex> = OnceLock::new();
    let re = TITLE_RE
        .get_or_init(|| Regex::new(r"(?is)<title[^>]*>\s*(.*?)\s*</title>").expect("regex válida"));
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
    facts.set_system(&detection);

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
    let candidate_items = service::for_device(db, device).await?;
    let candidate_ids: Vec<i64> = candidate_items.iter().map(|item| item.plugin.id).collect();
    let models_by_id: std::collections::HashMap<i64, netmonitor_core::models::plugins::Model> =
        if candidate_ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
            netmonitor_core::models::plugins::Entity::find()
                .filter(netmonitor_core::models::plugins::Column::Id.is_in(candidate_ids))
                .all(db)
                .await?
                .into_iter()
                .map(|m| (m.id, m))
                .collect()
        };
    for item in candidate_items {
        let Some(model) = models_by_id.get(&item.plugin.id) else {
            continue;
        };
        let package = service::package_of(model)?;
        let verdict = compat::evaluate(&package.manifest, &package.compatibility, &facts);
        let http_match = package
            .manifest
            .matcher
            .http_fingerprint
            .as_deref()
            .is_some_and(|pattern| {
                !page_text.trim().is_empty()
                    && runtime::cached_regex(&format!("(?i){pattern}"))
                        .is_ok_and(|re| re.is_match(&page_text))
            });
        if verdict.level > compat::Compat::Incompatible || http_match {
            suggested.push(SuggestedPlugin {
                id: model.id,
                slug: model.slug.clone(),
                name: model.name.clone(),
                status: model.status.clone(),
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
