//! Acesso direto a partir deste processo — a central, ou o agente remoto
//! atendendo um `Command::DeviceIo`.
//!
//! SSH usa o canal `exec` (um comando, saída separada de erro, código de
//! saída), reaproveitando a mesma conexão autenticada ao longo de uma
//! execução. Telnet reaproveita o cliente da ativação de syslog. HTTP é um
//! `reqwest` único por execução, sem seguir redirecionamento — o plugin vê o
//! `302` e o `Set-Cookie` do login, que o runtime guarda.

use std::{collections::HashMap, net::IpAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use russh::{client::Handle, ChannelMsg};
use tokio::{sync::Mutex, time::timeout};

use super::{
    bytes_to_text, DeviceIoCall, DeviceIoReply, DeviceTransport, Endpoint, HttpRequest, Login,
    MAX_OUTPUT_BYTES,
};
use crate::services::{
    shared::errors::{AppError, AppResult},
    syslog::provision::{falha_de_acesso, ssh, telnet},
};

type SshSession = Arc<Handle<ssh::AceitaQualquerChave>>;

pub struct LocalTransport {
    origin: String,
    ssh_sessions: Mutex<HashMap<(IpAddr, u16, String), SshSession>>,
    http: reqwest::Client,
}

impl LocalTransport {
    /// # Errors
    ///
    /// Falha ao montar o cliente HTTP (TLS indisponível).
    pub fn new(origin: impl Into<String>) -> AppResult<Self> {
        // Equipamento de rede quase sempre serve HTTPS com certificado
        // autoassinado. Recusar tornaria o recurso inútil; o alvo já é
        // restrito à rede privada e ao IP cadastrado.
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AppError::Internal(anyhow::anyhow!("cliente HTTP: {error}")))?;
        Ok(Self {
            origin: origin.into(),
            ssh_sessions: Mutex::new(HashMap::new()),
            http,
        })
    }

    async fn ssh_session(&self, endpoint: &Endpoint, login: &Login) -> AppResult<SshSession> {
        let ip = endpoint.ip().map_err(AppError::validation)?;
        let key = (ip, endpoint.port, login.username.clone());
        let mut sessions = self.ssh_sessions.lock().await;
        if let Some(session) = sessions.get(&key) {
            if !session.is_closed() {
                return Ok(session.clone());
            }
        }
        let password = login.password.as_deref().unwrap_or_default();
        let session = Arc::new(ssh::conecta(ip, endpoint.port, &login.username, password).await?);
        sessions.insert(key, session.clone());
        Ok(session)
    }

    async fn ssh_exec(
        &self,
        endpoint: &Endpoint,
        login: &Login,
        command: &str,
    ) -> AppResult<DeviceIoReply> {
        let session = self.ssh_session(endpoint, login).await?;
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(falha_de_acesso)?;
        channel
            .exec(true, command.as_bytes())
            .await
            .map_err(falha_de_acesso)?;
        let (mut stdout, mut stderr, mut exit_code) = (Vec::new(), Vec::new(), None);
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => append_capped(&mut stdout, &data),
                ChannelMsg::ExtendedData { data, .. } => append_capped(&mut stderr, &data),
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = i32::try_from(exit_status).ok();
                }
                ChannelMsg::Eof | ChannelMsg::Close if exit_code.is_some() => break,
                _ => {}
            }
        }
        let _ = channel.close().await;
        Ok(DeviceIoReply::Exec {
            stdout: bytes_to_text(&stdout),
            stderr: bytes_to_text(&stderr),
            exit_code,
        })
    }

    async fn telnet_exec(
        endpoint: &Endpoint,
        login: &Login,
        command: &str,
    ) -> AppResult<DeviceIoReply> {
        let ip = endpoint.ip().map_err(AppError::validation)?;
        let password = login.password.as_deref().unwrap_or_default();
        let mut shell = telnet::abre(ip, endpoint.port, &login.username, password).await?;
        // O banner/prompt que sobrou do login não é saída do comando.
        let _ = shell.le_ate_silenciar().await?;
        shell.envia_linha(command).await?;
        let output = shell.le_ate_silenciar().await?;
        shell.encerra().await;
        Ok(DeviceIoReply::Exec {
            stdout: strip_echo(&output, command),
            stderr: String::new(),
            exit_code: None,
        })
    }

    async fn http_request(
        &self,
        endpoint: &Endpoint,
        https: bool,
        request: &HttpRequest,
        timeout_ms: u64,
    ) -> AppResult<DeviceIoReply> {
        let ip = endpoint.ip().map_err(AppError::validation)?;
        let host = match ip {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(v6) => format!("[{v6}]"),
        };
        let scheme = if https { "https" } else { "http" };
        let url = format!("{scheme}://{host}:{}{}", endpoint.port, request.path);
        let method = reqwest::Method::from_bytes(request.method.to_ascii_uppercase().as_bytes())
            .map_err(|_| {
                AppError::validation(format!("método HTTP inválido: {}", request.method))
            })?;
        let mut builder = self
            .http
            .request(method, &url)
            .timeout(Duration::from_millis(timeout_ms));
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(login) = &request.basic_auth {
            builder = builder.basic_auth(&login.username, login.password.as_deref());
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        let response = builder.send().await.map_err(falha_de_acesso)?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_ascii_lowercase(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        let mut body = Vec::new();
        let mut response = response;
        while let Some(chunk) = response.chunk().await.map_err(falha_de_acesso)? {
            append_capped(&mut body, &chunk);
            if body.len() > MAX_OUTPUT_BYTES {
                break;
            }
        }
        Ok(DeviceIoReply::Http {
            status,
            headers,
            body: bytes_to_text(&body),
        })
    }
}

fn append_capped(buffer: &mut Vec<u8>, data: &[u8]) {
    // Um byte além do teto, para `bytes_to_text` saber que truncou.
    let room = (MAX_OUTPUT_BYTES + 1).saturating_sub(buffer.len());
    buffer.extend_from_slice(&data[..data.len().min(room)]);
}

/// O Telnet ecoa o comando digitado na primeira linha da saída.
fn strip_echo(output: &str, command: &str) -> String {
    let mut lines = output.lines();
    match lines.next() {
        Some(first) if first.trim_end().ends_with(command.trim()) => {
            lines.collect::<Vec<_>>().join("\n")
        }
        _ => output.to_owned(),
    }
}

#[async_trait]
impl DeviceTransport for LocalTransport {
    async fn execute(&self, call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
        let limit = Duration::from_millis(call.timeout_ms().clamp(1_000, 300_000));
        let work = async {
            match call {
                DeviceIoCall::SshExec {
                    endpoint,
                    login,
                    command,
                    ..
                } => self.ssh_exec(endpoint, login, command).await,
                DeviceIoCall::TelnetExec {
                    endpoint,
                    login,
                    command,
                    ..
                } => Self::telnet_exec(endpoint, login, command).await,
                DeviceIoCall::HttpRequest {
                    endpoint,
                    https,
                    request,
                    timeout_ms,
                } => {
                    self.http_request(endpoint, *https, request, *timeout_ms)
                        .await
                }
            }
        };
        timeout(limit, work).await.map_err(|_| {
            AppError::business_rule(format!(
                "O equipamento não respondeu em {} s.",
                limit.as_secs()
            ))
        })?
    }

    fn origin(&self) -> String {
        self.origin.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eco_do_telnet_sai_da_saida() {
        assert_eq!(
            strip_echo("root@x:~# uptime\n up 3 days", "uptime"),
            " up 3 days"
        );
        assert_eq!(strip_echo("sem eco", "uptime"), "sem eco");
    }

    #[test]
    fn buffer_respeita_o_teto() {
        let mut buffer = Vec::new();
        append_capped(&mut buffer, &vec![0; MAX_OUTPUT_BYTES * 2]);
        assert_eq!(buffer.len(), MAX_OUTPUT_BYTES + 1);
    }

    /// Chave de host **só de teste**, gerada para este servidor local.
    const TEST_HOST_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACANLwBTsH8/FS/CBfNUNjQsutaUEy8qdQ3hTN7COzElUgAAAJjZj0Da2Y9A
2gAAAAtzc2gtZWQyNTUxOQAAACANLwBTsH8/FS/CBfNUNjQsutaUEy8qdQ3hTN7COzElUg
AAAEDv3xpp8fBhWhAcvTQ+b0Y4gcPVjhRxTQhYBMDhiXU/7g0vAFOwfz8VL8IF81Q2NCy6
1pQTLyp1DeFM3sI7MSVSAAAAD25ldG1vbml0b3ItdGVzdAECAwQFBg==
-----END OPENSSH PRIVATE KEY-----
";

    /// Servidor SSH mínimo: aceita `root`/`s3nh4` e responde a cada `exec`
    /// com `ran: <comando>`; `false` sai com código 1.
    struct FakeSshServer;

    impl russh::server::Handler for FakeSshServer {
        type Error = russh::Error;

        async fn auth_password(
            &mut self,
            user: &str,
            password: &str,
        ) -> Result<russh::server::Auth, Self::Error> {
            Ok(if user == "root" && password == "s3nh4" {
                russh::server::Auth::Accept
            } else {
                russh::server::Auth::reject()
            })
        }

        async fn channel_open_session(
            &mut self,
            _channel: russh::Channel<russh::server::Msg>,
            reply: russh::server::ChannelOpenHandle,
            _session: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }

        async fn exec_request(
            &mut self,
            channel: russh::ChannelId,
            data: &[u8],
            session: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            let command = String::from_utf8_lossy(data).to_string();
            session.channel_success(channel)?;
            session.data(channel, format!("ran: {command}\n").into_bytes())?;
            session.exit_status_request(channel, u32::from(command == "false"))?;
            session.eof(channel)?;
            session.close(channel)?;
            Ok(())
        }
    }

    async fn ssh_server() -> u16 {
        let config = Arc::new(russh::server::Config {
            keys: vec![russh::keys::PrivateKey::from_openssh(TEST_HOST_KEY).unwrap()],
            auth_rejection_time: Duration::from_millis(10),
            auth_rejection_time_initial: Some(Duration::from_millis(0)),
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let config = config.clone();
                tokio::spawn(async move {
                    if let Ok(session) =
                        russh::server::run_stream(config, stream, FakeSshServer).await
                    {
                        let _ = session.await;
                    }
                });
            }
        });
        port
    }

    fn ssh_call(port: u16, password: &str, command: &str) -> DeviceIoCall {
        DeviceIoCall::SshExec {
            endpoint: Endpoint {
                host: "127.0.0.1".into(),
                port,
            },
            login: Login {
                username: "root".into(),
                password: Some(password.into()),
            },
            command: command.into(),
            timeout_ms: 5_000,
        }
    }

    #[tokio::test]
    async fn ssh_exec_reaproveita_a_sessao_e_traz_codigo_de_saida() {
        let port = ssh_server().await;
        let transport = LocalTransport::new("central").unwrap();
        for (command, code) in [("uptime", 0), ("false", 1)] {
            let DeviceIoReply::Exec {
                stdout, exit_code, ..
            } = transport
                .execute(&ssh_call(port, "s3nh4", command))
                .await
                .unwrap()
            else {
                panic!("resposta de exec esperada");
            };
            assert_eq!(stdout, format!("ran: {command}\n"));
            assert_eq!(exit_code, Some(code));
        }
        assert_eq!(
            transport.ssh_sessions.lock().await.len(),
            1,
            "uma sessão só"
        );
    }

    #[tokio::test]
    async fn ssh_com_senha_errada_e_recusado() {
        let port = ssh_server().await;
        let transport = LocalTransport::new("central").unwrap();
        let error = transport
            .execute(&ssh_call(port, "errada", "uptime"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("recusados"), "{error}");
    }

    #[tokio::test]
    async fn http_local_devolve_status_cabecalhos_e_corpo() {
        use axum::{routing::get, Router};
        let app = Router::new().route(
            "/status",
            get(|| async { ([("x-teste", "1")], "tudo certo") }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let transport = LocalTransport::new("central").unwrap();
        let reply = transport
            .execute(&DeviceIoCall::HttpRequest {
                endpoint: Endpoint {
                    host: "127.0.0.1".into(),
                    port,
                },
                https: false,
                request: HttpRequest {
                    method: "GET".into(),
                    path: "/status".into(),
                    headers: vec![],
                    body: None,
                    basic_auth: None,
                },
                timeout_ms: 5_000,
            })
            .await
            .unwrap();
        let DeviceIoReply::Http {
            status,
            headers,
            body,
        } = reply
        else {
            panic!("resposta HTTP esperada");
        };
        assert_eq!(status, 200);
        assert_eq!(body, "tudo certo");
        assert!(headers.iter().any(|(k, v)| k == "x-teste" && v == "1"));
    }
}
