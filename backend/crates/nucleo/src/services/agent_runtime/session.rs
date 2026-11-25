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
                AgentEvent, Body, Command, Envelope, ErrorCode, Hello, Outcome, RemoteError,
                TunnelFrame, TunnelPayload, Welcome, WireFrame,
            },
            tunnel::{pump, TunnelStream, INBOUND_CAPACITY},
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
const MAX_CONCURRENT_REQUESTS: usize = 16;
const INVENTORY_EVERY: u32 = 5;
/// Pontes simultâneas (banco, ADR 013; volume, ADR 014): o resto dos pedidos
/// não pode ficar sem vaga por causa de backups e downloads.
const MAX_TUNNELS: usize = 2;
/// Quadros de ponte na fila de saída; o controle passa na frente.
const TUNNEL_BUFFER: usize = 16;

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
    mut incoming: mpsc::Receiver<WireFrame>,
    outgoing: mpsc::Sender<WireFrame>,
) -> Result<Welcome, SessionError> {
    let (envelopes, mut to_send) = mpsc::channel::<Envelope>(64);
    let (tunnel_out, mut tunnel_frames) = mpsc::channel::<WireFrame>(TUNNEL_BUFFER);
    // Controle primeiro: resultado de monitor não espera atrás de um dump.
    let writer = tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                biased;
                envelope = to_send.recv() => match envelope {
                    Some(envelope) => match envelope.encode() {
                        Ok(text) => WireFrame::Text(text),
                        Err(_) => continue,
                    },
                    None => break,
                },
                Some(frame) = tunnel_frames.recv() => frame,
            };
            if outgoing.send(frame).await.is_err() {
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
        Ok(Some(WireFrame::Text(text))) => {
            match Envelope::decode(&text).map(|envelope| envelope.body) {
                Ok(Body::Welcome(welcome)) => welcome,
                _ => {
                    writer.abort();
                    return Err(SessionError::NoWelcome);
                }
            }
        }
        Ok(Some(WireFrame::Binary(_))) => {
            writer.abort();
            return Err(SessionError::NoWelcome);
        }
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
    let mut live = LivePublisher::new(deps.clone(), envelopes.clone());
    let limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let tunnel_slots = Arc::new(Semaphore::new(MAX_TUNNELS));
    let mut in_flight: HashMap<Uuid, CancellationToken> = HashMap::new();
    let mut tunnel_routes: HashMap<Uuid, mpsc::Sender<TunnelPayload>> = HashMap::new();
    let (finished_tx, mut finished) = mpsc::unbounded_channel::<Uuid>();
    let mut live_interval = welcome.live_interval_ms;
    let mut live_ticks: u32 = 0;
    let mut live_timer = live_ticker(live_interval);
    let mut flush_timer = tokio::time::interval(OUTBOX_FLUSH);

    loop {
        tokio::select! {
            frame = incoming.recv() => {
                let text = match frame {
                    Some(WireFrame::Text(text)) => text,
                    Some(WireFrame::Binary(bytes)) => {
                        route_tunnel_frame(&mut tunnel_routes, &bytes);
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
                        let _ = envelopes
                            .send(Envelope::reply(envelope.id, Body::Response { outcome: Outcome::Ok { value: Value::Null } }))
                            .await;
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
                        }
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
                flush_outbox(&deps.outbox, &envelopes).await;
            }
        }
    }

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

/// Entrega um quadro binário à ponte dele. Quem estoura a janela de crédito
/// perde a ponte — é violação do protocolo, não carga.
fn route_tunnel_frame(routes: &mut HashMap<Uuid, mpsc::Sender<TunnelPayload>>, bytes: &[u8]) {
    let Ok(frame) = TunnelFrame::decode(bytes) else {
        tracing::warn!("quadro de ponte inválido da central");
        return;
    };
    if let Some(route) = routes.get(&frame.id) {
        if route.try_send(frame.payload).is_err() {
            tracing::warn!("ponte excedeu a janela de crédito; encerrada");
            routes.remove(&frame.id);
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
    frames: mpsc::Receiver<TunnelPayload>,
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
        let stream = end.open(opener.as_ref()).await?;
        envelopes
            .send(Envelope::reply(
                id,
                Body::Chunk {
                    data: serde_json::json!({ "opened": true }),
                },
            ))
            .await
            .map_err(|_| RemoteError::disconnected())?;
        let totals = pump(id, stream, frames, tunnel_out, token.clone())
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
        self.0.push(AgentEvent::MetricsRollup {
            rollup: Box::new(rollup),
        });
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
}
