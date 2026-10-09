//! Uma conexão viva de agente, vista pela central.
//!
//! A sessão não conhece WebSocket: recebe [`Envelope`]s já decodificados em
//! [`AgentSession::handle`] e escreve os de saída num canal. Isso permite
//! testar correlação, timeout, cancelamento e queda do canal sem rede — e é
//! o mesmo código que o controller liga ao socket real.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::{
    policy::Permission,
    protocol::{
        AgentEvent, Body, Command, Envelope, ErrorCode, Hello, RemoteError, TunnelFrame,
        TunnelPayload, WireFrame,
    },
    tunnel::INBOUND_CAPACITY,
};

/// Um pedido à espera de resposta.
struct Pending {
    chunks: Option<mpsc::UnboundedSender<Value>>,
    reply: oneshot::Sender<Result<Value, RemoteError>>,
}

/// Parâmetros de um pedido. O `id` é público para quem precisa cancelar um
/// stream (logs em follow) depois de iniciá-lo.
pub struct Call {
    pub id: Uuid,
    pub command: Command,
    pub timeout: Duration,
    pub chunks: Option<mpsc::UnboundedSender<Value>>,
}

impl Call {
    #[must_use]
    pub fn new(command: Command, timeout: Duration) -> Self {
        Self {
            id: Uuid::new_v4(),
            command,
            timeout,
            chunks: None,
        }
    }

    #[must_use]
    pub fn with_chunks(mut self, chunks: mpsc::UnboundedSender<Value>) -> Self {
        self.chunks = Some(chunks);
        self
    }
}

pub struct AgentSession {
    pub probe_id: i64,
    pub name: String,
    pub hello: Hello,
    pub connected_at: DateTime<Utc>,
    outbound: mpsc::Sender<Envelope>,
    /// Fila dos quadros binários de ponte — separada e de menor prioridade
    /// que a de controle, para um dump não atrasar monitores e Docker.
    tunnel_out: Option<mpsc::Sender<WireFrame>>,
    /// Pontes abertas: para onde vão os quadros binários de cada uma.
    tunnels: Mutex<HashMap<Uuid, mpsc::Sender<TunnelPayload>>>,
    pending: Mutex<HashMap<Uuid, Pending>>,
    closed: AtomicBool,
}

impl AgentSession {
    #[must_use]
    pub fn new(
        probe_id: i64,
        name: impl Into<String>,
        hello: Hello,
        outbound: mpsc::Sender<Envelope>,
    ) -> Self {
        Self {
            probe_id,
            name: name.into(),
            hello,
            connected_at: Utc::now(),
            outbound,
            tunnel_out: None,
            tunnels: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
        }
    }

    /// Liga a fila de quadros binários (sem ela, a sessão não abre pontes).
    #[must_use]
    pub fn with_tunnel_out(mut self, tunnel_out: mpsc::Sender<WireFrame>) -> Self {
        self.tunnel_out = Some(tunnel_out);
        self
    }

    /// Fila de saída das pontes, se o canal tiver uma.
    #[must_use]
    pub fn tunnel_out(&self) -> Option<mpsc::Sender<WireFrame>> {
        self.tunnel_out.clone()
    }

    /// Reserva a rota dos quadros que chegarem para a ponte `id`.
    pub fn register_tunnel(&self, id: Uuid) -> mpsc::Receiver<TunnelPayload> {
        let (tx, rx) = mpsc::channel(INBOUND_CAPACITY);
        self.lock_tunnels().insert(id, tx);
        rx
    }

    pub fn unregister_tunnel(&self, id: Uuid) {
        self.lock_tunnels().remove(&id);
    }

    /// Entrega um quadro binário à ponte dele. Quadro inválido, de ponte
    /// desconhecida ou que estoura a janela é descartado — no último caso a
    /// ponte cai, porque o outro lado violou o crédito.
    pub fn handle_tunnel_frame(&self, bytes: &[u8]) {
        let frame = match TunnelFrame::decode(bytes) {
            Ok(frame) => frame,
            Err(error) => {
                tracing::warn!(%error, agent = %self.name, "quadro de ponte inválido");
                return;
            }
        };
        let mut tunnels = self.lock_tunnels();
        let Some(route) = tunnels.get(&frame.id) else {
            return;
        };
        if route.try_send(frame.payload).is_err() {
            tracing::warn!(agent = %self.name, "ponte excedeu a janela de crédito; encerrada");
            tunnels.remove(&frame.id);
        }
    }

    fn lock_tunnels(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<Uuid, mpsc::Sender<TunnelPayload>>> {
        self.tunnels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// O que a política local anunciou. A decisão final é do agente; isto só
    /// evita mandar pedidos que seriam recusados.
    #[must_use]
    pub fn allows(&self, permission: Permission) -> bool {
        self.hello.policy.contains(&permission)
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Pedido com resposta única.
    ///
    /// # Errors
    ///
    /// `Forbidden` pela política anunciada, `Timeout`, `Disconnected` ou o
    /// erro que o agente devolveu.
    pub async fn request(&self, command: Command, timeout: Duration) -> Result<Value, RemoteError> {
        self.call(Call::new(command, timeout)).await
    }

    /// Pedido completo: chunks intermediários vão para `call.chunks`.
    ///
    /// # Errors
    ///
    /// Ver [`Self::request`].
    pub async fn call(&self, call: Call) -> Result<Value, RemoteError> {
        let permission = call.command.permission();
        if !self.allows(permission) {
            return Err(RemoteError::new(
                ErrorCode::Forbidden,
                format!(
                    "A política do agente em {} não libera a permissão '{permission}'",
                    self.name
                ),
            ));
        }
        if self.is_closed() {
            return Err(RemoteError::disconnected());
        }
        let (reply, response) = oneshot::channel();
        self.lock_pending().insert(
            call.id,
            Pending {
                chunks: call.chunks,
                reply,
            },
        );
        let deadline_ms = u64::try_from(call.timeout.as_millis()).unwrap_or(u64::MAX);
        let request = Envelope::reply(
            call.id,
            Body::Request {
                deadline_ms,
                command: call.command,
            },
        );
        if self.outbound.send(request).await.is_err() {
            self.lock_pending().remove(&call.id);
            return Err(RemoteError::disconnected());
        }
        match tokio::time::timeout(call.timeout, response).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(RemoteError::disconnected()),
            Err(_) => {
                self.cancel(call.id).await;
                Err(RemoteError::timeout())
            }
        }
    }

    /// Interrompe um pedido em andamento. A espera local é resolvida na hora;
    /// o agente recebe o `Cancel` e para o que estiver fazendo.
    pub async fn cancel(&self, id: Uuid) {
        if let Some(pending) = self.lock_pending().remove(&id) {
            let _ = pending.reply.send(Err(RemoteError::new(
                ErrorCode::Timeout,
                "Pedido cancelado",
            )));
        }
        let _ = self.outbound.send(Envelope::reply(id, Body::Cancel)).await;
    }

    /// Entrega o quadro recebido a quem o espera. Devolve os eventos
    /// espontâneos para o chamador tratar.
    pub fn handle(&self, envelope: Envelope) -> Option<AgentEvent> {
        match envelope.body {
            Body::Chunk { data } => {
                if let Some(chunks) = self
                    .lock_pending()
                    .get(&envelope.id)
                    .and_then(|pending| pending.chunks.as_ref())
                {
                    let _ = chunks.send(data);
                }
                None
            }
            Body::Response { outcome } => {
                if let Some(pending) = self.lock_pending().remove(&envelope.id) {
                    let _ = pending.reply.send(outcome.into());
                }
                None
            }
            Body::Event { event } => Some(event),
            // `Hello` só vale como primeiro quadro; o resto não é do agente.
            Body::Hello(_) | Body::Welcome(_) | Body::Request { .. } | Body::Cancel => None,
        }
    }

    /// Canal caiu: todo pedido pendente termina com `Disconnected`.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        // Sem rota, as bombas das pontes veem o fim e encerram.
        self.lock_tunnels().clear();
        for (_, pending) in self.lock_pending().drain() {
            let _ = pending.reply.send(Err(RemoteError::disconnected()));
        }
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.lock_pending().len()
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, Pending>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::json;

    use super::*;
    use crate::services::agents::protocol::{Capability, DockerCall, Outcome, PROTOCOL_VERSION};

    pub(crate) fn hello(policy: Vec<Permission>) -> Hello {
        Hello {
            protocol: PROTOCOL_VERSION,
            agent_version: "1.0.0".into(),
            hostname: "srv".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            in_container: false,
            policy,
            docker: Capability {
                available: true,
                version: Some("27".into()),
                reason: None,
            },
            compose: Capability::default(),
        }
    }

    fn session(policy: Vec<Permission>) -> (AgentSession, mpsc::Receiver<Envelope>) {
        let (tx, rx) = mpsc::channel(8);
        (AgentSession::new(7, "srv", hello(policy), tx), rx)
    }

    fn list() -> Command {
        Command::Docker {
            call: DockerCall::ListContainers,
        }
    }

    #[tokio::test]
    async fn resposta_volta_para_quem_perguntou_com_os_chunks() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let session = std::sync::Arc::new(session);
        let (chunks_tx, mut chunks_rx) = mpsc::unbounded_channel();
        let caller = {
            let session = session.clone();
            tokio::spawn(async move {
                session
                    .call(Call::new(list(), Duration::from_secs(5)).with_chunks(chunks_tx))
                    .await
            })
        };
        let request = outbound.recv().await.expect("pedido enviado");
        assert!(matches!(request.body, Body::Request { .. }));
        assert!(session
            .handle(Envelope::reply(
                request.id,
                Body::Chunk {
                    data: json!("parcial")
                }
            ))
            .is_none());
        session.handle(Envelope::reply(
            request.id,
            Body::Response {
                outcome: Outcome::Ok { value: json!([1]) },
            },
        ));
        assert_eq!(caller.await.expect("tarefa").expect("ok"), json!([1]));
        assert_eq!(chunks_rx.recv().await, Some(json!("parcial")));
        assert_eq!(session.pending_count(), 0);
    }

    #[tokio::test]
    async fn politica_anunciada_barra_antes_de_enviar() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let error = session
            .request(
                Command::Docker {
                    call: DockerCall::UpdateContainer { id: "web".into() },
                },
                Duration::from_secs(1),
            )
            .await
            .expect_err("negado");
        assert_eq!(error.code, ErrorCode::Forbidden);
        assert!(outbound.try_recv().is_err(), "nada foi enviado");
    }

    #[tokio::test]
    async fn timeout_cancela_no_agente() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let error = session
            .request(list(), Duration::from_millis(30))
            .await
            .expect_err("timeout");
        assert_eq!(error.code, ErrorCode::Timeout);
        let request = outbound.recv().await.expect("pedido");
        let cancel = outbound.recv().await.expect("cancelamento");
        assert_eq!(cancel.id, request.id);
        assert!(matches!(cancel.body, Body::Cancel));
        assert_eq!(session.pending_count(), 0);
    }

    #[tokio::test]
    async fn queda_do_canal_resolve_os_pendentes() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let session = std::sync::Arc::new(session);
        let caller = {
            let session = session.clone();
            tokio::spawn(async move { session.request(list(), Duration::from_secs(5)).await })
        };
        outbound.recv().await.expect("pedido");
        session.close();
        let error = caller.await.expect("tarefa").expect_err("desconectado");
        assert_eq!(error.code, ErrorCode::Disconnected);
        let again = session
            .request(list(), Duration::from_secs(1))
            .await
            .expect_err("fechado");
        assert_eq!(again.code, ErrorCode::Disconnected);
    }
}
