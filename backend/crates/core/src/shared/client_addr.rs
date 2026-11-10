//! De onde veio a requisição — um lugar só para o processo inteiro.
//!
//! O IP de origem decide rate limit, auditoria e, no registry (ADR 022), se um
//! token pessoal é aceito (só de rede interna ou VPN). Confiar no primeiro
//! `X-Forwarded-For` que o cliente manda deixaria qualquer um se passar por
//! `10.8.0.5`. Aqui o cabeçalho só vale quando **quem o entregou** é um proxy
//! confiável (`TRUSTED_PROXIES`), e a leitura é da direita para a esquerda: o
//! cliente é o primeiro endereço que não é proxy confiável.
//!
//! A camada [`stamp`], montada uma vez no ponto de montagem, faz a conta com o
//! endereço real da conexão e grava o resultado em cabeçalhos internos — os
//! que o cliente mandar com esse nome são descartados antes. Quem precisa do
//! IP só lê [`client_ip`] / [`origin`].

use std::{
    net::{IpAddr, SocketAddr},
    sync::OnceLock,
};

use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use ipnet::IpNet;

/// O IP do cliente, como esta central o resolveu.
pub const CLIENT_IP_HEADER: &str = "x-netmonitor-client-ip";
/// `public` quando a requisição atravessou a borda pública (túnel, proxy da
/// internet); `internal` quando veio da rede interna ou da VPN.
pub const CLIENT_EDGE_HEADER: &str = "x-netmonitor-client-edge";

/// Variável com as faixas dos proxies confiáveis (`10.0.0.0/8,172.16.0.0/12`).
/// Ausente: loopback e redes privadas (o `cloudflared` e o proxy reverso moram
/// na rede do compose). `none`: ninguém — vale o endereço da conexão.
pub const TRUSTED_PROXIES_ENV: &str = "TRUSTED_PROXIES";

/// De onde a requisição veio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOrigin {
    pub ip: Option<IpAddr>,
    /// Atravessou a borda pública (Cloudflare, ou IP de origem público).
    pub public: bool,
}

impl ClientOrigin {
    /// Veio da rede interna ou da VPN, sem passar pela borda pública.
    #[must_use]
    pub const fn is_internal(&self) -> bool {
        !self.public && self.ip.is_some()
    }
}

/// As faixas em que se confia para dizer quem é o cliente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedProxies(Vec<IpNet>);

impl TrustedProxies {
    /// Loopback e redes privadas (IPv4 e IPv6).
    #[must_use]
    pub fn private_networks() -> Self {
        Self(
            [
                "127.0.0.0/8",
                "10.0.0.0/8",
                "172.16.0.0/12",
                "192.168.0.0/16",
                "100.64.0.0/10",
                "::1/128",
                "fc00::/7",
                "fe80::/10",
            ]
            .iter()
            .filter_map(|net| net.parse().ok())
            .collect(),
        )
    }

    /// Lê `TRUSTED_PROXIES`.
    ///
    /// # Errors
    ///
    /// Faixa inválida — é configuração de segurança: errada, não vira silêncio.
    pub fn parse(raw: Option<&str>) -> Result<Self, String> {
        let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
            return Ok(Self::private_networks());
        };
        if raw.eq_ignore_ascii_case("none") {
            return Ok(Self(Vec::new()));
        }
        raw.split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| {
                item.parse::<IpNet>()
                    .or_else(|_| item.parse::<IpAddr>().map(IpNet::from))
                    .map_err(|_| format!("{TRUSTED_PROXIES_ENV}: faixa inválida '{item}'"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    /// Os do processo (`TRUSTED_PROXIES`, lido uma vez). Valor inválido é
    /// registrado e cai no padrão seguro de ninguém confiável.
    #[must_use]
    pub fn from_env() -> &'static Self {
        static CACHE: OnceLock<TrustedProxies> = OnceLock::new();
        CACHE.get_or_init(|| {
            Self::parse(std::env::var(TRUSTED_PROXIES_ENV).ok().as_deref()).unwrap_or_else(
                |error| {
                    tracing::error!("{error}; nenhum proxy será confiável");
                    Self(Vec::new())
                },
            )
        })
    }

    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        self.0.iter().any(|net| net.contains(&canonical(ip)))
    }
}

/// `::ffff:10.0.0.1` é o `10.0.0.1`.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

/// Endereço de rede interna: loopback, privado, CGNAT/VPN ou link-local.
#[must_use]
pub fn is_internal_ip(ip: IpAddr) -> bool {
    TrustedProxies::private_networks().contains(ip)
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_ip(raw: &str) -> Option<IpAddr> {
    let raw = raw.trim().trim_matches('"');
    raw.parse::<IpAddr>()
        .ok()
        .or_else(|| raw.parse::<SocketAddr>().ok().map(|addr| addr.ip()))
        .map(canonical)
}

/// Resolve a origem a partir do endereço da conexão (`peer`) e dos cabeçalhos.
///
/// Sem `peer` (teste, tarefa fora do servidor), os cabeçalhos são lidos com a
/// mesma regra, como se tivessem vindo de um proxy confiável.
#[must_use]
pub fn resolve(
    peer: Option<IpAddr>,
    headers: &HeaderMap,
    trusted: &TrustedProxies,
) -> ClientOrigin {
    let peer = peer.map(canonical);
    let peer_trusted = peer.is_none_or(|ip| trusted.contains(ip));
    // A Cloudflare marca o que passou por ela; esse tráfego é sempre público.
    let via_cloudflare = peer_trusted
        && (header(headers, "cf-ray").is_some() || header(headers, "cf-connecting-ip").is_some());

    let ip = if peer_trusted {
        header(headers, "cf-connecting-ip")
            .and_then(parse_ip)
            .or_else(|| {
                let chain: Vec<IpAddr> = header(headers, "x-forwarded-for")
                    .map(|raw| raw.split(',').filter_map(parse_ip).collect())
                    .unwrap_or_default();
                chain
                    .iter()
                    .rev()
                    .copied()
                    .find(|ip| !trusted.contains(*ip))
                    .or_else(|| chain.first().copied())
            })
            .or_else(|| header(headers, "x-real-ip").and_then(parse_ip))
            .or(peer)
    } else {
        peer
    };
    ClientOrigin {
        ip,
        public: via_cloudflare || ip.is_some_and(|ip| !is_internal_ip(ip)),
    }
}

/// Camada do ponto de montagem: resolve a origem e a grava nos cabeçalhos
/// internos, descartando os que vieram de fora com o mesmo nome.
pub async fn stamp(mut request: Request, next: Next) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip());
    let headers = request.headers_mut();
    headers.remove(CLIENT_IP_HEADER);
    headers.remove(CLIENT_EDGE_HEADER);
    let origin = resolve(peer, headers, TrustedProxies::from_env());
    if let Some(ip) = origin.ip {
        if let Ok(value) = HeaderValue::from_str(&ip.to_string()) {
            headers.insert(CLIENT_IP_HEADER, value);
        }
    }
    headers.insert(
        CLIENT_EDGE_HEADER,
        HeaderValue::from_static(if origin.public { "public" } else { "internal" }),
    );
    next.run(request).await
}

/// A origem da requisição: a que a camada [`stamp`] gravou ou, sem ela, a
/// resolvida dos cabeçalhos.
#[must_use]
pub fn origin(headers: &HeaderMap) -> ClientOrigin {
    match header(headers, CLIENT_EDGE_HEADER) {
        Some(edge) => ClientOrigin {
            ip: header(headers, CLIENT_IP_HEADER).and_then(parse_ip),
            public: edge != "internal",
        },
        None => resolve(None, headers, TrustedProxies::from_env()),
    }
}

/// O IP do cliente, como texto (`None` quando desconhecido).
#[must_use]
pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    origin(headers).ip.map(|ip| ip.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    fn ip(raw: &str) -> IpAddr {
        raw.parse().unwrap()
    }

    #[test]
    fn cliente_de_fora_nao_se_passa_por_ip_da_vpn() {
        // Conexão direta de um IP público: o cabeçalho dele não vale nada.
        let origin = resolve(
            Some(ip("203.0.113.9")),
            &headers(&[("x-forwarded-for", "10.8.0.5")]),
            &TrustedProxies::private_networks(),
        );
        assert_eq!(origin.ip, Some(ip("203.0.113.9")));
        assert!(origin.public);
        assert!(!origin.is_internal());
    }

    #[test]
    fn pelo_proxy_confiavel_vale_o_primeiro_nao_confiavel_da_direita() {
        // O atacante escreve 10.8.0.5; o proxy acrescenta o IP real.
        let origin = resolve(
            Some(ip("172.18.0.4")),
            &headers(&[("x-forwarded-for", "10.8.0.5, 198.51.100.7")]),
            &TrustedProxies::private_networks(),
        );
        assert_eq!(origin.ip, Some(ip("198.51.100.7")));
        assert!(origin.public);
    }

    #[test]
    fn trafego_da_cloudflare_e_publico_mesmo_com_ip_interno() {
        let origin = resolve(
            Some(ip("172.18.0.4")),
            &headers(&[("cf-connecting-ip", "10.8.0.5"), ("cf-ray", "abc-GRU")]),
            &TrustedProxies::private_networks(),
        );
        assert!(origin.public);
        assert!(!origin.is_internal());
    }

    #[test]
    fn conexao_da_vpn_e_interna() {
        let origin = resolve(
            Some(ip("10.8.0.5")),
            &HeaderMap::new(),
            &TrustedProxies::private_networks(),
        );
        assert_eq!(origin.ip, Some(ip("10.8.0.5")));
        assert!(origin.is_internal());
        let mapped = resolve(
            Some(ip("::ffff:192.168.1.20")),
            &HeaderMap::new(),
            &TrustedProxies::private_networks(),
        );
        assert_eq!(mapped.ip, Some(ip("192.168.1.20")));
        assert!(mapped.is_internal());
    }

    #[test]
    fn sem_proxy_confiavel_vale_so_a_conexao() {
        let none = TrustedProxies::parse(Some("none")).unwrap();
        let origin = resolve(
            Some(ip("10.0.0.2")),
            &headers(&[("x-forwarded-for", "203.0.113.1")]),
            &none,
        );
        assert_eq!(origin.ip, Some(ip("10.0.0.2")));
    }

    #[test]
    fn faixa_invalida_e_erro() {
        assert!(TrustedProxies::parse(Some("10.0.0.0/8, banana")).is_err());
        let list = TrustedProxies::parse(Some("172.18.0.0/16, 10.0.0.1")).unwrap();
        assert!(list.contains(ip("10.0.0.1")));
        assert!(!list.contains(ip("10.0.0.2")));
    }

    #[test]
    fn sem_camada_le_os_cabecalhos_como_antes() {
        let map = headers(&[("x-forwarded-for", "203.0.113.7, 10.0.0.1")]);
        assert_eq!(client_ip(&map).as_deref(), Some("203.0.113.7"));
        assert!(origin(&map).public);
    }

    #[test]
    fn cabecalho_interno_gravado_pela_camada_e_o_que_vale() {
        let map = headers(&[
            (CLIENT_IP_HEADER, "10.8.0.9"),
            (CLIENT_EDGE_HEADER, "internal"),
            ("x-forwarded-for", "203.0.113.7"),
        ]);
        assert_eq!(client_ip(&map).as_deref(), Some("10.8.0.9"));
        assert!(origin(&map).is_internal());
    }
}
