//! O único caminho entre um plugin e o equipamento.
//!
//! [`DeviceIoCall`] é uma operação unitária e autocontida — alvo, credencial e
//! comando — para poder viajar tal como está até um agente remoto
//! (`Command::DeviceIo`). Estado de sessão (cookies HTTP, por exemplo) fica do
//! lado da central, no [`super::runtime`]; o transporte não guarda nada entre
//! chamadas além de conexões reaproveitáveis.
//!
//! Implementações:
//! * [`local::LocalTransport`] — a partir deste processo (central ou agente);
//! * [`agent::AgentTransport`] — encaminha ao agente do site do equipamento;
//! * [`fake::FakeTransport`] — responde com as fixtures do teste unitário.

pub mod agent;
pub mod fake;
pub mod local;

use std::{fmt, net::IpAddr};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::manifest::TransportKind;
use crate::services::shared::errors::AppResult;

/// Teto de bytes lidos de uma resposta. Saída maior que isto é truncada — um
/// `logread` inteiro não pode derrubar o processo nem inflar o transcript.
pub const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// Usuário e senha. O `Debug` não mostra a senha: este tipo aparece em log de
/// erro e em `tracing`, e a senha não pode ir junto.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Login {
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "********"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}

impl Endpoint {
    /// # Errors
    ///
    /// Host que não é um endereço IP — plugin só fala com o IP cadastrado.
    pub fn ip(&self) -> Result<IpAddr, String> {
        self.host
            .parse()
            .map_err(|_| format!("endereço inválido: {}", self.host))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequest {
    pub method: String,
    /// Caminho relativo (`/cgi-bin/luci`). URL absoluta é recusada antes de
    /// chegar aqui: o plugin só alcança o próprio equipamento.
    pub path: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub basic_auth: Option<Login>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DeviceIoCall {
    SshExec {
        endpoint: Endpoint,
        login: Login,
        command: String,
        timeout_ms: u64,
    },
    TelnetExec {
        endpoint: Endpoint,
        login: Login,
        command: String,
        timeout_ms: u64,
    },
    HttpRequest {
        endpoint: Endpoint,
        https: bool,
        request: HttpRequest,
        timeout_ms: u64,
    },
}

impl DeviceIoCall {
    #[must_use]
    pub const fn endpoint(&self) -> &Endpoint {
        match self {
            Self::SshExec { endpoint, .. }
            | Self::TelnetExec { endpoint, .. }
            | Self::HttpRequest { endpoint, .. } => endpoint,
        }
    }

    #[must_use]
    pub const fn transport(&self) -> TransportKind {
        match self {
            Self::SshExec { .. } => TransportKind::Ssh,
            Self::TelnetExec { .. } => TransportKind::Telnet,
            Self::HttpRequest { .. } => TransportKind::Http,
        }
    }

    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        match self {
            Self::SshExec { timeout_ms, .. }
            | Self::TelnetExec { timeout_ms, .. }
            | Self::HttpRequest { timeout_ms, .. } => *timeout_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DeviceIoReply {
    Exec {
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
    },
    Http {
        status: u16,
        headers: Vec<(String, String)>,
        body: String,
    },
}

/// O contrato que o runtime conhece. Uma chamada por vez; quem implementa
/// decide se reaproveita conexão.
#[async_trait]
pub trait DeviceTransport: Send + Sync {
    async fn execute(&self, call: &DeviceIoCall) -> AppResult<DeviceIoReply>;

    /// De onde a chamada parte, para o transcript ("central", "agente Filial").
    fn origin(&self) -> String;
}

/// Converte bytes lidos em texto, com teto. Equipamento manda latin-1 e byte
/// de controle; recusar a resposta inteira por isso perderia o diagnóstico.
#[must_use]
pub fn bytes_to_text(bytes: &[u8]) -> String {
    let slice = &bytes[..bytes.len().min(MAX_OUTPUT_BYTES)];
    let mut text = String::from_utf8_lossy(slice).replace('\r', "");
    if bytes.len() > MAX_OUTPUT_BYTES {
        text.push_str("\n[… saída truncada]");
    }
    text
}

/// Endereço que um plugin (ou agente) pode alcançar: rede privada, CGNAT,
/// link-local e loopback. Um IP público nunca é alvo de plugin — nem por
/// engano de cadastro, nem por um script que tente sair da rede administrada.
#[must_use]
pub fn is_reachable_target(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                // 100.64.0.0/10 (CGNAT) — comum em link de operadora e VPN.
                || (a == 100 && (64..=127).contains(&b))
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback()
                // fc00::/7 (ULA) e fe80::/10 (link-local).
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

/// [`is_reachable_target`] e, quando há lista, dentro de uma das faixas.
#[must_use]
pub fn target_allowed(ip: IpAddr, cidrs: &[ipnet::IpNet]) -> bool {
    is_reachable_target(ip) && (cidrs.is_empty() || cidrs.iter().any(|cidr| cidr.contains(&ip)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_de_faixas_restringe_ainda_mais() {
        let cidrs = vec!["192.168.10.0/24".parse().unwrap()];
        assert!(target_allowed("192.168.10.5".parse().unwrap(), &cidrs));
        assert!(!target_allowed("192.168.11.5".parse().unwrap(), &cidrs));
        assert!(target_allowed("192.168.11.5".parse().unwrap(), &[]));
    }

    #[test]
    fn debug_de_login_nao_mostra_senha() {
        let login = Login {
            username: "root".into(),
            password: Some("s3cr3t".into()),
        };
        let debug = format!("{login:?}");
        assert!(debug.contains("root"));
        assert!(!debug.contains("s3cr3t"));
    }

    #[test]
    fn chamada_serializa_no_formato_do_protocolo() {
        let call = DeviceIoCall::SshExec {
            endpoint: Endpoint {
                host: "192.168.1.1".into(),
                port: 22,
            },
            login: Login {
                username: "root".into(),
                password: None,
            },
            command: "uptime".into(),
            timeout_ms: 5_000,
        };
        let json = serde_json::to_value(&call).unwrap();
        assert_eq!(json["kind"], "ssh_exec");
        assert_eq!(json["timeoutMs"], 5_000);
        assert_eq!(serde_json::from_value::<DeviceIoCall>(json).unwrap(), call);
    }

    #[test]
    fn so_rede_privada_e_alvo() {
        for ip in [
            "192.168.1.1",
            "10.8.0.2",
            "172.16.5.4",
            "100.64.0.1",
            "127.0.0.1",
            "fd00::1",
        ] {
            assert!(is_reachable_target(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "1.1.1.1", "2001:4860::8888"] {
            assert!(!is_reachable_target(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn saida_grande_e_truncada() {
        let grande = vec![b'a'; MAX_OUTPUT_BYTES + 10];
        assert!(bytes_to_text(&grande).ends_with("[… saída truncada]"));
        assert_eq!(bytes_to_text(b"a\r\nb"), "a\nb");
    }
}
