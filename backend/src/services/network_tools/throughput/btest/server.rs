//! Servidor btest da central — **armado, nunca aberto** (ADR 015 §1).
//!
//! A porta fica escutando, mas uma conexão só é atendida se houver uma
//! sessão armada para o IP de origem ([`BtestServer::arm`]); fora disso,
//! fecha na hora. A sessão é de uso único e expira (TTL padrão 90 s).
//!
//! O equipamento (MikroTik, `/tool bandwidth-test`) é o **cliente** e quem
//! dirige o teste: escolhe direção e duração e fecha a conexão no fim. Por
//! isso o relatório entregue aqui já vem **do ponto de vista do cliente**:
//! `tx` = o que o equipamento enviou (a central recebeu), `rx` = o que o
//! equipamento recebeu (a central enviou). As amostras seguem a mesma regra.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};

use loco_rs::app::AppContext;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    time::{timeout, Instant},
};
use tokio_util::sync::CancellationToken;

use super::{
    codec::{protocol_error, Command, AUTH_OK, COMMAND_LEN, HELLO},
    duplex::{self, block_size, DuplexPlan},
    UNAVAILABLE_MESSAGE,
};
use crate::services::{
    network_tools::throughput::{
        cancelled, sample_channel, Flow, SampleSink, ThroughputEngine, ThroughputReport,
        ThroughputSample, HANDSHAKE_TIMEOUT, MAX_DURATION_S,
    },
    shared::errors::{AppError, AppResult},
};

/// Teto de uma sessão depois de conectada: a maior duração do contrato nos
/// dois sentidos, com folga. O cliente normalmente fecha bem antes.
const SESSION_CAP: Duration = Duration::from_secs(2 * MAX_DURATION_S as u64 + 15);

type ReportSender = oneshot::Sender<AppResult<ThroughputReport>>;

/// Sessão armada à espera da conexão.
struct Pending {
    id: u64,
    expires_at: Instant,
    sink: SampleSink,
    report: ReportSender,
    cancel: CancellationToken,
}

struct Inner {
    local_addr: SocketAddr,
    sessions: Mutex<HashMap<IpAddr, Pending>>,
    /// A última conexão recusada por não ter sessão armada — só diagnóstico:
    /// quando um equipamento armado não conecta, ela costuma ser ele chegando
    /// por NAT com outro endereço de origem.
    last_refused: Mutex<Option<(IpAddr, Instant)>>,
    next_id: AtomicU64,
    shutdown: CancellationToken,
}

impl Inner {
    fn sessions(&self) -> std::sync::MutexGuard<'_, HashMap<IpAddr, Pending>> {
        // Nenhuma seção crítica entra em pânico; um lock envenenado ainda
        // guarda um mapa coerente.
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn remember_refused(&self, peer: IpAddr) {
        *self
            .last_refused
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((peer, Instant::now()));
    }

    /// IP recusado desde `since`, se houve.
    fn refused_since(&self, since: Instant) -> Option<IpAddr> {
        self.last_refused
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .filter(|(_, at)| *at >= since)
            .map(|(ip, _)| ip)
    }

    /// Tira a sessão do mapa se ainda for a mesma (`id`).
    fn take(&self, peer: IpAddr, id: u64) -> Option<Pending> {
        let mut sessions = self.sessions();
        match sessions.get(&peer) {
            Some(pending) if pending.id == id => sessions.remove(&peer),
            _ => None,
        }
    }
}

/// Servidor btest em processo. Clonar é barato (handle compartilhado).
#[derive(Clone)]
pub struct BtestServer {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for BtestServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtestServer")
            .field("local_addr", &self.inner.local_addr)
            .finish_non_exhaustive()
    }
}

impl BtestServer {
    /// Abre a porta e começa a aceitar conexões (só atende as armadas).
    ///
    /// # Errors
    ///
    /// [`AppError::ServiceUnavailable`] se a porta não puder ser aberta
    /// (ocupada, sem permissão).
    pub async fn bind(addr: SocketAddr) -> AppResult<Self> {
        let listener = TcpListener::bind(addr).await.map_err(|error| {
            AppError::service_unavailable(format!(
                "{UNAVAILABLE_MESSAGE}: não foi possível escutar em {addr}: {error}"
            ))
        })?;
        let local_addr = listener
            .local_addr()
            .map_err(|error| AppError::Internal(error.into()))?;
        let inner = Arc::new(Inner {
            local_addr,
            sessions: Mutex::new(HashMap::new()),
            last_refused: Mutex::new(None),
            next_id: AtomicU64::new(1),
            shutdown: CancellationToken::new(),
        });
        tokio::spawn(accept_loop(
            listener,
            Arc::downgrade(&inner),
            inner.shutdown.clone(),
        ));
        Ok(Self { inner })
    }

    /// Endereço efetivamente escutado (útil com porta 0).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    /// Arma uma sessão de uso único para `peer` por até `ttl`.
    ///
    /// A primeira conexão vinda de `peer` (IPv4 mapeado em IPv6 conta como
    /// IPv4) consome a sessão; as demais, e as de qualquer outro IP, são
    /// fechadas na hora. Sem conexão até o `ttl`, o relatório da sessão vira
    /// erro de prazo. Descartar a [`ArmedSession`] desarma (ou cancela o teste
    /// em curso).
    ///
    /// # Errors
    ///
    /// [`AppError::Conflict`] se já houver sessão armada para o mesmo IP.
    pub fn arm(&self, peer: IpAddr, ttl: Duration) -> AppResult<ArmedSession> {
        let peer = peer.to_canonical();
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (sink, samples) = sample_channel();
        let (report_tx, report_rx) = oneshot::channel();
        let cancel = CancellationToken::new();
        {
            let mut sessions = self.inner.sessions();
            if sessions
                .get(&peer)
                .is_some_and(|pending| pending.expires_at > Instant::now())
            {
                return Err(AppError::conflict(format!(
                    "Já existe uma sessão btest armada para {peer}"
                )));
            }
            sessions.insert(
                peer,
                Pending {
                    id,
                    expires_at: Instant::now() + ttl,
                    sink,
                    report: report_tx,
                    cancel: cancel.clone(),
                },
            );
        }
        tokio::spawn(expire(
            Arc::downgrade(&self.inner),
            peer,
            id,
            ttl,
            cancel.clone(),
        ));
        Ok(ArmedSession {
            peer,
            samples,
            outcome: SessionOutcome {
                report: Some(report_rx),
            },
            cancel,
            id,
            armed_at: Instant::now(),
            server: Arc::downgrade(&self.inner),
        })
    }

    /// Para de aceitar conexões. Sessões em curso seguem até o fim.
    pub fn shutdown(&self) {
        self.inner.shutdown.cancel();
    }

    /// Guarda o servidor no `shared_store` do processo (initializer).
    pub fn install(&self, ctx: &AppContext) {
        ctx.shared_store.insert(self.clone());
    }

    /// O servidor btest deste processo.
    ///
    /// # Errors
    ///
    /// [`AppError::ServiceUnavailable`] ("servidor btest indisponível") quando
    /// o processo não o abriu: `BTEST_LISTEN` vazio, porta ocupada, ambiente
    /// de teste ou processo que não é o da API.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store
            .get::<Self>()
            .ok_or_else(|| AppError::service_unavailable(UNAVAILABLE_MESSAGE))
    }
}

/// Sessão armada: amostras ao vivo, relatório final e cancelamento.
///
/// Os campos são independentes, então dá para ler `samples` e esperar o
/// `outcome` no mesmo `tokio::select!`.
pub struct ArmedSession {
    /// IP (canônico) de quem pode conectar.
    pub peer: IpAddr,
    /// Amostras de 1 s, do ponto de vista do cliente.
    pub samples: mpsc::UnboundedReceiver<ThroughputSample>,
    /// Relatório final.
    pub outcome: SessionOutcome,
    cancel: CancellationToken,
    id: u64,
    armed_at: Instant,
    server: Weak<Inner>,
}

impl ArmedSession {
    /// IP de uma conexão recusada desde que a sessão foi armada — quase
    /// sempre o próprio equipamento chegando por NAT. Só diagnóstico: a
    /// recusa continua valendo.
    #[must_use]
    pub fn stray_peer(&self) -> Option<IpAddr> {
        self.server
            .upgrade()
            .and_then(|inner| inner.refused_since(self.armed_at))
            .filter(|ip| *ip != self.peer)
    }

    /// Cancela: desarma se ainda não conectou, ou interrompe o teste.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Token de cancelamento da sessão.
    #[must_use]
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }
}

impl Drop for ArmedSession {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(inner) = self.server.upgrade() {
            inner.take(self.peer, self.id);
        }
    }
}

/// Relatório final de uma sessão armada.
pub struct SessionOutcome {
    report: Option<oneshot::Receiver<AppResult<ThroughputReport>>>,
}

impl SessionOutcome {
    /// Espera o fim da sessão.
    ///
    /// # Errors
    ///
    /// Prazo do arm vencido sem conexão, cancelamento, comando fora do que o
    /// servidor atende (UDP, várias conexões) ou relatório já consumido.
    pub async fn wait(&mut self) -> AppResult<ThroughputReport> {
        let Some(report) = self.report.as_mut() else {
            return Err(AppError::conflict("Relatório da sessão btest já consumido"));
        };
        let result = report.await.unwrap_or_else(|_| {
            Err(AppError::service_unavailable(
                "Sessão btest encerrada sem relatório",
            ))
        });
        self.report = None;
        result
    }
}

/// Desarma a sessão quando o prazo vence (ou quando ela é cancelada) sem que
/// ninguém tenha conectado.
async fn expire(
    server: Weak<Inner>,
    peer: IpAddr,
    id: u64,
    ttl: Duration,
    cancel: CancellationToken,
) {
    let armed_at = Instant::now();
    let timed_out = tokio::select! {
        () = tokio::time::sleep(ttl) => true,
        () = cancel.cancelled() => false,
    };
    let Some(inner) = server.upgrade() else {
        return;
    };
    let error = if timed_out {
        AppError::service_unavailable(no_show_message(peer, inner.refused_since(armed_at)))
    } else {
        cancelled()
    };
    if let Some(pending) = inner.take(peer, id) {
        let _ = pending.report.send(Err(error));
    }
}

/// Mensagem de quem foi armado e não conectou. Se outro IP bateu na porta
/// durante a janela, quase sempre é o próprio equipamento atrás de NAT — dizer
/// isso poupa o operador de caçar firewall que não existe.
fn no_show_message(peer: IpAddr, refused: Option<IpAddr>) -> String {
    match refused {
        Some(other) => format!(
            "O equipamento {peer} não conectou ao servidor btest dentro do prazo. Uma conexão \
             chegou de {other}, que não é o IP cadastrado: há NAT entre o equipamento e o \
             servidor? Cadastre o IP pelo qual o servidor vê o equipamento ou rode o teste \
             sem NAT no caminho"
        ),
        None => format!(
            "O equipamento {peer} não conectou ao servidor btest dentro do prazo: confira se \
             ele alcança o servidor na porta TCP do btest (firewall, rota) e o endereço do \
             servidor em Configurações"
        ),
    }
}

async fn accept_loop(listener: TcpListener, server: Weak<Inner>, shutdown: CancellationToken) {
    loop {
        let accepted = tokio::select! {
            () = shutdown.cancelled() => break,
            accepted = listener.accept() => accepted,
        };
        let (stream, remote) = match accepted {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(%error, "btest: falha ao aceitar conexão");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let Some(inner) = server.upgrade() else {
            break;
        };
        let peer = remote.ip().to_canonical();
        let pending = {
            let mut sessions = inner.sessions();
            match sessions.remove(&peer) {
                Some(p) if p.expires_at > Instant::now() && !p.cancel.is_cancelled() => Some(p),
                Some(expired) => {
                    let _ = expired
                        .report
                        .send(Err(AppError::service_unavailable(format!(
                            "O equipamento {peer} conectou ao servidor btest depois do prazo"
                        ))));
                    None
                }
                None => None,
            }
        };
        match pending {
            Some(pending) => {
                tracing::info!(%peer, "btest: sessão armada conectou");
                tokio::spawn(serve(stream, pending));
            }
            None => {
                // Fechar sem dizer nada: quem não foi armado não sabe nem que
                // isto é um btest.
                tracing::debug!(%peer, "btest: conexão sem sessão armada recusada");
                inner.remember_refused(peer);
                drop(stream);
            }
        }
    }
}

async fn serve(stream: TcpStream, pending: Pending) {
    let Pending {
        sink,
        report,
        cancel,
        ..
    } = pending;
    let result = tokio::select! {
        () = cancel.cancelled() => Err(cancelled()),
        result = run_session(stream, &sink, &cancel) => result,
    };
    if let Err(error) = &result {
        tracing::info!(%error, "btest: sessão terminou com erro");
    }
    let _ = report.send(result);
}

async fn run_session(
    mut stream: TcpStream,
    sink: &SampleSink,
    cancel: &CancellationToken,
) -> AppResult<ThroughputReport> {
    let _ = stream.set_nodelay(true);
    let command = handshake(&mut stream).await?;
    let plan = DuplexPlan {
        // Os papéis se invertem: o servidor envia o que o cliente recebe.
        send: command.client_rx,
        receive: command.client_tx,
        block: block_size(command.tx_size),
        pace_bps: (command.remote_tx_speed > 0).then_some(u64::from(command.remote_tx_speed)),
        max_duration: SESSION_CAP,
        sent_flow: Flow::Rx,
        received_flow: Flow::Tx,
    };
    let outcome = duplex::run(stream, &plan, sink, cancel).await?;
    let mut report = ThroughputReport::empty(ThroughputEngine::Btest);
    if command.client_tx {
        report.set_tx(outcome.received, outcome.elapsed);
    }
    if command.client_rx {
        report.set_rx(outcome.sent, outcome.elapsed);
    }
    report.samples = outcome.samples;
    Ok(report)
}

/// Hello, comando e "sem autenticação". Só TCP de uma conexão.
async fn handshake(stream: &mut TcpStream) -> AppResult<Command> {
    let silent =
        || AppError::service_unavailable("O equipamento parou de responder no handshake btest");
    let closed =
        |_| AppError::service_unavailable("O equipamento fechou a conexão btest no handshake");
    timeout(HANDSHAKE_TIMEOUT, stream.write_all(&HELLO))
        .await
        .map_err(|_| silent())?
        .map_err(closed)?;
    let mut buf = [0u8; COMMAND_LEN];
    timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut buf))
        .await
        .map_err(|_| silent())?
        .map_err(closed)?;
    let command = Command::decode(&buf)?;
    if !command.tcp {
        return Err(protocol_error("o servidor btest da central só atende TCP"));
    }
    if command.tcp_connection_count > 1 {
        return Err(protocol_error(
            "o servidor btest da central atende uma conexão TCP só (tcp-connection-count=1)",
        ));
    }
    timeout(HANDSHAKE_TIMEOUT, stream.write_all(&AUTH_OK))
        .await
        .map_err(|_| silent())?
        .map_err(closed)?;
    Ok(command)
}
