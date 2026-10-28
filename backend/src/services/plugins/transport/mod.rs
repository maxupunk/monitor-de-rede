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

/// Quem recebe cada linha da saída enquanto o comando roda.
pub type LineSink<'a> = &'a (dyn Fn(&str) + Send + Sync);

/// O contrato que o runtime conhece. Uma chamada por vez; quem implementa
/// decide se reaproveita conexão.
#[async_trait]
pub trait DeviceTransport: Send + Sync {
    async fn execute(&self, call: &DeviceIoCall) -> AppResult<DeviceIoReply>;

    /// Igual a [`Self::execute`], entregando a `on_line` cada linha da saída
    /// (`device.ssh_live`). A resposta completa volta no fim, como sempre.
    ///
    /// A implementação padrão só entrega as linhas depois que o comando
    /// termina; quem consegue ler a saída aos pedaços (o SSH local) entrega
    /// à medida que chegam.
    async fn execute_streaming(
        &self,
        call: &DeviceIoCall,
        on_line: LineSink<'_>,
    ) -> AppResult<DeviceIoReply> {
        let reply = self.execute(call).await?;
        feed_lines(&reply, on_line);
        Ok(reply)
    }

    /// De onde a chamada parte, para o transcript ("central", "agente Filial").
    fn origin(&self) -> String;
}

/// Entrega as linhas não vazias do stdout de uma resposta já completa.
pub fn feed_lines(reply: &DeviceIoReply, on_line: LineSink<'_>) {
    if let DeviceIoReply::Exec { stdout, .. } = reply {
        stdout
            .split(['\n', '\r'])
            .filter(|line| !line.trim().is_empty())
            .for_each(on_line);
    }
}

/// Linha sem fim maior que isto é entregue assim mesmo, para o buffer não
/// crescer sem limite.
const MAX_PENDING_LINE: usize = 64 * 1024;

/// Junta pedaços de saída e entrega cada linha completa assim que o fim dela
/// chega. `\r` também encerra linha: equipamento que redesenha o andamento na
/// mesma linha entrega uma linha por quadro. Linha vazia não é entregue.
#[derive(Debug, Default)]
pub struct LineSplitter {
    pending: Vec<u8>,
}

impl LineSplitter {
    pub fn push(&mut self, data: &[u8], on_line: LineSink<'_>) {
        for &byte in data {
            if byte == b'\n' || byte == b'\r' {
                self.flush(on_line);
            } else {
                self.pending.push(byte);
                if self.pending.len() >= MAX_PENDING_LINE {
                    self.flush(on_line);
                }
            }
        }
    }

    /// Entrega o que sobrou (a última linha, sem `\n`).
    pub fn flush(&mut self, on_line: LineSink<'_>) {
        if self.pending.is_empty() {
            return;
        }
        let line = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        if !line.trim().is_empty() {
            on_line(&line);
        }
    }
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
            private_key: Some("-----BEGIN OPENSSH PRIVATE KEY-----chave".into()),
            host_key: Some("SHA256:abc".into()),
        };
        let debug = format!("{login:?}");
        assert!(debug.contains("root"));
        assert!(!debug.contains("s3cr3t"));
        assert!(!debug.contains("chave"));
        assert!(debug.contains("SHA256:abc"), "o pin não é segredo");
    }

    #[test]
    fn chamada_serializa_no_formato_do_protocolo() {
        let call = DeviceIoCall::SshExec {
            endpoint: Endpoint {
                host: "192.168.1.1".into(),
                port: 22,
            },
            login: Login::with_password("root", None),
            command: "uptime".into(),
            timeout_ms: 5_000,
        };
        let json = serde_json::to_value(&call).unwrap();
        assert!(
            json["login"].get("privateKey").is_none(),
            "login por senha não leva campo de chave"
        );
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

    fn collect(chunks: &[&[u8]]) -> Vec<String> {
        let lines = std::sync::Mutex::new(Vec::new());
        let sink = |line: &str| lines.lock().unwrap().push(line.to_owned());
        let mut splitter = LineSplitter::default();
        for chunk in chunks {
            splitter.push(chunk, &sink);
        }
        splitter.flush(&sink);
        lines.into_inner().unwrap()
    }

    #[test]
    fn linhas_partidas_entre_pedacos_sao_remontadas() {
        assert_eq!(
            collect(&[
                b"tx-current: 94",
                b".5Mbps\r\nrx-cur",
                b"rent: 10Mbps\n\n",
                b"fim"
            ]),
            ["tx-current: 94.5Mbps", "rx-current: 10Mbps", "fim"]
        );
    }

    #[test]
    fn resposta_completa_entrega_as_linhas_no_fim() {
        let lines = std::sync::Mutex::new(Vec::new());
        let reply = DeviceIoReply::Exec {
            stdout: "a\n\nb\n".into(),
            stderr: String::new(),
            exit_code: Some(0),
            host_key: None,
        };
        feed_lines(&reply, &|line: &str| {
            lines.lock().unwrap().push(line.to_owned());
        });
        assert_eq!(lines.into_inner().unwrap(), ["a", "b"]);
    }

    #[test]
    fn saida_grande_e_truncada() {
        let grande = vec![b'a'; MAX_OUTPUT_BYTES + 10];
        assert!(bytes_to_text(&grande).ends_with("[… saída truncada]"));
        assert_eq!(bytes_to_text(b"a\r\nb"), "a\nb");
    }
}
