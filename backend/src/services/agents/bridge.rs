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
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use serde_json::Value;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, DuplexStream},
    net::TcpListener,
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    hub::AgentHub,
    policy::Permission,
    protocol::{Command, ErrorCode, RemoteError, TunnelPayload, TUNNEL_DATA_MAX},
    session::{AgentSession, Call},
    tunnel::{pump, TunnelTotals},
};

/// Tempo para o agente abrir a ponta dele (banco ou volume) e confirmar.
const OPEN_TIMEOUT: Duration = Duration::from_secs(20);

/// Vida máxima de uma ponte: o maior dump que se espera, com folga.
const MAX_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);

/// Conexões que uma ponte local aceita. Um backup usa uma por banco; a
/// restauração em banco novo, duas.
const MAX_CONNECTIONS: usize = 64;

/// Uma ponte aberta no agente, pronta para receber os bytes de um socket.
pub struct OpenTunnel {
    id: Uuid,
    session: Arc<AgentSession>,
    inbound: Option<mpsc::Receiver<TunnelPayload>>,
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
        let inbound = session.register_tunnel(id);
        let mut done = {
            let session = session.clone();
            tokio::spawn(async move { session.call(call).await })
        };

        let mut tunnel = Self {
            id,
            session,
            inbound: Some(inbound),
            done: None,
        };
        tokio::select! {
            opened = chunks.recv() => match opened {
                Some(_) => {
                    tunnel.done = Some(done);
                    Ok(tunnel)
                }
                None => Err(finished_early(done.await)),
            },
            finished = &mut done => Err(finished_early(finished)),
            () = tokio::time::sleep(OPEN_TIMEOUT) => {
                done.abort();
                Err(RemoteError::new(
                    ErrorCode::Timeout,
                    format!("O agente não abriu a ponte até {target} a tempo"),
                ))
            }
        }
    }

    /// A conexão do agente pela qual esta ponte foi aberta já caiu?
    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.session.is_closed()
    }

    /// Bombeia os bytes de `stream` até um dos lados fechar.
    ///
    /// # Errors
    ///
    /// Falha no socket local.
    pub async fn run<S>(mut self, stream: S) -> std::io::Result<TunnelTotals>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let Some(outbound) = self.session.tunnel_out() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "canal do agente caiu",
            ));
        };
        let inbound = self
            .inbound
            .take()
            .ok_or_else(|| std::io::Error::other("ponte já usada"))?;
        let totals = pump(self.id, stream, inbound, outbound, CancellationToken::new()).await?;
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

/// O tar do volume `name` do host do agente, como um leitor (ADR 014).
///
/// A central não manda nada pela ponte: fecha a escrita do seu lado na hora
/// e lê até o agente fechar o dele. Soltar o leitor antes do fim (o download
/// foi cancelado) derruba a bomba, e a queda cancela a ponte no agente — que
/// então remove o container auxiliar da exportação.
///
/// # Errors
///
/// Ver [`OpenTunnel::open`].
pub async fn download_volume(
    session: Arc<AgentSession>,
    name: &str,
) -> Result<DuplexStream, RemoteError> {
    let tunnel = OpenTunnel::open(
        session,
        Command::VolumeExport {
            name: name.to_string(),
        },
    )
    .await?;
    let (mut reader, pump_side) = tokio::io::duplex(TUNNEL_DATA_MAX * 2);
    reader
        .shutdown()
        .await
        .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))?;
    tokio::spawn(async move {
        if let Err(error) = tunnel.run(pump_side).await {
            tracing::debug!(%error, "download de volume interrompido");
        }
    });
    Ok(reader)
}

/// A ponte sai do agente quando sai daqui: fechar a rota e cancelar o pedido
/// é o que libera o socket e a vaga do agente (no máximo 2 pontes).
impl Drop for OpenTunnel {
    fn drop(&mut self) {
        self.session.unregister_tunnel(self.id);
        let still_open = self.done.as_ref().is_some_and(|done| !done.is_finished());
        if still_open {
            let session = self.session.clone();
            let id = self.id;
            tokio::spawn(async move { session.cancel(id).await });
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

        let task = tokio::spawn(async move {
            let mut first = Some(first);
            for _ in 0..MAX_CONNECTIONS {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                // A primeira ponte vale se a conexão do agente ainda é a mesma.
                let tunnel = match first.take().filter(|tunnel| !tunnel.is_stale()) {
                    Some(tunnel) => Ok(tunnel),
                    None => open_current(&hub, probe_id, &host, port).await,
                };
                match tunnel {
                    Ok(tunnel) => {
                        let _ = stream.set_nodelay(true);
                        tokio::spawn(async move {
                            if let Err(error) = tunnel.run(stream).await {
                                tracing::debug!(%error, "ponte de banco encerrada com erro");
                            }
                        });
                    }
                    // A conexão cai e o `sqlx` reporta; o motivo vai para o log.
                    Err(error) => tracing::warn!(%error, "ponte de banco recusada pelo agente"),
                }
            }
        });
        Ok(Self { addr, task })
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
        self.task.abort();
    }
}
