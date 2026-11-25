//! A ponte vista pela central (ADR 013): um endereço em `127.0.0.1` que leva
//! ao banco da rede do agente.
//!
//! O `sqlx` só conecta por TCP — não aceita um canal injetado. Então a central
//! abre, só durante um backup ou uma restauração, um listener em
//! `127.0.0.1:<efêmera>` e aponta o `sqlx` para ele. Cada conexão aceita vira
//! uma ponte ([`OpenTunnel`]) pelo WebSocket do agente.
//!
//! A **primeira** ponte é aberta antes de devolver o endereço: se o agente
//! recusar (destino fora de `AGENT_DATABASE_TARGETS`, permissão desligada,
//! banco fora do ar), a mensagem dele chega ao operador — e não um
//! "connection reset" do `sqlx` sem explicação.
//!
//! A ponte local guarda o **agente**, não a conexão dele: cada conexão aceita
//! pede a sessão atual ao [`AgentHub`]. Se o canal cair e o agente reconectar
//! no meio de um backup, a tentativa seguinte já sai pela conexão nova.
//!
//! A mesma ponte, só de ida, traz o tar de um volume ([`download_volume`],
//! ADR 014): o Docker de todo host — o desta central inclusive — chega pelo
//! agente, e o download não pode ir em JSON.

use std::{
    future::Future,
    io,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use serde_json::Value;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf},
    net::TcpListener,
    sync::{mpsc, oneshot, Semaphore},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use agent_protocol::{
    policy::Permission,
    tunnel::{pump_raw, TunnelTotals},
    Command, ErrorCode, RemoteError, TUNNEL_DATA_MAX,
};

use super::{
    hub::AgentHub,
    session::{AgentSession, Call},
};

/// Tempo para o agente abrir a ponta dele (banco ou volume) e confirmar.
const OPEN_TIMEOUT: Duration = Duration::from_secs(20);

/// Folga para receber o `Response` final do agente depois que os bytes terminaram.
const DONE_GRACE: Duration = Duration::from_secs(5);

/// Vida máxima de uma ponte: o maior dump que se espera, com folga.
const MAX_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);

/// Conexões simultâneas que uma ponte local aceita.
const MAX_CONNECTIONS: usize = 64;

/// Uma ponte aberta no agente, pronta para receber os bytes de um socket.
pub struct OpenTunnel {
    id: Uuid,
    session: Arc<AgentSession>,
    inbound: Option<mpsc::Receiver<Vec<u8>>>,
    done: Option<JoinHandle<Result<Value, RemoteError>>>,
}

/// Por que a política anunciada recusa a ponte, com o que liberar no host.
fn refusal(agent: &str, command: &Command) -> String {
    match command.permission() {
        Permission::Database => format!(
            "O agente em {agent} não libera ponte de banco: acrescente 'database' em \
             AGENT_ALLOW e o destino em AGENT_DATABASE_TARGETS no host"
        ),
        permission => format!(
            "O agente em {agent} não libera '{permission}': acrescente a permissão em \
             AGENT_ALLOW no host"
        ),
    }
}

/// A ponta do agente, para as mensagens.
fn describe(command: &Command) -> String {
    match command {
        Command::DatabaseTunnel { host, port } => format!("{host}:{port}"),
        Command::VolumeExport { name } => format!("o volume {name}"),
        Command::ImageExport { .. } => "a imagem compilada".to_string(),
        Command::ImageImport { .. } => "o Docker do host".to_string(),
        _ => "a ponte".to_string(),
    }
}

impl OpenTunnel {
    /// Pede ao agente a ponte de `command` (`DatabaseTunnel` ou
    /// `VolumeExport`) e espera ela abrir.
    ///
    /// # Errors
    ///
    /// Agente sem a permissão do comando ou sem suporte a ponte, recusa do
    /// agente (destino fora da lista, banco inacessível, volume inexistente)
    /// ou tempo esgotado.
    pub async fn open(session: Arc<AgentSession>, command: Command) -> Result<Self, RemoteError> {
        if !session.allows(command.permission()) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                refusal(&session.name, &command),
            ));
        }
        if session.tunnel_out().is_none() {
            return Err(RemoteError::disconnected());
        }

        let target = describe(&command);
        let (chunks_tx, mut chunks) = mpsc::unbounded_channel();
        let call = Call::new(command, MAX_LIFETIME).with_chunks(chunks_tx);
        let id = call.id;
        let inbound = session
            .register_tunnel(id)
            .ok_or_else(RemoteError::disconnected)?;
        let done = {
            let session = session.clone();
            tokio::spawn(async move { session.call(call).await })
        };

        // `done` já nasce anexado a `tunnel`: qualquer cancelamento ou queda de
        // `open()` dispara `Drop`, aborta a tarefa e manda `Cancel` ao agente.
        let mut tunnel = Self {
            id,
            session,
            inbound: Some(inbound),
            done: Some(done),
        };
        let done_handle = tunnel
            .done
            .as_mut()
            .expect("handle de conclusão acabou de ser criado");
        tokio::select! {
            biased;
            opened = chunks.recv() => match opened {
                Some(_) => Ok(tunnel),
                None => {
                    let result = tunnel.done.take().expect("handle").await;
                    Err(finished_early(result))
                }
            },
            finished = done_handle => {
                // Evita que `Drop` tente cancelar um pedido já encerrado.
                tunnel.done = None;
                Err(finished_early(finished))
            }
            () = tokio::time::sleep(OPEN_TIMEOUT) => {
                if let Some(handle) = tunnel.done.take() {
                    handle.abort();
                }
                tunnel.session.cancel(id).await;
                Err(RemoteError::new(
                    ErrorCode::Timeout,
                    format!("O agente não abriu a ponte até {target} a tempo"),
                ))
            }
        }
    }

    /// A conexão do agente pela qual esta ponte foi aberta já caiu ou o pedido encerrou?
    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.session.is_closed() || self.done.as_ref().is_some_and(JoinHandle::is_finished)
    }

    /// Bombeia os bytes de `stream` até um dos lados fechar.
    ///
    /// # Errors
    ///
    /// Falha no socket local, encerramento prematuro do canal ou erro devolvido
    /// pelo agente no `Response` final.
    pub async fn run<S>(self, stream: S) -> io::Result<TunnelTotals>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        self.run_with_cancel(stream, CancellationToken::new()).await
    }

    /// Como [`Self::run`], encerrando a bomba quando `cancel` dispara.
    ///
    /// # Errors
    ///
    /// Ver [`Self::run`].
    pub async fn run_with_cancel<S>(
        mut self,
        stream: S,
        cancel: CancellationToken,
    ) -> io::Result<TunnelTotals>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let Some(outbound) = self.session.tunnel_out() else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "canal do agente caiu",
            ));
        };
        let inbound = self
            .inbound
            .take()
            .ok_or_else(|| io::Error::other("ponte já usada"))?;
        let totals = pump_raw(self.id, stream, inbound, outbound, cancel.clone()).await?;
        if !cancel.is_cancelled() {
            if let Some(done) = self.done.take() {
                match tokio::time::timeout(DONE_GRACE, done).await {
                    Ok(Ok(Ok(_))) => {}
                    Ok(Ok(Err(error))) => {
                        return Err(remote_to_io(error));
                    }
                    Ok(Err(_)) => {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "canal do agente caiu",
                        ));
                    }
                    Err(_) => {
                        // A ponte terminou em Eof mas o Response atrasou: cancela para liberar o slot.
                        self.session.cancel(self.id).await;
                    }
                }
            }
        }
        tracing::debug!(
            agente = %self.session.name,
            enviados = totals.sent,
            no_fio = totals.sent_on_wire,
            recebidos = totals.received,
            "ponte encerrada"
        );
        Ok(totals)
    }
}

fn remote_to_io(error: RemoteError) -> io::Error {
    let kind = match error.code {
        ErrorCode::Disconnected | ErrorCode::Unavailable => io::ErrorKind::BrokenPipe,
        ErrorCode::Timeout => io::ErrorKind::TimedOut,
        ErrorCode::Forbidden => io::ErrorKind::PermissionDenied,
        ErrorCode::NotFound => io::ErrorKind::NotFound,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, error.message)
}

/// Ponta local de uma ponte aberta por [`open_stream`] / [`download`].
///
/// Propaga erros da bomba (desconexão no meio da transferência) e o `RemoteError`
/// final do agente como [`io::Error`] no momento do EOF, evitando truncamento silencioso.
pub struct TunnelBridgeStream {
    inner: DuplexStream,
    done: Option<oneshot::Receiver<io::Result<()>>>,
    cancel: CancellationToken,
}

impl AsyncRead for TunnelBridgeStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {
                if buf.filled().len() > before {
                    return Poll::Ready(Ok(()));
                }
                let Some(rx) = self.done.as_mut() else {
                    return Poll::Ready(Ok(()));
                };
                match Pin::new(rx).poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Ok(Ok(()))) => {
                        self.done = None;
                        Poll::Ready(Ok(()))
                    }
                    Poll::Ready(Ok(Err(error))) => {
                        self.done = None;
                        Poll::Ready(Err(error))
                    }
                    Poll::Ready(Err(_)) => {
                        self.done = None;
                        Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "ponte interrompida inesperadamente",
                        )))
                    }
                }
            }
        }
    }
}

impl AsyncWrite for TunnelBridgeStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl Drop for TunnelBridgeStream {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Abre uma ponte de mão dupla e devolve a ponta daqui: o que se escreve vai
/// ao agente, o que se lê veio dele. Fechar a escrita manda o fim ao agente.
///
/// # Errors
///
/// Ver [`OpenTunnel::open`].
pub async fn open_stream(
    session: Arc<AgentSession>,
    command: Command,
) -> Result<TunnelBridgeStream, RemoteError> {
    let tunnel = OpenTunnel::open(session, command).await?;
    let (ours, mut pump_side) = tokio::io::duplex(TUNNEL_DATA_MAX * 2);
    let (done_tx, done_rx) = oneshot::channel();
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    tokio::spawn(async move {
        let result = tunnel.run_with_cancel(&mut pump_side, task_cancel).await;
        let outcome = match result {
            Ok(_) => Ok(()),
            Err(error) => {
                tracing::debug!(%error, "ponte interrompida");
                Err(error)
            }
        };
        let _ = done_tx.send(outcome);
        drop(pump_side);
    });
    Ok(TunnelBridgeStream {
        inner: ours,
        done: Some(done_rx),
        cancel,
    })
}

/// Só de ida: o que o agente manda, até o fim (volume, artefato).
///
/// # Errors
///
/// Ver [`OpenTunnel::open`].
pub async fn download(
    session: Arc<AgentSession>,
    command: Command,
) -> Result<TunnelBridgeStream, RemoteError> {
    let mut reader = open_stream(session, command).await?;
    reader
        .shutdown()
        .await
        .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?;
    Ok(reader)
}

/// O tar de um volume (ADR 014).
///
/// # Errors
///
/// Ver [`OpenTunnel::open`].
pub async fn download_volume(
    session: Arc<AgentSession>,
    name: &str,
) -> Result<TunnelBridgeStream, RemoteError> {
    download(
        session,
        Command::VolumeExport {
            name: name.to_string(),
        },
    )
    .await
}

/// A ponte sai do agente quando sai daqui: fechar a rota e cancelar o pedido
/// é o que libera o socket e a vaga do agente (no máximo 2 pontes).
impl Drop for OpenTunnel {
    fn drop(&mut self) {
        self.session.unregister_tunnel(self.id);
        if let Some(done) = self.done.take() {
            let still_open = !done.is_finished();
            if still_open {
                done.abort();
                let session = self.session.clone();
                let id = self.id;
                tokio::spawn(async move { session.cancel(id).await });
            }
        }
    }
}

/// A resposta chegou antes do "aberta": é o erro do agente (ou a queda).
fn finished_early(
    result: Result<Result<Value, RemoteError>, tokio::task::JoinError>,
) -> RemoteError {
    match result {
        Ok(Err(error)) => error,
        Ok(Ok(_)) => RemoteError::new(ErrorCode::Internal, "O agente fechou a ponte sem abri-la"),
        Err(_) => RemoteError::disconnected(),
    }
}

/// Endereço local que leva ao banco pelo agente, enquanto esta struct viver.
pub struct LocalBridge {
    addr: SocketAddr,
    cancel: CancellationToken,
    task: JoinHandle<()>,
}

impl LocalBridge {
    /// Abre a primeira ponte e o listener.
    ///
    /// # Errors
    ///
    /// Agente desconectado; ver [`OpenTunnel::open`]; ou falha ao escutar em
    /// `127.0.0.1`.
    pub async fn start(
        hub: AgentHub,
        probe_id: i64,
        host: String,
        port: u16,
    ) -> Result<Self, RemoteError> {
        let first = open_current(&hub, probe_id, &host, port).await?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?;
        let addr = listener
            .local_addr()
            .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?;

        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();
        let limiter = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let task = tokio::spawn(async move {
            let mut first = Some(first);
            loop {
                let accepted = tokio::select! {
                    biased;
                    () = task_cancel.cancelled() => break,
                    res = listener.accept() => res,
                };
                let Ok((stream, _)) = accepted else {
                    break;
                };
                let Ok(permit) = limiter.clone().try_acquire_owned() else {
                    tracing::warn!("limite de conexões simultâneas da ponte de banco atingido");
                    continue;
                };
                let preopened = first.take().filter(|tunnel| !tunnel.is_stale());
                let hub = hub.clone();
                let host = host.clone();
                let conn_cancel = task_cancel.child_token();
                tokio::spawn(async move {
                    let _permit = permit;
                    let tunnel = match preopened {
                        Some(tunnel) => Ok(tunnel),
                        None => open_current(&hub, probe_id, &host, port).await,
                    };
                    match tunnel {
                        Ok(tunnel) => {
                            let _ = stream.set_nodelay(true);
                            if let Err(error) = tunnel.run_with_cancel(stream, conn_cancel).await {
                                tracing::debug!(%error, "ponte de banco encerrada com erro");
                            }
                        }
                        // A conexão cai e o `sqlx` reporta; o motivo vai para o log.
                        Err(error) => {
                            tracing::warn!(%error, "ponte de banco recusada pelo agente");
                        }
                    }
                });
            }
        });
        Ok(Self { addr, cancel, task })
    }

    #[must_use]
    pub const fn addr(&self) -> SocketAddr {
        self.addr
    }
}

/// Abre uma ponte pela conexão **atual** do agente.
async fn open_current(
    hub: &AgentHub,
    probe_id: i64,
    host: &str,
    port: u16,
) -> Result<OpenTunnel, RemoteError> {
    let session = hub.get(probe_id).ok_or_else(RemoteError::disconnected)?;
    OpenTunnel::open(
        session,
        Command::DatabaseTunnel {
            host: host.to_string(),
            port,
        },
    )
    .await
}

impl Drop for LocalBridge {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::io::AsyncReadExt;

    use super::*;
    use crate::agents::session::testing::hello;
    use agent_protocol::{Body, Envelope, Outcome, TunnelFrame, TunnelPayload, WireFrame};

    #[tokio::test]
    async fn download_propaga_erro_quando_canal_cai_no_meio() {
        let (env_tx, mut env_rx) = mpsc::channel::<Envelope>(8);
        let (tun_tx, _tun_rx) = mpsc::channel::<WireFrame>(8);
        let session = Arc::new(
            AgentSession::new(1, "srv", hello(vec![Permission::VolumeExport]), env_tx)
                .with_tunnel_out(tun_tx),
        );
        let (ready_tx, ready_rx) = oneshot::channel::<()>();

        let agent = {
            let session = session.clone();
            tokio::spawn(async move {
                let req = env_rx.recv().await.expect("pedido");
                session.handle(Envelope::reply(
                    req.id,
                    Body::Chunk {
                        data: json!({"opened": true}),
                    },
                ));
                let _ = ready_rx.await;
                session.handle_tunnel_frame_owned(TunnelFrame::encode_raw(req.id, b"parte-1"));
                // Canal cai sem mandar Eof nem Response.
                session.close();
            })
        };

        let mut reader = download_volume(session, "dados").await.expect("abriu");
        let _ = ready_tx.send(());
        let mut buf = Vec::new();
        let err = reader
            .read_to_end(&mut buf)
            .await
            .expect_err("queda no meio deve retornar erro de E/S, não EOF limpo");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(buf, b"parte-1");
        agent.await.unwrap();
    }

    #[tokio::test]
    async fn download_propaga_remote_error_apos_abertura() {
        let (env_tx, mut env_rx) = mpsc::channel::<Envelope>(8);
        let (tun_tx, _tun_rx) = mpsc::channel::<WireFrame>(8);
        let session = Arc::new(
            AgentSession::new(1, "srv", hello(vec![Permission::VolumeExport]), env_tx)
                .with_tunnel_out(tun_tx),
        );

        let agent = {
            let session = session.clone();
            tokio::spawn(async move {
                let req = env_rx.recv().await.expect("pedido");
                session.handle(Envelope::reply(
                    req.id,
                    Body::Chunk {
                        data: json!({"opened": true}),
                    },
                ));
                session.handle_tunnel_frame_owned(TunnelFrame::encode_raw(req.id, b"dados"));
                session.handle_tunnel_frame_owned(
                    TunnelFrame {
                        id: req.id,
                        payload: TunnelPayload::Eof,
                    }
                    .encode(),
                );
                session.handle(Envelope::reply(
                    req.id,
                    Body::Response {
                        outcome: Outcome::Err {
                            error: RemoteError::new(ErrorCode::Engine, "falha no tar"),
                        },
                    },
                ));
            })
        };

        let mut reader = download_volume(session, "dados").await.expect("abriu");
        let mut buf = Vec::new();
        let err = reader
            .read_to_end(&mut buf)
            .await
            .expect_err("erro no Response precisa chegar ao leitor");
        assert!(err.to_string().contains("falha no tar"), "{err}");
        agent.await.unwrap();
    }
}
