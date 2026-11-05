//! Uma operação de E/S de plugin de dispositivo (`Command::DeviceIo`, ADR 012):
//! alvo, credencial e comando, autocontida para viajar até o agente. O script
//! do plugin nunca viaja — só esta operação unitária.

use std::{fmt, net::IpAddr};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Por onde o plugin fala com o equipamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum TransportKind {
    Ssh,
    Http,
    Telnet,
}

impl TransportKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Http => "http",
            Self::Telnet => "telnet",
        }
    }

    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Ssh => 22,
            Self::Http => 80,
            Self::Telnet => 23,
        }
    }

    /// # Errors
    ///
    /// Tipo fora de `ssh`, `http` e `telnet`.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ssh" => Ok(Self::Ssh),
            "http" => Ok(Self::Http),
            "telnet" => Ok(Self::Telnet),
            other => Err(format!("tipo de acesso desconhecido: {other}")),
        }
    }
}

/// Usuário e senha — ou a chave do acesso dedicado (ADR 016). O `Debug` não
/// mostra segredo: este tipo aparece em log de erro e em `tracing`, e a senha
/// ou a chave não podem ir junto.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Login {
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    /// Chave privada OpenSSH do acesso dedicado. Com ela o SSH autentica por
    /// chave, nunca por senha.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    /// Fingerprint SHA-256 da chave de host fixada no provisionamento: o SSH
    /// recusa equipamento que se apresente com outra.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key: Option<String>,
}

impl Login {
    /// Usuário e senha, sem chave.
    #[must_use]
    pub fn with_password(username: impl Into<String>, password: Option<String>) -> Self {
        Self {
            username: username.into(),
            password,
            ..Self::default()
        }
    }
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "********"))
            .field(
                "private_key",
                &self.private_key.as_ref().map(|_| "********"),
            )
            .field("host_key", &self.host_key)
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
        /// Impressão digital da chave de host que o equipamento apresentou
        /// (só SSH). O provisionamento do acesso dedicado a fixa — inclusive
        /// quando a chamada saiu por um agente remoto.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host_key: Option<String>,
    },
    Http {
        status: u16,
        headers: Vec<(String, String)>,
        body: String,
    },
}
