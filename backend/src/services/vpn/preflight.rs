//! Teste de pré-voo: descobre o endereço público e diagnostica se roteadores
//! remotos conseguirão iniciar o túnel (§8.10.4).
//!
//! O CGNAT é a única condição realmente impeditiva — e ele é detectável pela
//! faixa 100.64.0.0/10 (RFC 6598).

use std::{net::Ipv4Addr, time::Duration};

use serde::Serialize;
use ts_rs::TS;

/// Serviços públicos usados para descobrir o IP visto pela internet.
const PUBLIC_IP_ENDPOINTS: [&str; 2] = [
    "https://api.ipify.org?format=json",
    "https://ifconfig.co/json",
];

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Faixa reservada ao CGNAT (RFC 6598).
const CGNAT_RANGE: (Ipv4Addr, Ipv4Addr) = (
    Ipv4Addr::new(100, 64, 0, 0),
    Ipv4Addr::new(100, 127, 255, 255),
);

const PRIVATE_RANGES: [(Ipv4Addr, Ipv4Addr); 5] = [
    (Ipv4Addr::new(10, 0, 0, 0), Ipv4Addr::new(10, 255, 255, 255)),
    (
        Ipv4Addr::new(172, 16, 0, 0),
        Ipv4Addr::new(172, 31, 255, 255),
    ),
    (
        Ipv4Addr::new(192, 168, 0, 0),
        Ipv4Addr::new(192, 168, 255, 255),
    ),
    (
        Ipv4Addr::new(169, 254, 0, 0),
        Ipv4Addr::new(169, 254, 255, 255),
    ),
    (
        Ipv4Addr::new(127, 0, 0, 0),
        Ipv4Addr::new(127, 255, 255, 255),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum PreflightStatus {
    Reachable,
    PortForwardRequired,
    Cgnat,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum PreflightLevel {
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PreflightResult {
    pub status: PreflightStatus,
    pub level: PreflightLevel,
    pub message: String,
    pub recommendation: String,
    pub public_ip: Option<String>,
    pub resolved_ip: Option<String>,
    pub port: i32,
    pub is_cgnat: bool,
    pub behind_nat: bool,
    /// Falso quando não houve confirmação externa real da porta UDP.
    pub verified: bool,
}

fn in_range(ip: Ipv4Addr, (start, end): (Ipv4Addr, Ipv4Addr)) -> bool {
    let target = u32::from(ip);
    target >= u32::from(start) && target <= u32::from(end)
}

#[must_use]
pub fn is_cgnat_address(ip: Ipv4Addr) -> bool {
    in_range(ip, CGNAT_RANGE)
}

#[must_use]
pub fn is_private_address(ip: Ipv4Addr) -> bool {
    PRIVATE_RANGES.iter().any(|range| in_range(ip, *range))
}

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

/// Resolve o host do endpoint para IPv4. Já sendo IP, devolve como está.
async fn resolve_host(host: &str) -> Option<Ipv4Addr> {
    if host.trim().is_empty() {
        return None;
    }
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Some(ip);
    }
    // `lookup_host` exige porta; a 0 serve porque só interessa o endereço.
    let target = format!("{}:0", host.trim());
    let resolved = tokio::net::lookup_host(target).await.ok()?;
    resolved.into_iter().find_map(|address| match address.ip() {
        std::net::IpAddr::V4(ip) => Some(ip),
        std::net::IpAddr::V6(_) => None,
    })
}

/// Diagnostica a acessibilidade externa do endpoint configurado.
///
/// **Observação honesta:** sem um verificador externo, o sistema não consegue
/// *provar* que a porta UDP está aberta de fora. O que ele faz é identificar as
/// condições que impedem a conexão (CGNAT) e as que exigem ação do usuário
/// (servidor atrás de NAT sem port-forward). Por isso `verified` é `false` em
/// quase todos os caminhos — inventar um `true` ali seria mentir para quem
/// depende do diagnóstico.
pub async fn run(endpoint_host: Option<&str>, port: i32) -> PreflightResult {
    let public_ip = detect_public_ip().await;
    let resolved_ip = match endpoint_host.filter(|host| !host.trim().is_empty()) {
        Some(host) => resolve_host(host).await,
        None => public_ip,
    };
    let candidate = resolved_ip.or(public_ip);
    let locals = local_addresses();

    let base =
        |status, level, message: String, recommendation: String, is_cgnat, behind_nat, verified| {
            PreflightResult {
                status,
                level,
                message,
                recommendation,
                public_ip: public_ip.map(|ip| ip.to_string()),
                resolved_ip: resolved_ip.map(|ip| ip.to_string()),
                port,
                is_cgnat,
                behind_nat,
                verified,
            }
        };

    let Some(candidate) = candidate else {
        return base(
            PreflightStatus::Unknown,
            PreflightLevel::Warning,
            "Não foi possível determinar o endereço público do servidor.".to_string(),
            "Verifique a conectividade de saída do servidor ou informe o endereço público (ou DDNS) manualmente."
                .to_string(),
            false,
            false,
            false,
        );
    };

    if is_cgnat_address(candidate) {
        return base(
            PreflightStatus::Cgnat,
            PreflightLevel::Error,
            format!("CGNAT detectado (IP {candidate}). Seu provedor não permite conexões de entrada."),
            "Solicite um IP público ao provedor ou hospede um relay WireGuard em uma VPS de baixo custo."
                .to_string(),
            true,
            true,
            // Este é o único `true`: a faixa 100.64/10 é prova por si só.
            true,
        );
    }

    if locals.contains(&candidate) {
        return base(
            PreflightStatus::Reachable,
            PreflightLevel::Success,
            format!("Porta UDP {port} publicada em {candidate}. Roteadores podem conectar."),
            "Nenhuma ação necessária. Gere os scripts dos equipamentos.".to_string(),
            false,
            false,
            false,
        );
    }

    let private_locals: Vec<String> = locals
        .iter()
        .filter(|ip| is_private_address(**ip))
        .map(ToString::to_string)
        .collect();
    let listed = if private_locals.is_empty() {
        "não identificados".to_string()
    } else {
        private_locals.join(", ")
    };

    base(
        PreflightStatus::PortForwardRequired,
        PreflightLevel::Warning,
        format!(
            "O servidor está atrás de NAT (endereço público {candidate}, endereços locais {listed})."
        ),
        format!("Configure no seu roteador o redirecionamento da porta UDP {port} para este servidor."),
        false,
        true,
        false,
    )
}

/// Endereços IPv4 atribuídos às interfaces locais.
///
/// A `std` não enumera interfaces, e trazer uma crate só para isso não se paga:
/// o que interessa é saber se o IP público **é** um endereço desta máquina, e
/// um `UDP connect` para fora revela o endereço de saída sem enviar pacote
/// algum. Cobre o caso real (servidor com IP público direto) sem dependência.
fn local_addresses() -> Vec<Ipv4Addr> {
    let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") else {
        return Vec::new();
    };
    // 203.0.113.1 é TEST-NET-3 (RFC 5737): nunca roteável, e `connect` em UDP
    // só escolhe a rota de saída — não há tráfego.
    if socket.connect("203.0.113.1:9").is_err() {
        return Vec::new();
    }
    match socket.local_addr() {
        Ok(std::net::SocketAddr::V4(address)) => vec![*address.ip()],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> Ipv4Addr {
        text.parse().expect("IPv4 do teste")
    }

    #[test]
    fn reconhece_a_faixa_de_cgnat() {
        assert!(is_cgnat_address(ip("100.64.0.1")));
        assert!(is_cgnat_address(ip("100.127.255.255")));
        // Limites: 100.63 e 100.128 estão fora.
        assert!(!is_cgnat_address(ip("100.63.255.255")));
        assert!(!is_cgnat_address(ip("100.128.0.0")));
        assert!(!is_cgnat_address(ip("203.0.113.1")));
    }

    #[test]
    fn reconhece_as_faixas_privadas() {
        for privado in [
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "127.0.0.1",
        ] {
            assert!(is_private_address(ip(privado)), "{privado}");
        }
        for publico in ["172.32.0.1", "8.8.8.8", "203.0.113.1"] {
            assert!(!is_private_address(ip(publico)), "{publico}");
        }
        // CGNAT não é faixa privada: é justamente o que confunde o diagnóstico.
        assert!(!is_private_address(ip("100.64.0.1")));
    }

    #[tokio::test]
    async fn host_que_ja_e_ip_nao_passa_pelo_dns() {
        assert_eq!(resolve_host("203.0.113.7").await, Some(ip("203.0.113.7")));
        assert_eq!(resolve_host("").await, None);
        assert_eq!(resolve_host("   ").await, None);
    }

    #[tokio::test]
    async fn localhost_resolve_para_ipv4() {
        assert_eq!(resolve_host("localhost").await, Some(ip("127.0.0.1")));
    }

    #[test]
    fn o_status_serializa_no_vocabulario_do_frontend() {
        assert_eq!(
            serde_json::to_value(PreflightStatus::PortForwardRequired).unwrap(),
            serde_json::json!("port_forward_required")
        );
        assert_eq!(
            serde_json::to_value(PreflightLevel::Error).unwrap(),
            serde_json::json!("error")
        );
    }
}
