//! Laço principal do agente: enrollment, conexão, reconexão.
//!
//! Só disca para fora — nenhuma porta é aberta no host (ADR 011). A
//! reconexão usa backoff exponencial com jitter para que dezenas de agentes
//! não voltem todos no mesmo segundo depois de uma queda da central.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};

use crate::services::agents::{
    protocol::{WireFrame, MAX_FRAME_BYTES},
    send_bounded,
};
use futures::{SinkExt, StreamExt};
use rand::Rng;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{HeaderValue, StatusCode},
    protocol::{Message, WebSocketConfig},
    Error as WsError,
};
use tokio_util::sync::CancellationToken;

use netmonitor_core::agents::{command::AgentCommandHandlers, host::HostDocker};

use crate::{
    services::{
        monitoring::runner::CheckDeps, shared::secret_file,
        telemetry::host_metrics::HostMetricsReader,
    },
    views::agents::AgentEnrollResponse,
};

use super::{
    config::AgentConfig,
    executor::LocalExecutor,
    identity,
    outbox::Outbox,
    sampler,
    session::{self, HostLive, OutboxSink, SessionDeps},
    tunnel::LocalTunnelOpener,
};

/// O que as áreas contribuem ao agente, montado pelo binário
/// (`netmonitor-agent`, ponto de montagem do agente): as dependências dos
/// checkers (com os tipos de monitor das áreas), os comandos das áreas e o
/// Docker deste host.
pub struct AgentHost {
    pub deps: CheckDeps,
    pub handlers: AgentCommandHandlers,
    pub docker: Arc<dyn HostDocker>,
}

const TOKEN_HEADER: &str = "x-probe-token";
const BACKOFF_MIN: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(60);
/// Conexão que durou isto é considerada estável e zera o backoff.
const STABLE_AFTER: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Sem qualquer quadro ou `Pong` neste prazo, a conexão é considerada half-open.
const PONG_TIMEOUT_SECS: u64 = 55;
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
/// Escrita no socket presa além disto (buffer cheio, par sumido) é half-open.
const SEND_TIMEOUT: Duration = Duration::from_secs(PONG_TIMEOUT_SECS);
/// Sem token (código ausente, central fora do ar, arquivo ainda não gravado).
const TOKEN_RETRY: Duration = Duration::from_secs(30);

fn http_client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Próxima espera: dobra até o teto e sorteia entre metade e o total.
#[must_use]
pub fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(BACKOFF_MAX)
}

fn jitter(base: Duration) -> Duration {
    let millis = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
    Duration::from_millis(rand::thread_rng().gen_range(millis / 2..=millis.max(1)))
}

async fn read_token_file(path: std::path::PathBuf) -> Option<String> {
    tokio::task::spawn_blocking(move || secret_file::read(&path))
        .await
        .ok()
        .flatten()
}

async fn write_token_file(path: std::path::PathBuf, token: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || secret_file::write(&path, &token))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("não foi possível salvar o token: {error}"))
}

/// Obtém o token: ambiente, arquivo ou troca do código de enrollment.
///
/// Chamada a cada tentativa de conexão: o agente local lê um arquivo que a
/// central regrava quando gera um token novo (ADR 014), e um token recusado
/// não pode prender o agente até reiniciar.
///
/// # Errors
///
/// Sem token nem código, ou recusa da central.
pub async fn obtain_token(config: &AgentConfig) -> Result<String, String> {
    if let Some(token) = &config.token {
        return Ok(token.clone());
    }
    if let Some(token) = read_token_file(config.token_path()).await {
        return Ok(token);
    }
    let code = config.enroll_code.as_deref().ok_or(
        "Sem token: defina AGENT_ENROLL_CODE com o código gerado na central (ou, no \
             agente local, monte o arquivo de AGENT_TOKEN_FILE que a central grava)",
    )?;
    enroll(config, code).await
}

/// Troca o código de enrollment por um token e só então o grava, por cima do
/// anterior: uma recusa (código já usado ou vencido) não toca no token salvo.
async fn enroll(config: &AgentConfig, code: &str) -> Result<String, String> {
    let response = http_client()?
        .post(format!("{}/api/agents/enroll", config.server_url))
        .json(&serde_json::json!({ "code": code, "hostname": std::env::var("HOSTNAME").ok() }))
        .send()
        .await
        .map_err(|error| format!("central inacessível: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "a central recusou o código de enrollment ({}): gere um novo na tela de agentes",
            response.status()
        ));
    }
    let enrolled: AgentEnrollResponse = response.json().await.map_err(|error| error.to_string())?;
    write_token_file(config.token_path(), enrolled.token.clone()).await?;
    tracing::info!(probe_id = enrolled.probe_id, name = %enrolled.name, "agente registrado na central");
    Ok(enrolled.token)
}

/// Espera até ter um token ou o encerramento ser pedido.
async fn wait_for_token(config: &AgentConfig, shutdown: &CancellationToken) -> Option<String> {
    loop {
        if shutdown.is_cancelled() {
            return None;
        }
        match obtain_token(config).await {
            Ok(token) => return Some(token),
            Err(error) => {
                tracing::error!(%error, "não foi possível obter o token do agente");
                tokio::select! {
                    biased;
                    () = shutdown.cancelled() => return None,
                    () = tokio::time::sleep(jitter(TOKEN_RETRY)) => {}
                }
            }
        }
    }
}

/// Roda o agente para sempre.
pub async fn run_forever(config: AgentConfig, host: AgentHost) {
    run_until_shutdown(config, host, CancellationToken::new()).await;
}

/// Roda o agente até `shutdown` ser cancelado (encerramento gracioso via SIGTERM/SIGINT).
pub async fn run_until_shutdown(config: AgentConfig, host: AgentHost, shutdown: CancellationToken) {
    let outbox = Arc::new(Outbox::open(config.outbox_path(), config.outbox_max));
    let deps = SessionDeps {
        executor: Arc::new(LocalExecutor::new(
            config.policy.clone(),
            host.deps,
            host.handlers,
        )),
        outbox: outbox.clone(),
        live: Arc::new(HostLive(host.docker.clone())),
        tunnels: Arc::new(
            LocalTunnelOpener::new(config.policy.clone(), config.database_targets.clone())
                .with_docker(host.docker.clone()),
        ),
    };
    let sampler_handle = if sampler::enabled() {
        Some(sampler::spawn_with_shutdown(
            sampler::Sampler::new(HostMetricsReader::from_env(), host.docker.clone()),
            Arc::new(OutboxSink(outbox)),
            shutdown.clone(),
        ))
    } else {
        tracing::info!("histórico de métricas do host desligado (TELEMETRY_ENABLED=false)");
        None
    };

    let mut backoff = BACKOFF_MIN;
    let mut retried_reenroll = false;
    while !shutdown.is_cancelled() {
        let Some(token) = wait_for_token(&config, &shutdown).await else {
            break;
        };
        let started = tokio::time::Instant::now();
        match connect_once(
            &config,
            &token,
            deps.clone(),
            host.docker.as_ref(),
            &shutdown,
        )
        .await
        {
            Ok(()) => {
                retried_reenroll = false;
                if shutdown.is_cancelled() {
                    tracing::info!("conexão encerrada para desligamento do agente");
                    break;
                }
                tracing::warn!("conexão com a central encerrada");
            }
            Err(ConnectError::Unauthorized) => {
                // Reinstalação em container com volume preservado + novo
                // AGENT_ENROLL_CODE: descarta o token recusado uma vez para tentar
                // o enrollment novo (não toca em AGENT_TOKEN_FILE do agente local).
                if !retried_reenroll
                    && config.token.is_none()
                    && config.token_file.is_none()
                    && config.enroll_code.is_some()
                {
                    retried_reenroll = true;
                    // O código fica no agent.env para sempre e é de uso único:
                    // troca primeiro e só sobrescreve o token se der certo. Um
                    // 401 transitório (túnel da VPN caído, por exemplo) não
                    // pode destruir o token que ainda vale.
                    let code = config.enroll_code.as_deref().unwrap_or_default();
                    match enroll(&config, code).await {
                        Ok(_) => {
                            tracing::warn!(
                                "token salvo recusado (401); substituído pelo enrollment de AGENT_ENROLL_CODE"
                            );
                            continue;
                        }
                        Err(error) => tracing::warn!(
                            %error,
                            "token salvo recusado (401) e AGENT_ENROLL_CODE não vale mais; o token salvo foi mantido"
                        ),
                    }
                }
                tracing::warn!(
                    "a central recusou o token do agente (401): verifique se o agente foi revogado ou reinstale com novo código"
                );
            }
            Err(ConnectError::Other(error)) => {
                tracing::warn!(%error, "falha ao conectar na central");
            }
        }
        if started.elapsed() >= STABLE_AFTER {
            backoff = BACKOFF_MIN;
        }
        let wait = jitter(backoff);
        tracing::info!(segundos = wait.as_secs_f32(), "reconectando");
        tokio::select! {
            biased;
            () = shutdown.cancelled() => break,
            () = tokio::time::sleep(wait) => {}
        }
        backoff = next_backoff(backoff);
    }

    if let Some(handle) = sampler_handle {
        let _ = tokio::time::timeout(CLOSE_TIMEOUT, handle).await;
    }
    // O último minuto fechado pelo amostrador e o que ficou pendente chegam
    // ao disco antes de o processo sair.
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, deps.outbox.flush()).await;
}

enum ConnectError {
    Unauthorized,
    Other(String),
}

async fn connect_once(
    config: &AgentConfig,
    token: &str,
    deps: SessionDeps,
    docker: &dyn HostDocker,
    shutdown: &CancellationToken,
) -> Result<(), ConnectError> {
    let mut request = config
        .websocket_url()
        .into_client_request()
        .map_err(|error| ConnectError::Other(error.to_string()))?;
    request.headers_mut().insert(
        TOKEN_HEADER,
        HeaderValue::from_str(token).map_err(|error| ConnectError::Other(error.to_string()))?,
    );
    let mut ws_config = WebSocketConfig::default();
    ws_config.max_message_size = Some(MAX_FRAME_BYTES);
    ws_config.max_frame_size = Some(MAX_FRAME_BYTES);

    let (socket, _) = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_tungstenite::connect_async_with_config(request, Some(ws_config), true),
    )
    .await
    .map_err(|_| ConnectError::Other("tempo esgotado conectando à central".into()))?
    .map_err(|error| match error {
        WsError::Http(response) if response.status() == StatusCode::UNAUTHORIZED => {
            ConnectError::Unauthorized
        }
        other => ConnectError::Other(other.to_string()),
    })?;
    let (mut sink, mut stream) = socket.split();

    let (incoming_tx, incoming_rx) = mpsc::channel::<WireFrame>(64);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<WireFrame>(64);
    let transport_stop = shutdown.child_token();
    let started_at = tokio::time::Instant::now();
    let last_rx = Arc::new(AtomicU64::new(0));

    let reader = {
        let stop = transport_stop.clone();
        let last_rx = last_rx.clone();
        tokio::spawn(async move {
            loop {
                let next = tokio::select! {
                    biased;
                    () = stop.cancelled() => break,
                    next = stream.next() => next,
                };
                let Some(Ok(message)) = next else {
                    break;
                };
                last_rx.store(started_at.elapsed().as_secs(), Ordering::Relaxed);
                let frame = match message {
                    Message::Text(text) => WireFrame::Text(text.to_string()),
                    Message::Binary(bytes) => WireFrame::Binary(Vec::from(bytes)),
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                };
                if incoming_tx.send(frame).await.is_err() {
                    break;
                }
            }
            stop.cancel();
        })
    };
    let writer = {
        let stop = transport_stop.clone();
        let last_rx = last_rx.clone();
        tokio::spawn(async move {
            let mut ping = tokio::time::interval(PING_INTERVAL);
            ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            ping.tick().await;
            loop {
                tokio::select! {
                    biased;
                    () = stop.cancelled() => break,
                    frame = outgoing_rx.recv() => {
                        let message = match frame {
                            Some(WireFrame::Text(text)) => Message::Text(text.into()),
                            Some(WireFrame::Binary(bytes)) => Message::Binary(bytes.into()),
                            None => break,
                        };
                        if !send_bounded(&mut sink, message, &stop, SEND_TIMEOUT).await {
                            break;
                        }
                    }
                    _ = ping.tick() => {
                        let idle = started_at
                            .elapsed()
                            .as_secs()
                            .saturating_sub(last_rx.load(Ordering::Relaxed));
                        if idle >= PONG_TIMEOUT_SECS {
                            tracing::warn!(
                                segundos = idle,
                                "canal sem resposta da central (half-open); reconectando"
                            );
                            break;
                        }
                        if !send_bounded(&mut sink, Message::Ping(Vec::new().into()), &stop, SEND_TIMEOUT).await {
                            break;
                        }
                    }
                }
            }
            stop.cancel();
            let _ = tokio::time::timeout(CLOSE_TIMEOUT, sink.close()).await;
        })
    };

    let hello = identity::hello(&config.policy, docker).await;
    let result = session::run_with_shutdown(
        deps,
        hello,
        incoming_rx,
        outgoing_tx,
        transport_stop.clone(),
    )
    .await
    .map(|_| ())
    .map_err(|error| ConnectError::Other(error.to_string()));
    transport_stop.cancel();
    reader.abort();
    if tokio::time::timeout(CLOSE_TIMEOUT, writer).await.is_err() {
        tracing::debug!("encerramento do socket do agente excedeu o prazo");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_dobra_ate_o_teto() {
        assert_eq!(next_backoff(Duration::from_secs(1)), Duration::from_secs(2));
        assert_eq!(next_backoff(Duration::from_secs(40)), BACKOFF_MAX);
        let waited = jitter(Duration::from_secs(10));
        assert!(waited >= Duration::from_secs(5) && waited <= Duration::from_secs(10));
    }
}
