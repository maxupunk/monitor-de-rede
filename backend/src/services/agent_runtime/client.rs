//! Laço principal do agente: enrollment, conexão, reconexão.
//!
//! Só disca para fora — nenhuma porta é aberta no host (ADR 011). A
//! reconexão usa backoff exponencial com jitter para que dezenas de agentes
//! não voltem todos no mesmo segundo depois de uma queda da central.

use std::{sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use rand::Rng;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest, http::HeaderValue, protocol::Message,
};

use crate::{
    services::{
        docker::source::LocalEngine,
        monitoring::{checkers::ping::PingClient, runner::CheckDeps},
        telemetry::{host_metrics::HostMetricsReader, sampler},
    },
    views::agents::AgentEnrollResponse,
};

use super::{
    config::{read_token, write_token, AgentConfig},
    executor::LocalExecutor,
    identity,
    outbox::Outbox,
    session::{self, DockerLiveSource, OutboxSink, SessionDeps},
};

const TOKEN_HEADER: &str = "x-probe-token";
const BACKOFF_MIN: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(60);
/// Conexão que durou isto é considerada estável e zera o backoff.
const STABLE_AFTER: Duration = Duration::from_secs(60);
const PING_INTERVAL: Duration = Duration::from_secs(20);

/// Próxima espera: dobra até o teto e sorteia entre metade e o total.
#[must_use]
pub fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(BACKOFF_MAX)
}

fn jitter(base: Duration) -> Duration {
    let millis = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
    Duration::from_millis(rand::thread_rng().gen_range(millis / 2..=millis.max(1)))
}

/// Obtém o token: ambiente, arquivo salvo ou troca do código de enrollment.
///
/// # Errors
///
/// Sem token nem código, ou recusa da central.
pub async fn obtain_token(config: &AgentConfig) -> Result<String, String> {
    if let Some(token) = &config.token {
        return Ok(token.clone());
    }
    if let Some(token) = read_token(&config.token_path()) {
        return Ok(token);
    }
    let code = config
        .enroll_code
        .as_deref()
        .ok_or("Sem token salvo: defina AGENT_ENROLL_CODE com o código gerado na central")?;
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?
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
    write_token(&config.token_path(), &enrolled.token)
        .map_err(|error| format!("não foi possível salvar o token: {error}"))?;
    tracing::info!(probe_id = enrolled.probe_id, name = %enrolled.name, "agente registrado na central");
    Ok(enrolled.token)
}

/// Roda o agente para sempre.
pub async fn run_forever(config: AgentConfig) {
    let token = loop {
        match obtain_token(&config).await {
            Ok(token) => break token,
            Err(error) => {
                tracing::error!(%error, "não foi possível obter o token do agente");
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    };

    let outbox = Arc::new(Outbox::open(config.outbox_path(), config.outbox_max));
    let deps = SessionDeps {
        executor: Arc::new(LocalExecutor::new(
            config.policy.clone(),
            CheckDeps {
                ping: PingClient::create()
                    .map_err(|error| tracing::warn!(%error, "ICMP indisponível neste host"))
                    .ok(),
            },
        )),
        outbox: outbox.clone(),
        live: Arc::new(DockerLiveSource),
    };
    sampler::spawn(
        sampler::Sampler::new(HostMetricsReader::from_env(), Arc::new(LocalEngine)),
        Arc::new(OutboxSink(outbox)),
    );

    let mut backoff = BACKOFF_MIN;
    loop {
        let started = tokio::time::Instant::now();
        match connect_once(&config, &token, deps.clone()).await {
            Ok(()) => tracing::warn!("conexão com a central encerrada"),
            Err(error) => tracing::warn!(%error, "falha ao conectar na central"),
        }
        if started.elapsed() >= STABLE_AFTER {
            backoff = BACKOFF_MIN;
        }
        let wait = jitter(backoff);
        tracing::info!(segundos = wait.as_secs_f32(), "reconectando");
        tokio::time::sleep(wait).await;
        backoff = next_backoff(backoff);
    }
}

async fn connect_once(config: &AgentConfig, token: &str, deps: SessionDeps) -> Result<(), String> {
    let mut request = config
        .websocket_url()
        .into_client_request()
        .map_err(|error| error.to_string())?;
    request.headers_mut().insert(
        TOKEN_HEADER,
        HeaderValue::from_str(token).map_err(|error| error.to_string())?,
    );
    let (socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|error| error.to_string())?;
    let (mut sink, mut stream) = socket.split();

    let (incoming_tx, incoming_rx) = mpsc::channel::<String>(64);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<String>(64);
    let reader = tokio::spawn(async move {
        while let Some(Ok(message)) = stream.next().await {
            match message {
                Message::Text(text) => {
                    if incoming_tx.send(text.to_string()).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval(PING_INTERVAL);
        loop {
            tokio::select! {
                text = outgoing_rx.recv() => {
                    let Some(text) = text else { break };
                    if sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                _ = ping.tick() => {
                    if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = sink.close().await;
    });

    let hello = identity::hello(&config.policy).await;
    let result = session::run(deps, hello, incoming_rx, outgoing_tx)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string());
    reader.abort();
    writer.abort();
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
