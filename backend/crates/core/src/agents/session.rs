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
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use agent_protocol::{
    policy::Permission, tunnel::INBOUND_CAPACITY, AgentEvent, Body, Command, Envelope, ErrorCode,
    Hello, RemoteError, TunnelFrame, WireFrame,
};

/// Pedido que levou isto ou mais aparece no log da aplicação.
const SLOW_REQUEST: Duration = Duration::from_secs(5);

/// Limite para despachar `Body::Cancel` quando a fila de saída está cheia.
const CANCEL_SEND_TIMEOUT: Duration = Duration::from_secs(1);

const fn permission_bit(permission: Permission) -> u16 {
    match permission {
        Permission::Read => 1 << 0,
        Permission::Lifecycle => 1 << 1,
        Permission::Update => 1 << 2,
        Permission::Compose => 1 << 3,
        Permission::Monitor => 1 << 4,
        Permission::Discovery => 1 << 5,
        Permission::DeviceIo => 1 << 6,
        Permission::Database => 1 << 7,
        Permission::VolumeExport => 1 << 8,
        Permission::Throughput => 1 << 9,
        Permission::Deploy => 1 << 10,
        Permission::Build => 1 << 11,
    }
}

fn policy_mask(permissions: &[Permission]) -> u16 {
    permissions
        .iter()
        .fold(0_u16, |mask, &permission| mask | permission_bit(permission))
}

/// Remove um pedido pendente ao ser descartado caso o futuro seja cancelado no
/// meio — e avisa o agente com um `Cancel`, para ele não seguir executando o
/// que ninguém mais espera (operação ou deploy cuja tarefa foi abortada).
struct PendingGuard<'a> {
    session: &'a AgentSession,
    id: Uuid,
    armed: bool,
}

impl<'a> PendingGuard<'a> {
    const fn new(session: &'a AgentSession, id: Uuid) -> Self {
        Self {
            session,
            id,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let removed = self
            .session
            .lock_pending()
            .as_mut()
            .and_then(|pending| pending.remove(&self.id))
            .is_some();
        if removed && !self.session.is_closed() {
            // `Drop` não espera: com a fila cheia o aviso se perde, e o prazo
            // do próprio pedido encerra a execução no agente.
            let _ = self
                .session
                .outbound
                .try_send(Envelope::reply(self.id, Body::Cancel));
        }
    }
}

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
    policy_mask: u16,
    outbound: mpsc::Sender<Envelope>,
    /// Fila dos quadros binários de ponte — separada e de menor prioridade
    /// que a de controle, para um dump não atrasar monitores e Docker.
    tunnel_out: Option<mpsc::Sender<WireFrame>>,
    /// Pontes abertas: `None` após [`Self::close`], impedindo inserções pós-fechamento.
    tunnels: Mutex<Option<HashMap<Uuid, mpsc::Sender<Vec<u8>>>>>,
    /// Pedidos pendentes: `None` após [`Self::close`], impedindo inserções pós-fechamento.
    pending: Mutex<Option<HashMap<Uuid, Pending>>>,
    closed: AtomicBool,
    close_token: CancellationToken,
}

impl AgentSession {
    #[must_use]
    pub fn new(
        probe_id: i64,
        name: impl Into<String>,
        hello: Hello,
        outbound: mpsc::Sender<Envelope>,
    ) -> Self {
        let policy_mask = policy_mask(&hello.policy);
        Self {
            probe_id,
            name: name.into(),
            hello,
            connected_at: Utc::now(),
            policy_mask,
            outbound,
            tunnel_out: None,
            tunnels: Mutex::new(Some(HashMap::new())),
            pending: Mutex::new(Some(HashMap::new())),
            closed: AtomicBool::new(false),
            close_token: CancellationToken::new(),
        }
    }

    /// Token cancelado quando [`Self::close`] encerra a sessão (revogação ou substituição).
    #[must_use]
    pub fn close_token(&self) -> CancellationToken {
        self.close_token.clone()
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

    /// Reserva a rota dos quadros binários crus que chegarem para a ponte `id`.
    /// Devolve `None` se a sessão já foi fechada.
    #[must_use]
    pub fn register_tunnel(&self, id: Uuid) -> Option<mpsc::Receiver<Vec<u8>>> {
        let mut tunnels = self.lock_tunnels();
        let map = tunnels.as_mut()?;
        let (tx, rx) = mpsc::channel(INBOUND_CAPACITY);
        map.insert(id, tx);
        Some(rx)
    }

    pub fn unregister_tunnel(&self, id: Uuid) {
        if let Some(tunnels) = self.lock_tunnels().as_mut() {
            tunnels.remove(&id);
        }
    }

    /// Entrega um quadro binário cru à ponte dele sem descomprimir no laço do
    /// WebSocket. Quadro sem id válido, de ponte desconhecida ou que estoura a
    /// janela é descartado — no último caso a ponte cai e o agente recebe `Cancel`.
    pub fn handle_tunnel_frame(&self, bytes: &[u8]) {
        self.handle_tunnel_frame_owned(bytes.to_vec());
    }

    /// Versão zero-copy de [`Self::handle_tunnel_frame`] quando o chamador já
    /// possui o `Vec<u8>` do quadro WebSocket.
    pub fn handle_tunnel_frame_owned(&self, bytes: Vec<u8>) {
        let Some(id) = TunnelFrame::peek_id(&bytes) else {
            tracing::warn!(agent = %self.name, "quadro de ponte inválido (curto demais)");
            return;
        };
        let mut tunnels = self.lock_tunnels();
        let Some(map) = tunnels.as_mut() else {
            return;
        };
        let Some(route) = map.get(&id) else {
            return;
        };
        match route.try_send(bytes) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(agent = %self.name, "ponte excedeu a janela de crédito; encerrada");
                map.remove(&id);
                drop(tunnels);
                let _ = self.outbound.try_send(Envelope::reply(id, Body::Cancel));
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                map.remove(&id);
            }
        }
    }

    fn lock_tunnels(
        &self,
    ) -> std::sync::MutexGuard<'_, Option<HashMap<Uuid, mpsc::Sender<Vec<u8>>>>> {
        self.tunnels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// O que a política local anunciou. A decisão final é do agente; isto só
    /// evita mandar pedidos que seriam recusados.
    #[must_use]
    pub fn allows(&self, permission: Permission) -> bool {
        (self.policy_mask & permission_bit(permission)) != 0
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
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
        let label = call.command.label();
        let started = std::time::Instant::now();
        let timeout = call.timeout;
        let result = self.send_and_wait(call).await;
        self.log_outcome(label, started.elapsed(), timeout, &result);
        result
    }

    /// O que o operador precisa achar no log quando a tela falha: qual agente,
    /// qual pedido, quanto tempo. Erro de negócio (não encontrado, conflito)
    /// e cancelamento explícito não entram — não são falhas do canal.
    fn log_outcome(
        &self,
        label: &str,
        elapsed: Duration,
        timeout: Duration,
        result: &Result<Value, RemoteError>,
    ) {
        let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        match result {
            Err(error) if *error == RemoteError::cancelled() => {}
            Err(error) if matches!(error.code, ErrorCode::Timeout) => tracing::warn!(
                agent = %self.name,
                command = label,
                elapsed_ms,
                timeout_s = timeout.as_secs(),
                "o agente não respondeu ao pedido a tempo"
            ),
            Err(error) if matches!(error.code, ErrorCode::Disconnected) => tracing::warn!(
                agent = %self.name,
                command = label,
                elapsed_ms,
                "o canal com o agente caiu durante o pedido"
            ),
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::Unavailable | ErrorCode::Engine | ErrorCode::Internal
                ) =>
            {
                tracing::warn!(
                    agent = %self.name,
                    command = label,
                    elapsed_ms,
                    error = %error.message,
                    "o agente não conseguiu executar o pedido"
                );
            }
            Ok(_) if elapsed >= SLOW_REQUEST => tracing::info!(
                agent = %self.name,
                command = label,
                elapsed_ms,
                "pedido ao agente demorou"
            ),
            _ => {}
        }
    }

    async fn send_and_wait(&self, call: Call) -> Result<Value, RemoteError> {
        let (reply, response) = oneshot::channel();
        {
            let mut pending = self.lock_pending();
            let Some(map) = pending.as_mut() else {
                return Err(RemoteError::disconnected());
            };
            map.insert(
                call.id,
                Pending {
                    chunks: call.chunks,
                    reply,
                },
            );
        }
        let mut guard = PendingGuard::new(self, call.id);
        let deadline_ms = u64::try_from(call.timeout.as_millis()).unwrap_or(u64::MAX);
        let request = Envelope::reply(
            call.id,
            Body::Request {
                deadline_ms,
                command: call.command,
            },
        );
        if self.outbound.send(request).await.is_err() {
            return Err(RemoteError::disconnected());
        }
        let outcome = match tokio::time::timeout(call.timeout, response).await {
            Ok(Ok(result)) => {
                guard.disarm();
                result
            }
            Ok(Err(_)) => {
                guard.disarm();
                Err(RemoteError::disconnected())
            }
            Err(_) => {
                guard.disarm();
                self.cancel(call.id).await;
                Err(RemoteError::timeout())
            }
        };
        outcome
    }

    /// Pedido cujos chunks vão a `on_chunk`, na ordem, no próprio laço de
    /// quem espera — sem tarefa à parte, então `on_chunk` pode emprestar
    /// referências. Todo chunk é entregue antes do resultado. `cancel`
    /// interrompe: o agente recebe o `Cancel` e o erro é
    /// [`RemoteError::cancelled`].
    ///
    /// # Errors
    ///
    /// Ver [`Self::request`].
    pub async fn stream(
        &self,
        command: Command,
        timeout: Duration,
        cancel: &CancellationToken,
        mut on_chunk: impl FnMut(Value) + Send,
    ) -> Result<Value, RemoteError> {
        if cancel.is_cancelled() {
            return Err(RemoteError::cancelled());
        }
        let (chunks, mut received) = mpsc::unbounded_channel();
        let call = Call::new(command, timeout).with_chunks(chunks);
        let id = call.id;
        let reply = self.call(call);
        tokio::pin!(reply);
        let result = loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => {
                    self.cancel(id).await;
                    return Err(RemoteError::cancelled());
                }
                Some(chunk) = received.recv() => on_chunk(chunk),
                result = &mut reply => break result,
            }
        };
        // A resposta chega depois dos chunks, mas pode vencer a corrida do
        // `select!`: o que ficou no canal ainda é deste pedido.
        while let Ok(chunk) = received.try_recv() {
            on_chunk(chunk);
        }
        result
    }

    /// Interrompe um pedido em andamento. A espera local é resolvida na hora;
    /// o agente recebe o `Cancel` em melhor esforço sem bloquear indefinidamente.
    pub async fn cancel(&self, id: Uuid) {
        if let Some(pending) = self.lock_pending().as_mut().and_then(|map| map.remove(&id)) {
            let _ = pending.reply.send(Err(RemoteError::cancelled()));
        }
        if self.is_closed() {
            return;
        }
        let envelope = Envelope::reply(id, Body::Cancel);
        match self.outbound.try_send(envelope) {
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(envelope)) => {
                let _ =
                    tokio::time::timeout(CANCEL_SEND_TIMEOUT, self.outbound.send(envelope)).await;
            }
        }
    }

    /// Entrega o quadro recebido a quem o espera. Devolve os eventos
    /// espontâneos para o chamador tratar.
    pub fn handle(&self, envelope: Envelope) -> Option<AgentEvent> {
        match envelope.body {
            Body::Chunk { data } => {
                if let Some(chunks) = self
                    .lock_pending()
                    .as_ref()
                    .and_then(|map| map.get(&envelope.id))
                    .and_then(|pending| pending.chunks.as_ref())
                {
                    let _ = chunks.send(data);
                }
                None
            }
            Body::Response { outcome } => {
                if let Some(pending) = self
                    .lock_pending()
                    .as_mut()
                    .and_then(|map| map.remove(&envelope.id))
                {
                    let _ = pending.reply.send(outcome.into());
                }
                None
            }
            Body::Event { event } => Some(event),
            // `Hello` só vale como primeiro quadro; o resto não é do agente.
            Body::Hello(_) | Body::Welcome(_) | Body::Request { .. } | Body::Cancel => None,
        }
    }

    /// Canal caiu: todo pedido pendente termina com `Disconnected` e nenhuma
    /// nova chamada ou ponte consegue se registrar nesta sessão.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.close_token.cancel();
        // Sem rota, as bombas das pontes veem o fim e encerram.
        let _ = self.lock_tunnels().take();
        if let Some(mut pending) = self.lock_pending().take() {
            for (_, entry) in pending.drain() {
                let _ = entry.reply.send(Err(RemoteError::disconnected()));
            }
        }
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.lock_pending().as_ref().map_or(0, HashMap::len)
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, Option<HashMap<Uuid, Pending>>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Apoio aos testes das áreas que falam com um agente de mentira.
#[doc(hidden)]
pub mod testing {
    use agent_protocol::{policy::Permission, Capability, Hello, PROTOCOL_VERSION};

    #[must_use]
    pub fn hello(policy: Vec<Permission>) -> Hello {
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
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{testing::hello, *};
    use agent_protocol::{DockerCall, Outcome};

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
    async fn stream_entrega_os_chunks_antes_do_resultado() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let session = std::sync::Arc::new(session);
        let agent = {
            let session = session.clone();
            tokio::spawn(async move {
                let request = outbound.recv().await.expect("pedido");
                for n in 1..=3 {
                    session.handle(Envelope::reply(request.id, Body::Chunk { data: json!(n) }));
                }
                session.handle(Envelope::reply(
                    request.id,
                    Body::Response {
                        outcome: Outcome::Ok {
                            value: json!("fim"),
                        },
                    },
                ));
            })
        };
        let mut seen = Vec::new();
        let value = session
            .stream(
                list(),
                Duration::from_secs(5),
                &CancellationToken::new(),
                |chunk| seen.push(chunk),
            )
            .await
            .expect("ok");
        agent.await.expect("agente");
        assert_eq!(value, json!("fim"));
        assert_eq!(seen, vec![json!(1), json!(2), json!(3)]);
    }

    #[tokio::test]
    async fn stream_cancelado_avisa_o_agente() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let cancel = CancellationToken::new();
        let stopper = {
            let cancel = cancel.clone();
            tokio::spawn(async move {
                let request = outbound.recv().await.expect("pedido");
                cancel.cancel();
                let next = outbound.recv().await.expect("cancelamento");
                (request.id, next)
            })
        };
        let error = session
            .stream(list(), Duration::from_secs(5), &cancel, |_| {})
            .await
            .expect_err("cancelado");
        assert_eq!(error, RemoteError::cancelled());
        let (id, next) = stopper.await.expect("tarefa");
        assert_eq!(next.id, id);
        assert!(matches!(next.body, Body::Cancel));
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
        assert!(session.close_token().is_cancelled());
        assert!(session.register_tunnel(Uuid::new_v4()).is_none());
    }

    #[tokio::test]
    async fn descartar_futuro_de_pedido_limpa_pendente_sem_vazar() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let session = std::sync::Arc::new(session);
        let task = {
            let session = session.clone();
            tokio::spawn(async move { session.request(list(), Duration::from_secs(30)).await })
        };
        let _ = outbound.recv().await.expect("pedido");
        assert_eq!(session.pending_count(), 1);
        task.abort();
        let _ = task.await;
        assert_eq!(session.pending_count(), 0);
    }

    #[tokio::test]
    async fn descartar_futuro_de_pedido_avisa_o_agente() {
        let (session, mut outbound) = session(vec![Permission::Read]);
        let session = std::sync::Arc::new(session);
        let task = {
            let session = session.clone();
            tokio::spawn(async move { session.request(list(), Duration::from_secs(30)).await })
        };
        let pedido = outbound.recv().await.expect("pedido");
        task.abort();
        let _ = task.await;
        let aviso = outbound.recv().await.expect("cancel");
        assert_eq!(aviso.id, pedido.id);
        assert!(matches!(aviso.body, Body::Cancel));
    }

    #[tokio::test]
    async fn cancelar_com_fila_cheia_nao_trava_indefinidamente() {
        let (tx, _rx) = mpsc::channel(1);
        let session = AgentSession::new(7, "srv", hello(vec![Permission::Read]), tx.clone());
        // Enche a fila de saída com um quadro que ninguém lê.
        tx.send(Envelope::reply(Uuid::new_v4(), Body::Cancel))
            .await
            .unwrap();
        let result =
            tokio::time::timeout(Duration::from_secs(2), session.cancel(Uuid::new_v4())).await;
        assert!(result.is_ok(), "cancel não deve travar com fila cheia");
    }
}
