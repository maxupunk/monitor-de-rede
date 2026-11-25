//! Acesso direto a partir deste processo — a central, ou o agente remoto
//! atendendo um `Command::DeviceIo`.
//!
//! SSH usa o canal `exec` (um comando, saída separada de erro, código de
//! saída), reaproveitando a mesma conexão autenticada ao longo de uma
//! execução. Telnet reaproveita o cliente da ativação de syslog. HTTP é um
//! `reqwest` único por execução, sem seguir redirecionamento — o plugin vê o
//! `302` e o `Set-Cookie` do login, que o runtime guarda.
//!
//! Só o SSH entrega a saída linha a linha de verdade (`execute_streaming`):
//! as mensagens `Data` do canal chegam aos pedaços enquanto o comando roda.
//! Telnet e HTTP entregam as linhas no fim.

use std::{
    collections::HashMap,
    hash::Hash,
    net::IpAddr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use russh::ChannelMsg;
use tokio::{sync::Mutex, time::timeout};

use super::{
    bytes_to_text, feed_lines, DeviceIoCall, DeviceIoReply, DeviceTransport, Endpoint, HttpRequest,
    LineSink, LineSplitter, Login, MAX_OUTPUT_BYTES,
};
use crate::plugins::transport::shell::{falha_de_acesso, ssh, telnet};
use netmonitor_core::shared::errors::{AppError, AppResult};

/// A sessão e a chave de host que o equipamento apresentou nela.
type SshSession = (Arc<ssh::Sessao>, String);

/// Alvo, usuário e o resumo das credenciais (senha, chave e chave de host
/// fixada): a sessão de um não serve ao outro — a rotação do acesso dedicado
/// prova a chave nova com a antiga ainda aberta, e a senha trocada (ou
/// errada) não pode herdar a sessão autenticada com a anterior.
type SessionKey = (IpAddr, u16, String, [u8; 32]);

/// SHA-256 das credenciais do login — nunca a senha ou a chave em si.
fn login_identity(login: &Login) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for part in [&login.password, &login.private_key, &login.host_key] {
        match part {
            Some(value) => {
                hasher.update([1]);
                hasher.update((value.len() as u64).to_le_bytes());
                hasher.update(value.as_bytes());
            }
            None => hasher.update([0]),
        }
    }
    hasher.finalize().into()
}

const MAX_CACHED_SSH_SESSIONS: usize = 128;
/// Sessão parada por mais que isso é fechada: o pool é do processo inteiro
/// (`LocalTransport::shared`) e não deve segurar conexões esquecidas.
const SSH_SESSION_IDLE_TTL: Duration = Duration::from_secs(5 * 60);

/// Uma entrada do pool com o último uso, para a ociosidade e a evicção.
struct Pooled<V> {
    value: V,
    last_used: Instant,
}

/// Tira do pool o que morreu ou ficou ocioso demais e devolve a entrada de
/// `key`, marcando o uso.
fn pool_get<K: Eq + Hash, V: Clone>(
    pool: &mut HashMap<K, Pooled<V>>,
    key: &K,
    now: Instant,
    alive: impl Fn(&V) -> bool,
) -> Option<V> {
    pool.retain(|_, entry| {
        alive(&entry.value) && now.saturating_duration_since(entry.last_used) < SSH_SESSION_IDLE_TTL
    });
    pool.get_mut(key).map(|entry| {
        entry.last_used = now;
        entry.value.clone()
    })
}

/// Guarda `value`, abrindo espaço pela entrada usada há mais tempo.
fn pool_insert<K: Eq + Hash + Clone, V>(
    pool: &mut HashMap<K, Pooled<V>>,
    key: K,
    value: V,
    now: Instant,
    max: usize,
) {
    if !pool.contains_key(&key) && pool.len() >= max {
        if let Some(oldest) = pool
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone())
        {
            pool.remove(&oldest);
        }
    }
    pool.insert(
        key,
        Pooled {
            value,
            last_used: now,
        },
    );
}

pub struct LocalTransport {
    origin: String,
    ssh_sessions: Mutex<HashMap<SessionKey, Pooled<SshSession>>>,
    http: reqwest::Client,
}

impl LocalTransport {
    /// Instância compartilhada por origem (`"central"` ou `"agente"`),
    /// reutilizando o cliente HTTP e o pool de sessões SSH.
    ///
    /// # Errors
    ///
    /// Falha ao montar o cliente HTTP (TLS indisponível).
    pub fn shared(origin: &'static str) -> AppResult<Arc<Self>> {
        static INSTANCES: OnceLock<std::sync::Mutex<HashMap<&'static str, Arc<LocalTransport>>>> =
            OnceLock::new();
        let map = INSTANCES.get_or_init(std::sync::Mutex::default);
        {
            let guard = map
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(existing) = guard.get(origin) {
                return Ok(Arc::clone(existing));
            }
        }
        let created = Arc::new(Self::new(origin)?);
        let mut guard = map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(Arc::clone(guard.entry(origin).or_insert(created)))
    }

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
        let key = (
            ip,
            endpoint.port,
            login.username.clone(),
            login_identity(login),
        );
        let alive = |(handle, _): &SshSession| !handle.is_closed();
        {
            let mut sessions = self.ssh_sessions.lock().await;
            if let Some(session) = pool_get(&mut sessions, &key, Instant::now(), alive) {
                return Ok(session);
            }
        }
        let (session, host_key) = ssh::conecta_vendo_host(ip, endpoint.port, login).await?;
        let session = (Arc::new(session), host_key);
        let mut sessions = self.ssh_sessions.lock().await;
        let now = Instant::now();
        if let Some(existing) = pool_get(&mut sessions, &key, now, alive) {
            return Ok(existing);
        }
        pool_insert(
            &mut sessions,
            key,
            session.clone(),
            now,
            MAX_CACHED_SSH_SESSIONS,
        );
        Ok(session)
    }

    async fn ssh_exec(
        &self,
        endpoint: &Endpoint,
        login: &Login,
        command: &str,
        on_line: Option<LineSink<'_>>,
    ) -> AppResult<DeviceIoReply> {
        let (session, host_key) = self.ssh_session(endpoint, login).await?;
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(falha_de_acesso)?;
        channel
            .exec(true, command.as_bytes())
            .await
            .map_err(falha_de_acesso)?;
        let (mut stdout, mut stderr, mut exit_code) = (Vec::new(), Vec::new(), None);
        let mut lines = LineSplitter::default();
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => {
                    if let Some(on_line) = on_line {
                        lines.push(&data, on_line);
                    }
                    append_capped(&mut stdout, &data);
                }
                ChannelMsg::ExtendedData { data, .. } => append_capped(&mut stderr, &data),
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = i32::try_from(exit_status).ok();
                }
                ChannelMsg::Eof | ChannelMsg::Close if exit_code.is_some() => break,
                _ => {}
            }
        }
        if let Some(on_line) = on_line {
            lines.flush(on_line);
        }
        let _ = channel.close().await;
        Ok(DeviceIoReply::Exec {
            stdout: bytes_to_text(&stdout),
            stderr: bytes_to_text(&stderr),
            exit_code,
            host_key: Some(host_key),
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
            host_key: None,
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

impl LocalTransport {
    /// A chamada, com o prazo dela; `on_line` recebe a saída do SSH aos
    /// pedaços e a dos outros transportes no fim.
    async fn dispatch(
        &self,
        call: &DeviceIoCall,
        on_line: Option<LineSink<'_>>,
    ) -> AppResult<DeviceIoReply> {
        let limit = Duration::from_millis(call.timeout_ms().clamp(1_000, 300_000));
        let work = async {
            match call {
                DeviceIoCall::SshExec {
                    endpoint,
                    login,
                    command,
                    ..
                } => self.ssh_exec(endpoint, login, command, on_line).await,
                DeviceIoCall::TelnetExec {
                    endpoint,
                    login,
                    command,
                    ..
                } => {
                    let reply = Self::telnet_exec(endpoint, login, command).await?;
                    if let Some(on_line) = on_line {
                        feed_lines(&reply, on_line);
                    }
                    Ok(reply)
                }
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
}

#[async_trait]
impl DeviceTransport for LocalTransport {
    async fn execute(&self, call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
        self.dispatch(call, None).await
    }

    async fn execute_streaming(
        &self,
        call: &DeviceIoCall,
        on_line: LineSink<'_>,
    ) -> AppResult<DeviceIoReply> {
        self.dispatch(call, Some(on_line)).await
    }

    fn origin(&self) -> String {
        self.origin.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identidade_do_login_muda_com_a_senha_e_nao_a_contem() {
        let a = Login::with_password("root", Some("s3nh4".into()));
        let b = Login::with_password("root", Some("outra".into()));
        assert_ne!(login_identity(&a), login_identity(&b));
        assert_eq!(login_identity(&a), login_identity(&a.clone()));
        let mut fixado = a.clone();
        fixado.host_key = Some("SHA256:abc".into());
        assert_ne!(login_identity(&a), login_identity(&fixado));
        assert!(!format!("{:?}", login_identity(&a)).contains("s3nh4"));
    }

    #[test]
    fn pool_expira_ocioso_e_despeja_o_menos_usado() {
        let start = Instant::now();
        let mut pool = HashMap::new();
        pool_insert(&mut pool, "a", 1, start, 2);
        pool_insert(&mut pool, "b", 2, start + Duration::from_secs(1), 2);
        // "a" é usado de novo: o menos recente passa a ser "b".
        assert_eq!(
            pool_get(&mut pool, &"a", start + Duration::from_secs(2), |_| true),
            Some(1)
        );
        pool_insert(&mut pool, "c", 3, start + Duration::from_secs(3), 2);
        assert!(pool.contains_key("a") && pool.contains_key("c"));
        assert!(!pool.contains_key("b"));
        // Morta sai na hora; ociosa além do prazo também.
        assert_eq!(
            pool_get(&mut pool, &"c", start + Duration::from_secs(4), |v| *v != 3),
            None
        );
        let later = start + Duration::from_secs(2) + SSH_SESSION_IDLE_TTL;
        assert_eq!(pool_get(&mut pool, &"a", later, |_| true), None);
        assert!(pool.is_empty());
    }

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
            if command == "stream" {
                // A saída chega em pedaços que não respeitam o fim da linha.
                for chunk in ["tx: 1Mb", "ps\ntx: 2Mbps\n", "tx: 3Mbps"] {
                    session.data(channel, chunk.as_bytes().to_vec())?;
                }
            } else {
                session.data(channel, format!("ran: {command}\n").into_bytes())?;
            }
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
            login: Login::with_password("root", Some(password.into())),
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
    async fn ssh_ao_vivo_entrega_linha_a_linha_e_a_saida_inteira_no_fim() {
        let port = ssh_server().await;
        let transport = LocalTransport::new("central").unwrap();
        let lines = std::sync::Mutex::new(Vec::new());
        let sink = |line: &str| lines.lock().unwrap().push(line.to_owned());
        let reply = transport
            .execute_streaming(&ssh_call(port, "s3nh4", "stream"), &sink)
            .await
            .unwrap();
        assert_eq!(
            lines.into_inner().unwrap(),
            ["tx: 1Mbps", "tx: 2Mbps", "tx: 3Mbps"]
        );
        let DeviceIoReply::Exec { stdout, .. } = reply else {
            panic!("resposta de exec esperada");
        };
        assert_eq!(stdout, "tx: 1Mbps\ntx: 2Mbps\ntx: 3Mbps");
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
