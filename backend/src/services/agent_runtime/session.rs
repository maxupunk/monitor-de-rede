//! Uma conexão do agente com a central, vista pelo agente.
//!
//! Como a sessão da central, não conhece WebSocket: fala texto por canais.
//! Isso permite ligar as duas pontas em memória nos testes e é o mesmo
//! código que o cliente real liga ao socket.

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{mpsc, Notify, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    services::{
        agents::{
            policy::Permission,
            protocol::{AgentEvent, Body, Command, Envelope, Hello, Outcome, RemoteError, Welcome},
        },
        docker::{
            realtime::{inventory_snapshot, live_snapshot},
            source::{DockerEngine, LocalEngine},
        },
        monitoring::contracts::CheckResult,
        probes::receiver::ProbeDiscoveryResultPayload,
    },
    views::docker::{DockerInventorySnapshot, DockerLiveSnapshot},
};

use super::{executor::CommandExecutor, outbox::Outbox};

const WELCOME_TIMEOUT: Duration = Duration::from_secs(15);
const OUTBOX_FLUSH: Duration = Duration::from_secs(5);
const MAX_CONCURRENT_REQUESTS: usize = 16;
const INVENTORY_EVERY: u32 = 5;

/// De onde vêm os snapshots ao vivo (o Docker local, nos testes um falso).
#[async_trait]
pub trait LiveSource: Send + Sync {
    async fn live(&self) -> Option<DockerLiveSnapshot>;
    async fn inventory(&self) -> Option<DockerInventorySnapshot>;
}

pub struct DockerLiveSource;

#[async_trait]
impl LiveSource for DockerLiveSource {
    async fn live(&self) -> Option<DockerLiveSnapshot> {
        live_snapshot(&LocalEngine, LocalEngine.metrics())
            .await
            .ok()
    }

    async fn inventory(&self) -> Option<DockerInventorySnapshot> {
        inventory_snapshot(&LocalEngine).await.ok()
    }
}

/// Não publica nada (host sem Docker ou testes).
pub struct NoLiveSource;

#[async_trait]
impl LiveSource for NoLiveSource {
    async fn live(&self) -> Option<DockerLiveSnapshot> {
        None
    }

    async fn inventory(&self) -> Option<DockerInventorySnapshot> {
        None
    }
}

#[derive(Clone)]
pub struct SessionDeps {
    pub executor: Arc<dyn CommandExecutor>,
    pub outbox: Arc<Outbox>,
    pub live: Arc<dyn LiveSource>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SessionError {
    #[error("a central não respondeu ao Hello")]
    NoWelcome,
    #[error("o canal caiu antes da apresentação")]
    Closed,
}

/// Como um resultado se refaz em evento se a resposta não puder sair.
enum Replay {
    Monitor { monitor_id: i64, task_id: String },
    Discovery { run_id: i64, task_id: String },
    None,
}

impl Replay {
    fn of(command: &Command) -> Self {
        match command {
            Command::Monitor { task } => Self::Monitor {
                monitor_id: task.monitor_id,
                task_id: task.id.clone(),
            },
            Command::Discovery { task } => Self::Discovery {
                run_id: task.run_id,
                task_id: task.id.clone(),
            },
            _ => Self::None,
        }
    }

    fn event(self, result: &Result<Value, RemoteError>) -> Option<AgentEvent> {
        match (self, result) {
            (
                Self::Monitor {
                    monitor_id,
                    task_id,
                },
                Ok(value),
            ) => serde_json::from_value::<CheckResult>(value.clone())
                .ok()
                .map(|result| AgentEvent::MonitorResult {
                    monitor_id,
                    task_id: Some(task_id),
                    result: Box::new(result),
                }),
            (Self::Discovery { run_id, task_id }, result) => {
                let (hosts, error) = match result {
                    Ok(value) => (
                        serde_json::from_value(value.clone()).unwrap_or_default(),
                        None,
                    ),
                    Err(error) => (Vec::new(), Some(error.message.clone())),
                };
                Some(AgentEvent::DiscoveryResult {
                    result: Box::new(ProbeDiscoveryResultPayload {
                        run_id,
                        task_id: Some(task_id),
                        hosts,
                        error,
                    }),
                })
            }
            _ => None,
        }
    }
}

/// Apresenta-se, espera o `Welcome` e atende pedidos até o canal cair.
///
/// # Errors
///
/// Falha de apresentação. Depois dela, retorna `Ok` quando o canal cai.
pub async fn run(
    deps: SessionDeps,
    hello: Hello,
    mut incoming: mpsc::Receiver<String>,
    outgoing: mpsc::Sender<String>,
) -> Result<Welcome, SessionError> {
    let (envelopes, mut to_send) = mpsc::channel::<Envelope>(64);
    let writer = tokio::spawn(async move {
        while let Some(envelope) = to_send.recv().await {
            let Ok(text) = envelope.encode() else {
                continue;
            };
            if outgoing.send(text).await.is_err() {
                break;
            }
        }
    });

    if envelopes
        .send(Envelope::new(Body::Hello(hello)))
        .await
        .is_err()
    {
        writer.abort();
        return Err(SessionError::Closed);
    }
    let welcome = match tokio::time::timeout(WELCOME_TIMEOUT, incoming.recv()).await {
        Ok(Some(text)) => match Envelope::decode(&text).map(|envelope| envelope.body) {
            Ok(Body::Welcome(welcome)) => welcome,
            _ => {
                writer.abort();
                return Err(SessionError::NoWelcome);
            }
        },
        Ok(None) => {
            writer.abort();
            return Err(SessionError::Closed);
        }
        Err(_) => {
            writer.abort();
            return Err(SessionError::NoWelcome);
        }
    };
    tracing::info!(probe_id = welcome.probe_id, name = %welcome.name, "conectado à central");

    let refresh = Arc::new(Notify::new());
    let limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut in_flight: HashMap<Uuid, CancellationToken> = HashMap::new();
    let (finished_tx, mut finished) = mpsc::unbounded_channel::<Uuid>();
    let mut live_interval = welcome.live_interval_ms;
    let mut live_ticks: u32 = 0;
    let mut live_timer = live_ticker(live_interval);
    let mut flush_timer = tokio::time::interval(OUTBOX_FLUSH);

    loop {
        tokio::select! {
            frame = incoming.recv() => {
                let Some(text) = frame else { break };
                let Ok(envelope) = Envelope::decode(&text) else {
                    tracing::warn!("quadro inválido da central");
                    continue;
                };
                match envelope.body {
                    Body::Request { command: Command::SetLive { interval_ms }, .. } => {
                        live_interval = interval_ms;
                        live_timer = live_ticker(live_interval);
                        live_ticks = 0;
                        let _ = envelopes
                            .send(Envelope::reply(envelope.id, Body::Response { outcome: Outcome::Ok { value: Value::Null } }))
                            .await;
                    }
                    Body::Request { deadline_ms, command } => {
                        let token = CancellationToken::new();
                        in_flight.insert(envelope.id, token.clone());
                        tokio::spawn(handle_request(
                            deps.clone(),
                            envelopes.clone(),
                            refresh.clone(),
                            limiter.clone(),
                            finished_tx.clone(),
                            envelope.id,
                            Duration::from_millis(deadline_ms),
                            command,
                            token,
                        ));
                    }
                    Body::Cancel => {
                        if let Some(token) = in_flight.remove(&envelope.id) {
                            token.cancel();
                        }
                    }
                    _ => {}
                }
            }
            Some(id) = finished.recv() => {
                in_flight.remove(&id);
            }
            _ = live_timer.tick(), if live_interval > 0 => {
                publish_live(&deps, &envelopes, live_ticks.is_multiple_of(INVENTORY_EVERY)).await;
                live_ticks = live_ticks.wrapping_add(1);
            }
            () = refresh.notified(), if live_interval > 0 => {
                publish_live(&deps, &envelopes, true).await;
            }
            _ = flush_timer.tick() => {
                flush_outbox(&deps.outbox, &envelopes).await;
            }
        }
    }

    for token in in_flight.into_values() {
        token.cancel();
    }
    drop(envelopes);
    writer.abort();
    Ok(welcome)
}

fn live_ticker(interval_ms: u64) -> tokio::time::Interval {
    let period = Duration::from_millis(interval_ms.max(500));
    let mut ticker = tokio::time::interval(period);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ticker
}

async fn publish_live(
    deps: &SessionDeps,
    envelopes: &mpsc::Sender<Envelope>,
    with_inventory: bool,
) {
    if let Some(snapshot) = deps.live.live().await {
        let available = snapshot.status.available;
        let _ = envelopes
            .send(Envelope::new(Body::Event {
                event: AgentEvent::DockerLive {
                    snapshot: Box::new(snapshot),
                },
            }))
            .await;
        if !available || !with_inventory {
            return;
        }
    } else {
        return;
    }
    if let Some(snapshot) = deps.live.inventory().await {
        let _ = envelopes
            .send(Envelope::new(Body::Event {
                event: AgentEvent::DockerInventory {
                    snapshot: Box::new(snapshot),
                },
            }))
            .await;
    }
}

async fn flush_outbox(outbox: &Outbox, envelopes: &mpsc::Sender<Envelope>) {
    let pending = outbox.drain();
    let mut iter = pending.into_iter();
    while let Some(event) = iter.next() {
        if let Err(error) = envelopes.send(Envelope::new(Body::Event { event })).await {
            let Body::Event { event } = error.0.body else {
                break;
            };
            let mut rest = vec![event];
            rest.extend(iter);
            outbox.restore(rest);
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_request(
    deps: SessionDeps,
    envelopes: mpsc::Sender<Envelope>,
    refresh: Arc<Notify>,
    limiter: Arc<Semaphore>,
    finished: mpsc::UnboundedSender<Uuid>,
    id: Uuid,
    deadline: Duration,
    command: Command,
    token: CancellationToken,
) {
    let _permit = limiter.acquire_owned().await;
    let replay = Replay::of(&command);
    let mutates =
        command.permission() != Permission::Read && matches!(command, Command::Docker { .. });

    let (chunks, mut chunk_rx) = mpsc::unbounded_channel::<Value>();
    let forward = {
        let envelopes = envelopes.clone();
        tokio::spawn(async move {
            while let Some(data) = chunk_rx.recv().await {
                if envelopes
                    .send(Envelope::reply(id, Body::Chunk { data }))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        })
    };
    let result = match tokio::time::timeout(
        deadline.max(Duration::from_secs(1)),
        deps.executor.execute(command, chunks, token.clone()),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            token.cancel();
            Err(RemoteError::timeout())
        }
    };
    let _ = forward.await;
    let _ = finished.send(id);
    if token.is_cancelled() && result.is_err() {
        return;
    }
    if mutates && result.is_ok() {
        refresh.notify_one();
    }
    let response = Envelope::reply(
        id,
        Body::Response {
            outcome: result.clone().into(),
        },
    );
    if envelopes.send(response).await.is_err() {
        // O canal caiu com o resultado pronto: guarda para a próxima conexão.
        if let Some(event) = replay.event(&result) {
            deps.outbox.push(event);
        }
    }
}

/// Destino do amostrador no agente: o minuto fechado entra no buffer e sai
/// na próxima descarga (ou na próxima conexão).
pub struct OutboxSink(pub Arc<Outbox>);

#[async_trait]
impl crate::services::telemetry::sampler::RollupSink for OutboxSink {
    async fn accept(&self, rollup: crate::services::telemetry::rollup::MetricsRollup) {
        self.0.push(AgentEvent::MetricsRollup {
            rollup: Box::new(rollup),
        });
    }
}
