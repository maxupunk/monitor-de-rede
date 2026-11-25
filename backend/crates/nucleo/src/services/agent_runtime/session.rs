//! Uma conexão do agente com a central, vista pelo agente.
//!
//! Como a sessão da central, não conhece WebSocket: fala [`WireFrame`]s por
//! canais.
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
            protocol::{
                AgentEvent, Body, Command, DockerCall, Envelope, ErrorCode, Hello, Outcome,
                RemoteError, TunnelFrame, Welcome, WireFrame,
            },
            spawn_priority_writer,
            tunnel::{pump_raw, TunnelStream, INBOUND_CAPACITY},
        },
        monitoring::contracts::CheckResult,
        probes::receiver::ProbeDiscoveryResultPayload,
    },
    views::docker::{DockerInventorySnapshot, DockerLiveSnapshot},
};

use agent_protocol::docker::images::ImageImportRequest;
use netmonitor_core::agents::host::HostDocker;

use super::{executor::CommandExecutor, outbox::Outbox, tunnel::TunnelOpener};

const WELCOME_TIMEOUT: Duration = Duration::from_secs(15);
const OUTBOX_FLUSH: Duration = Duration::from_secs(5);
/// Quantos eventos do buffer offline são enfileirados por ciclo sem bloquear a leitura.
const OUTBOX_BATCH: usize = 32;
/// Pedidos rápidos simultâneos (monitores, listagens, ações Docker, plugins).
const MAX_CONCURRENT_REQUESTS: usize = 16;
/// Operações longas/de stream simultâneas (`FollowLogs`, `ImageBuild`, `StackDeploy`, etc.)
/// em cota separada para nunca matarem de fome monitores nem ações rápidas.
const MAX_CONCURRENT_STREAMS: usize = 8;
const INVENTORY_EVERY: u32 = 5;
/// Pontes simultâneas (banco, ADR 013; volume, ADR 014): o resto dos pedidos
/// não pode ficar sem vaga por causa de backups e downloads.
const MAX_TUNNELS: usize = 2;
/// Quadros de ponte na fila de saída; o controle passa na frente.
const TUNNEL_BUFFER: usize = 16;
/// Folga para a tarefa de encaminhamento de chunks drenar após cancelamento/timeout.
const CHUNK_FORWARD_GRACE: Duration = Duration::from_millis(250);

/// De onde vêm os snapshots ao vivo (o Docker local, nos testes um falso).
#[async_trait]
pub trait LiveSource: Send + Sync {
    async fn live(&self) -> Option<DockerLiveSnapshot>;
    async fn inventory(&self) -> Option<DockerInventorySnapshot>;
}

/// O Docker deste host (Servidores, montado pelo binário do agente).
pub struct HostLive(pub Arc<dyn HostDocker>);

#[async_trait]
impl LiveSource for HostLive {
    async fn live(&self) -> Option<DockerLiveSnapshot> {
        self.0.live().await
    }

    async fn inventory(&self) -> Option<DockerInventorySnapshot> {
        self.0.inventory().await
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
    pub tunnels: Arc<dyn TunnelOpener>,
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

    fn event(self, result: Result<Value, RemoteError>) -> Option<AgentEvent> {
        match (self, result) {
            (
                Self::Monitor {
                    monitor_id,
                    task_id,
                },
                Ok(value),
            ) => serde_json::from_value::<CheckResult>(value)
                .ok()
                .map(|result| AgentEvent::MonitorResult {
                    monitor_id,
                    task_id: Some(task_id),
                    result: Box::new(result),
                }),
            (Self::Discovery { run_id, task_id }, result) => {
                let (hosts, error) = match result {
                    Ok(value) => (serde_json::from_value(value).unwrap_or_default(), None),
                    Err(error) => (Vec::new(), Some(error.message)),
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

const fn is_long_running(command: &Command) -> bool {
    matches!(
        command,
        Command::Docker {
            call: DockerCall::FollowLogs { .. }
                | DockerCall::ImageBuild { .. }
                | DockerCall::StackDeploy { .. }
                | DockerCall::PullImage { .. }
                | DockerCall::Compose { .. }
                | DockerCall::UpdateContainer { .. }
        } | Command::Throughput { .. }
            | Command::Discovery { .. }
    )
}

const fn is_droppable_stream(command: &Command) -> bool {
    matches!(
        command,
        Command::Docker {
            call: DockerCall::FollowLogs { .. }
        }
    )
}

async fn handshake(
    hello: Hello,
    envelopes: &mpsc::Sender<Envelope>,
    incoming: &mut mpsc::Receiver<WireFrame>,
    stop: &CancellationToken,
) -> Result<Welcome, SessionError> {
    if envelopes
        .send(Envelope::new(Body::Hello(hello)))
        .await
        .is_err()
    {
        return Err(SessionError::Closed);
    }
    let frame = tokio::select! {
        biased;
        () = stop.cancelled() => return Err(SessionError::Closed),
        res = tokio::time::timeout(WELCOME_TIMEOUT, incoming.recv()) => res,
    };
    match frame {
        Ok(Some(WireFrame::Text(text))) => {
            match Envelope::decode(&text).map(|envelope| envelope.body) {
                Ok(Body::Welcome(welcome)) => Ok(welcome),
                _ => Err(SessionError::NoWelcome),
            }
        }
        Ok(Some(WireFrame::Binary(_))) | Err(_) => Err(SessionError::NoWelcome),
        Ok(None) => Err(SessionError::Closed),
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
    incoming: mpsc::Receiver<WireFrame>,
    outgoing: mpsc::Sender<WireFrame>,
) -> Result<Welcome, SessionError> {
    run_with_shutdown(deps, hello, incoming, outgoing, CancellationToken::new()).await
}

/// Como [`run`], mas encerra o laço quando `shutdown` é cancelado.
///
/// # Errors
///
/// Falha de apresentação. Depois dela, retorna `Ok` quando o canal cai ou encerra.
pub async fn run_with_shutdown(
    deps: SessionDeps,
    hello: Hello,
    mut incoming: mpsc::Receiver<WireFrame>,
    outgoing: mpsc::Sender<WireFrame>,
    shutdown: CancellationToken,
) -> Result<Welcome, SessionError> {
    let (envelopes, to_send) = mpsc::channel::<Envelope>(64);
    let (tunnel_out, tunnel_frames) = mpsc::channel::<WireFrame>(TUNNEL_BUFFER);
    let loop_stop = shutdown.child_token();
    let writer = spawn_priority_writer(to_send, tunnel_frames, outgoing, loop_stop.clone());

    let welcome = match handshake(hello, &envelopes, &mut incoming, &loop_stop).await {
        Ok(welcome) => welcome,
        Err(error) => {
            writer.abort();
            return Err(error);
        }
    };
    tracing::info!(probe_id = welcome.probe_id, name = %welcome.name, "conectado à central");

    let refresh = Arc::new(Notify::new());
    let mut live = LivePublisher::new(deps.clone(), envelopes.clone());
    let limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let stream_limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_STREAMS));
    let tunnel_slots = Arc::new(Semaphore::new(MAX_TUNNELS));
    let mut in_flight: HashMap<Uuid, CancellationToken> = HashMap::new();
    let mut tunnel_routes: HashMap<Uuid, mpsc::Sender<Vec<u8>>> = HashMap::new();
    let (finished_tx, mut finished) = mpsc::unbounded_channel::<Uuid>();
    // Cancelado quando o laço termina (canal caiu ou encerramento): distingue
    // essa causa do cancelamento pedido pela central (`Body::Cancel`).
    let session_closed = CancellationToken::new();
    let mut live_interval = welcome.live_interval_ms;
    let mut live_ticks: u32 = 0;
    let mut live_timer = live_ticker(live_interval);
    let mut flush_timer = tokio::time::interval(OUTBOX_FLUSH);
    flush_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            biased;
            () = loop_stop.cancelled() => break,
            frame = incoming.recv() => {
                let text = match frame {
                    Some(WireFrame::Text(text)) => text,
                    Some(WireFrame::Binary(bytes)) => {
                        route_tunnel_frame(&mut tunnel_routes, &in_flight, bytes);
                        continue;
                    }
                    None => break,
                };
                let Ok(envelope) = Envelope::decode(&text) else {
                    tracing::warn!("quadro inválido da central");
                    continue;
                };
                match envelope.body {
                    Body::Request { command: Command::SetLive { interval_ms }, .. } => {
                        live_interval = interval_ms;
                        live_timer = live_ticker(live_interval);
                        live_ticks = 0;
                        if live_interval == 0 {
                            live.stop();
                        }
                        send_control_nonblocking(
                            &envelopes,
                            Envelope::reply(
                                envelope.id,
                                Body::Response { outcome: Outcome::Ok { value: Value::Null } },
                            ),
                        );
                    }
                    Body::Request { deadline_ms, command } => {
                        let token = CancellationToken::new();
                        in_flight.insert(envelope.id, token.clone());
                        match TunnelEnd::of(&command) {
                            Some(end) => {
                                let (route, frames) = mpsc::channel(INBOUND_CAPACITY);
                                tunnel_routes.insert(envelope.id, route);
                                tokio::spawn(serve_tunnel(TunnelRequest {
                                    opener: deps.tunnels.clone(),
                                    slots: tunnel_slots.clone(),
                                    envelopes: envelopes.clone(),
                                    tunnel_out: tunnel_out.clone(),
                                    finished: finished_tx.clone(),
                                    id: envelope.id,
                                    end,
                                    frames,
                                    token,
                                }));
                            }
                            None => {
                                let chosen_limiter = if is_long_running(&command) {
                                    stream_limiter.clone()
                                } else {
                                    limiter.clone()
                                };
                                tokio::spawn(handle_request(
                                    deps.clone(),
                                    envelopes.clone(),
                                    refresh.clone(),
                                    chosen_limiter,
                                    finished_tx.clone(),
                                    envelope.id,
                                    Duration::from_millis(deadline_ms),
                                    command,
                                    token,
                                    session_closed.clone(),
                                ));
                            }
                        }
                    }
                    Body::Cancel => {
                        if let Some(token) = in_flight.remove(&envelope.id) {
                            token.cancel();
                        }
                        tunnel_routes.remove(&envelope.id);
                    }
                    _ => {}
                }
            }
            Some(id) = finished.recv() => {
                in_flight.remove(&id);
                tunnel_routes.remove(&id);
            }
            _ = live_timer.tick(), if live_interval > 0 => {
                live.request(live_ticks.is_multiple_of(INVENTORY_EVERY), false);
                live_ticks = live_ticks.wrapping_add(1);
            }
            () = refresh.notified(), if live_interval > 0 => {
                live.request(true, true);
            }
            () = live.finished() => {
                live.collected(live_interval > 0);
            }
            _ = flush_timer.tick() => {
                flush_outbox_step(&deps.outbox, &envelopes);
            }
        }
    }

    session_closed.cancel();
    for token in in_flight.into_values() {
        token.cancel();
    }
    live.stop();
    drop(tunnel_routes);
    drop(tunnel_out);
    drop(envelopes);
    writer.abort();
    Ok(welcome)
}

/// Envia um quadro de controle sem jamais bloquear o laço de leitura.
fn send_control_nonblocking(envelopes: &mpsc::Sender<Envelope>, envelope: Envelope) {
    if let Err(mpsc::error::TrySendError::Full(envelope)) = envelopes.try_send(envelope) {
        let envelopes = envelopes.clone();
        tokio::spawn(async move {
            let _ = envelopes.send(envelope).await;
        });
    }
}

fn live_ticker(interval_ms: u64) -> tokio::time::Interval {
    let period = Duration::from_millis(interval_ms.max(500));
    let mut ticker = tokio::time::interval(period);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ticker
}

/// A coleta ao vivo, **fora** do laço que atende a central.
///
/// Ler status, containers e `stats` de cada container leva segundos (a
/// Engine espera uma segunda amostra para a CPU), e o inventário mais ainda.
/// Feita dentro do `select!`, ela segurava a leitura dos pedidos: uma
/// listagem ou um `stop` ficavam na fila até a coleta acabar e o navegador
/// desistia antes — a ação acontecia e a tela dizia "falha de conexão".
///
/// Uma coleta por vez: um tick que chega no meio dela é dispensado (a leitura
/// em curso já é a mais nova); um pedido de inventário ou de atualização
/// depois de uma mudança fica guardado e roda assim que ela termina.
struct LivePublisher {
    deps: SessionDeps,
    envelopes: mpsc::Sender<Envelope>,
    running: Option<tokio::task::JoinHandle<()>>,
    queued: Option<bool>,
}

impl LivePublisher {
    const fn new(deps: SessionDeps, envelopes: mpsc::Sender<Envelope>) -> Self {
        Self {
            deps,
            envelopes,
            running: None,
            queued: None,
        }
    }

    /// Pede uma coleta. `must_run` vale para depois de uma mudança: mesmo
    /// com outra em curso, uma nova roda em seguida.
    fn request(&mut self, with_inventory: bool, must_run: bool) {
        if self.running.is_none() {
            self.start(with_inventory);
        } else if with_inventory || must_run {
            self.queued = Some(self.queued.unwrap_or(false) || with_inventory);
        }
    }

    fn start(&mut self, with_inventory: bool) {
        let deps = self.deps.clone();
        let envelopes = self.envelopes.clone();
        self.running = Some(tokio::spawn(async move {
            publish_live(&deps, &envelopes, with_inventory).await;
        }));
    }

    /// Termina quando a coleta em curso acaba; sem coleta, nunca.
    async fn finished(&mut self) {
        match self.running.as_mut() {
            Some(handle) => {
                let _ = handle.await;
            }
            None => std::future::pending().await,
        }
    }

    /// A coleta acabou: roda a que ficou guardada, se a central ainda quer.
    fn collected(&mut self, enabled: bool) {
        self.running = None;
        if let Some(with_inventory) = self.queued.take() {
            if enabled {
                self.start(with_inventory);
            }
        }
    }

    fn stop(&mut self) {
        if let Some(handle) = self.running.take() {
            handle.abort();
        }
    }
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

/// Despacha um lote pequeno do buffer offline sem bloquear o laço de leitura.
/// Eventos só saem do disco depois de aceitos pela fila de saída.
fn flush_outbox_step(outbox: &Outbox, envelopes: &mpsc::Sender<Envelope>) {
    let pending = outbox.peek_batch(OUTBOX_BATCH);
    if pending.is_empty() {
        return;
    }
    let mut sent = Vec::with_capacity(pending.len());
    for (seq, event) in pending {
        if envelopes
            .try_send(Envelope::new(Body::Event { event }))
            .is_ok()
        {
            sent.push(seq);
        } else {
            break;
        }
    }
    outbox.ack(&sent);
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
    session_closed: CancellationToken,
) {
    let deadline_at = tokio::time::Instant::now() + deadline.max(Duration::from_secs(1));
    let _permit = tokio::select! {
        biased;
        () = token.cancelled() => {
            let _ = finished.send(id);
            return;
        }
        () = tokio::time::sleep_until(deadline_at) => {
            let _ = finished.send(id);
            let _ = envelopes
                .send(Envelope::reply(
                    id,
                    Body::Response {
                        outcome: Err::<Value, RemoteError>(RemoteError::timeout()).into(),
                    },
                ))
                .await;
            return;
        }
        permit = limiter.acquire_owned() => match permit {
            Ok(permit) => permit,
            Err(_) => {
                let _ = finished.send(id);
                return;
            }
        }
    };
    let replay = Replay::of(&command);
    let mutates =
        command.permission() != Permission::Read && matches!(command, Command::Docker { .. });
    let droppable_chunks = is_droppable_stream(&command);

    let (chunks, mut chunk_rx) = mpsc::unbounded_channel::<Value>();
    let exec_cancel = token.child_token();
    let mut forward = {
        let envelopes = envelopes.clone();
        let cancel = exec_cancel.clone();
        tokio::spawn(async move {
            loop {
                let data = tokio::select! {
                    biased;
                    () = cancel.cancelled() => break,
                    item = chunk_rx.recv() => match item {
                        Some(data) => data,
                        None => break,
                    },
                };
                let frame = Envelope::reply(id, Body::Chunk { data });
                if droppable_chunks {
                    match envelopes.try_send(frame) {
                        Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                } else if envelopes.send(frame).await.is_err() {
                    break;
                }
            }
        })
    };
    let result = match tokio::time::timeout_at(
        deadline_at,
        deps.executor.execute(command, chunks, exec_cancel.clone()),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            exec_cancel.cancel();
            Err(RemoteError::timeout())
        }
    };
    if exec_cancel.is_cancelled() {
        if tokio::time::timeout(CHUNK_FORWARD_GRACE, &mut forward)
            .await
            .is_err()
        {
            forward.abort();
        }
    } else {
        let _ = forward.await;
    }
    let _ = finished.send(id);
    if session_closed.is_cancelled() {
        // O canal caiu (ou o agente encerra) com o pedido em curso: o
        // resultado pronto vai para o buffer offline e sai na próxima
        // conexão. Só o erro do próprio cancelamento é descartado.
        let cancelled_error = token.is_cancelled() && result.is_err();
        if !cancelled_error {
            if let Some(event) = replay.event(result) {
                deps.outbox.push(event).await;
            }
        }
        return;
    }
    // Cancelado pela central: ela já desistiu, não há resposta. Timeout
    // local não cancela `token` e ainda avisa a central.
    if token.is_cancelled() {
        return;
    }
    if mutates && result.is_ok() {
        refresh.notify_one();
    }
    let response = Envelope::reply(
        id,
        Body::Response {
            outcome: result.into(),
        },
    );
    if let Err(mpsc::error::SendError(envelope)) = envelopes.send(response).await {
        // O canal caiu com o resultado pronto: recupera o valor sem clonar no
        // caminho feliz e guarda para a próxima conexão.
        if let Body::Response { outcome } = envelope.body {
            let result: Result<Value, RemoteError> = outcome.into();
            if let Some(event) = replay.event(result) {
                deps.outbox.push(event).await;
            }
        }
    }
}

/// Entrega um quadro binário cru à ponte dele sem descomprimir no laço do
/// WebSocket. Quem estoura a janela de crédito perde a ponte e tem o token
/// cancelado — é violação do protocolo, não carga.
fn route_tunnel_frame(
    routes: &mut HashMap<Uuid, mpsc::Sender<Vec<u8>>>,
    in_flight: &HashMap<Uuid, CancellationToken>,
    bytes: Vec<u8>,
) {
    let Some(id) = TunnelFrame::peek_id(&bytes) else {
        tracing::warn!("quadro de ponte inválido da central");
        return;
    };
    if let Some(route) = routes.get(&id) {
        if route.try_send(bytes).is_err() {
            tracing::warn!("ponte excedeu a janela de crédito; encerrada");
            routes.remove(&id);
            if let Some(token) = in_flight.get(&id) {
                token.cancel();
            }
        }
    }
}

/// A ponta local que a ponte liga ao canal.
enum TunnelEnd {
    Database {
        host: String,
        port: u16,
    },
    Volume {
        name: String,
    },
    /// A imagem compilada aqui, a caminho do registry da central (ADR 023).
    Artifact {
        artifact: Uuid,
    },
    /// Uma imagem do registry da central, a caminho do Docker daqui (ADR 022).
    Import {
        request: ImageImportRequest,
    },
}

impl TunnelEnd {
    /// A ponta pedida, quando o comando é uma ponte.
    fn of(command: &Command) -> Option<Self> {
        match command {
            Command::DatabaseTunnel { host, port } => Some(Self::Database {
                host: host.clone(),
                port: *port,
            }),
            Command::VolumeExport { name } => Some(Self::Volume { name: name.clone() }),
            Command::ImageExport { artifact } => Some(Self::Artifact {
                artifact: *artifact,
            }),
            Command::ImageImport { request } => Some(Self::Import {
                request: request.clone(),
            }),
            _ => None,
        }
    }

    async fn open(&self, opener: &dyn TunnelOpener) -> Result<TunnelStream, RemoteError> {
        match self {
            Self::Database { host, port } => opener.open(host, *port).await,
            Self::Volume { name } => opener.open_volume(name).await,
            Self::Artifact { artifact } => opener.open_artifact(*artifact).await,
            Self::Import { request } => opener.open_import(request).await,
        }
    }
}

/// O que atender uma ponte precisa.
struct TunnelRequest {
    opener: Arc<dyn TunnelOpener>,
    slots: Arc<Semaphore>,
    envelopes: mpsc::Sender<Envelope>,
    tunnel_out: mpsc::Sender<WireFrame>,
    finished: mpsc::UnboundedSender<Uuid>,
    id: Uuid,
    end: TunnelEnd,
    frames: mpsc::Receiver<Vec<u8>>,
    token: CancellationToken,
}

/// Abre a ponta local, avisa "aberta" e bombeia até o fim. A resposta leva os
/// totais; cancelada pela central, não há resposta.
async fn serve_tunnel(request: TunnelRequest) {
    let TunnelRequest {
        opener,
        slots,
        envelopes,
        tunnel_out,
        finished,
        id,
        end,
        frames,
        token,
    } = request;
    let result = async {
        let _slot = slots.try_acquire_owned().map_err(|_| {
            RemoteError::new(
                ErrorCode::Conflict,
                format!("Este agente já tem {MAX_TUNNELS} pontes abertas; tente em seguida"),
            )
        })?;
        let stream = tokio::select! {
            biased;
            () = token.cancelled() => return Err(RemoteError::cancelled()),
            opened = end.open(opener.as_ref()) => opened?,
        };
        envelopes
            .send(Envelope::reply(
                id,
                Body::Chunk {
                    data: serde_json::json!({ "opened": true }),
                },
            ))
            .await
            .map_err(|_| RemoteError::disconnected())?;
        let totals = pump_raw(id, stream, frames, tunnel_out, token.clone())
            .await
            .map_err(|error| RemoteError::new(ErrorCode::Unavailable, error.to_string()))?;
        Ok(serde_json::json!({
            "sent": totals.sent,
            "sentOnWire": totals.sent_on_wire,
            "received": totals.received,
        }))
    }
    .await;
    let _ = finished.send(id);
    if token.is_cancelled() {
        return;
    }
    let _ = envelopes
        .send(Envelope::reply(
            id,
            Body::Response {
                outcome: result.into(),
            },
        ))
        .await;
}

/// Destino do amostrador no agente: o minuto fechado entra no buffer e sai
/// na próxima descarga (ou na próxima conexão).
pub struct OutboxSink(pub Arc<Outbox>);

#[async_trait]
impl super::sampler::RollupSink for OutboxSink {
    async fn accept(&self, rollup: crate::services::telemetry::rollup::MetricsRollup) {
        self.0
            .push(AgentEvent::MetricsRollup {
                rollup: Box::new(rollup),
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::services::agents::protocol::{Capability, DockerCall, PROTOCOL_VERSION};

    /// Coleta que nunca termina: a Engine travada num `stats`.
    struct StuckLive;

    #[async_trait]
    impl LiveSource for StuckLive {
        async fn live(&self) -> Option<DockerLiveSnapshot> {
            std::future::pending().await
        }

        async fn inventory(&self) -> Option<DockerInventorySnapshot> {
            std::future::pending().await
        }
    }

    struct ListsNothing;

    #[async_trait]
    impl CommandExecutor for ListsNothing {
        async fn execute(
            &self,
            _command: Command,
            _chunks: mpsc::UnboundedSender<Value>,
            _cancel: CancellationToken,
        ) -> Result<Value, RemoteError> {
            Ok(json!([]))
        }
    }

    struct NoTunnels;

    #[async_trait]
    impl TunnelOpener for NoTunnels {
        async fn open(&self, _host: &str, _port: u16) -> Result<TunnelStream, RemoteError> {
            Err(RemoteError::new(ErrorCode::Unsupported, "sem ponte"))
        }

        async fn open_volume(&self, _name: &str) -> Result<TunnelStream, RemoteError> {
            Err(RemoteError::new(ErrorCode::Unsupported, "sem ponte"))
        }
    }

    fn hello() -> Hello {
        Hello {
            protocol: PROTOCOL_VERSION,
            agent_version: "0.1.0".into(),
            hostname: "srv".into(),
            os: "Debian".into(),
            arch: "x86_64".into(),
            in_container: false,
            policy: vec![Permission::Read],
            docker: Capability::default(),
            compose: Capability::default(),
        }
    }

    fn text(envelope: &Envelope) -> WireFrame {
        WireFrame::Text(envelope.encode().expect("quadro"))
    }

    #[tokio::test]
    async fn pedido_da_central_nao_espera_a_coleta_ao_vivo() {
        let (to_agent, incoming) = mpsc::channel(16);
        let (outgoing, mut from_agent) = mpsc::channel(16);
        let deps = SessionDeps {
            executor: Arc::new(ListsNothing),
            outbox: Arc::new(Outbox::in_memory(10)),
            live: Arc::new(StuckLive),
            tunnels: Arc::new(NoTunnels),
        };
        let agent = tokio::spawn(run(deps, hello(), incoming, outgoing));

        // Hello → Welcome com o ao vivo ligado: o primeiro tick dispara na
        // hora e a coleta trava.
        from_agent.recv().await.expect("hello");
        to_agent
            .send(text(&Envelope::new(Body::Welcome(Welcome {
                probe_id: 1,
                name: "srv".into(),
                live_interval_ms: 500,
            }))))
            .await
            .expect("welcome");
        tokio::time::sleep(Duration::from_millis(50)).await;

        let request = Envelope::new(Body::Request {
            deadline_ms: 5_000,
            command: Command::Docker {
                call: DockerCall::ListContainers,
            },
        });
        to_agent.send(text(&request)).await.expect("pedido");

        let answered = tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(WireFrame::Text(text)) = from_agent.recv().await {
                let envelope = Envelope::decode(&text).expect("quadro do agente");
                if envelope.id == request.id {
                    return Some(envelope.body);
                }
            }
            None
        })
        .await
        .expect("a resposta não pode esperar a coleta ao vivo terminar");
        assert!(matches!(
            answered,
            Some(Body::Response {
                outcome: Outcome::Ok { .. }
            })
        ));

        drop(to_agent);
        let _ = tokio::time::timeout(Duration::from_secs(2), agent).await;
    }

    struct SlowFollowFastList;

    #[async_trait]
    impl CommandExecutor for SlowFollowFastList {
        async fn execute(
            &self,
            command: Command,
            _chunks: mpsc::UnboundedSender<Value>,
            cancel: CancellationToken,
        ) -> Result<Value, RemoteError> {
            match command {
                Command::Docker {
                    call: DockerCall::FollowLogs { .. },
                } => {
                    cancel.cancelled().await;
                    Err(RemoteError::cancelled())
                }
                Command::Docker {
                    call: DockerCall::Status,
                } => {
                    std::future::pending::<()>().await;
                    unreachable!()
                }
                _ => Ok(json!(["ok"])),
            }
        }
    }

    #[tokio::test]
    async fn follow_logs_lotado_nao_trava_pedidos_rapidos() {
        let (to_agent, incoming) = mpsc::channel(32);
        let (outgoing, mut from_agent) = mpsc::channel(32);
        let deps = SessionDeps {
            executor: Arc::new(SlowFollowFastList),
            outbox: Arc::new(Outbox::in_memory(10)),
            live: Arc::new(NoLiveSource),
            tunnels: Arc::new(NoTunnels),
        };
        let agent = tokio::spawn(run(deps, hello(), incoming, outgoing));

        from_agent.recv().await.expect("hello");
        to_agent
            .send(text(&Envelope::new(Body::Welcome(Welcome {
                probe_id: 1,
                name: "srv".into(),
                live_interval_ms: 0,
            }))))
            .await
            .expect("welcome");

        // Lotar todos os slots de stream com FollowLogs em aberto.
        for i in 0..MAX_CONCURRENT_STREAMS {
            let follow = Envelope::new(Body::Request {
                deadline_ms: 60_000,
                command: Command::Docker {
                    call: DockerCall::FollowLogs {
                        id: format!("c-{i}"),
                        tail: "10".into(),
                    },
                },
            });
            to_agent.send(text(&follow)).await.expect("follow");
        }
        tokio::time::sleep(Duration::from_millis(30)).await;

        let list = Envelope::new(Body::Request {
            deadline_ms: 5_000,
            command: Command::Docker {
                call: DockerCall::ListContainers,
            },
        });
        to_agent.send(text(&list)).await.expect("list");

        let answered = tokio::time::timeout(Duration::from_secs(2), async {
            while let Some(WireFrame::Text(raw)) = from_agent.recv().await {
                let env = Envelope::decode(&raw).expect("quadro");
                if env.id == list.id {
                    return Some(env.body);
                }
            }
            None
        })
        .await
        .expect("pedido rápido não pode esperar FollowLogs");
        assert!(matches!(
            answered,
            Some(Body::Response {
                outcome: Outcome::Ok { .. }
            })
        ));

        drop(to_agent);
        let _ = tokio::time::timeout(Duration::from_secs(2), agent).await;
    }

    #[tokio::test]
    async fn timeout_local_ainda_responde_erro_timeout() {
        let (to_agent, incoming) = mpsc::channel(16);
        let (outgoing, mut from_agent) = mpsc::channel(16);
        let deps = SessionDeps {
            executor: Arc::new(SlowFollowFastList),
            outbox: Arc::new(Outbox::in_memory(10)),
            live: Arc::new(NoLiveSource),
            tunnels: Arc::new(NoTunnels),
        };
        let agent = tokio::spawn(run(deps, hello(), incoming, outgoing));

        from_agent.recv().await.expect("hello");
        to_agent
            .send(text(&Envelope::new(Body::Welcome(Welcome {
                probe_id: 1,
                name: "srv".into(),
                live_interval_ms: 0,
            }))))
            .await
            .expect("welcome");

        let req = Envelope::new(Body::Request {
            deadline_ms: 1_000,
            command: Command::Docker {
                call: DockerCall::Status,
            },
        });
        to_agent.send(text(&req)).await.expect("pedido");

        let answered = tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(WireFrame::Text(raw)) = from_agent.recv().await {
                let env = Envelope::decode(&raw).expect("quadro");
                if env.id == req.id {
                    return Some(env.body);
                }
            }
            None
        })
        .await
        .expect("timeout local precisa responder à central");
        assert!(matches!(
            answered,
            Some(Body::Response {
                outcome: Outcome::Err { error }
            }) if error.code == ErrorCode::Timeout
        ));

        drop(to_agent);
        let _ = tokio::time::timeout(Duration::from_secs(2), agent).await;
    }

    /// Monitor que termina depois de o canal cair (e ignora o cancelamento,
    /// como o executor real).
    struct SlowMonitor;

    #[async_trait]
    impl CommandExecutor for SlowMonitor {
        async fn execute(
            &self,
            _command: Command,
            _chunks: mpsc::UnboundedSender<Value>,
            _cancel: CancellationToken,
        ) -> Result<Value, RemoteError> {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let now = chrono::Utc::now();
            Ok(serde_json::to_value(CheckResult {
                success: true,
                status: crate::services::monitoring::contracts::MonitorStatus::Up,
                started_at: now,
                finished_at: now,
                duration_ms: 1,
                message: None,
                metrics: Vec::new(),
                data: Value::Null,
            })
            .expect("resultado"))
        }
    }

    #[tokio::test]
    async fn resultado_de_monitor_apos_queda_do_canal_vai_para_o_buffer() {
        let (to_agent, incoming) = mpsc::channel(16);
        let (outgoing, mut from_agent) = mpsc::channel(16);
        let outbox = Arc::new(Outbox::in_memory(10));
        let deps = SessionDeps {
            executor: Arc::new(SlowMonitor),
            outbox: outbox.clone(),
            live: Arc::new(NoLiveSource),
            tunnels: Arc::new(NoTunnels),
        };
        let agent = tokio::spawn(run(deps, hello(), incoming, outgoing));

        from_agent.recv().await.expect("hello");
        to_agent
            .send(text(&Envelope::new(Body::Welcome(Welcome {
                probe_id: 1,
                name: "srv".into(),
                live_interval_ms: 0,
            }))))
            .await
            .expect("welcome");
        let request = Envelope::new(Body::Request {
            deadline_ms: 5_000,
            command: Command::Monitor {
                task: agent_protocol::probes::ProbeTask {
                    id: "t-1".into(),
                    monitor_id: 42,
                    task_type: "ping".into(),
                    timeout_ms: 1_000,
                    payload: Value::Null,
                },
            },
        });
        to_agent.send(text(&request)).await.expect("pedido");
        tokio::time::sleep(Duration::from_millis(30)).await;
        // O canal cai com o monitor em curso.
        drop(to_agent);
        let _ = tokio::time::timeout(Duration::from_secs(2), agent).await;

        let saved = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if !outbox.is_empty() {
                    return outbox.drain();
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("o resultado pronto não pode se perder");
        assert!(matches!(
            saved.as_slice(),
            [AgentEvent::MonitorResult { monitor_id: 42, .. }]
        ));
    }
}
