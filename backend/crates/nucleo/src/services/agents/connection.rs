//! Ciclo de vida de uma conexão de agente, do lado da central.
//!
//! Independente de WebSocket: recebe e envia quadros ([`WireFrame`]) por canais. O controller
//! só faz a ponte entre o socket e estes canais, e os testes ligam os canais
//! direto num agente em memória.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};

use loco_rs::app::AppContext;
use tokio::sync::mpsc;

use netmonitor_core::agents::HostKey;

use crate::{models::probes, services::events::EventBus};

use super::{
    hub::AgentHub,
    inbound,
    protocol::{AgentEvent, Body, Envelope, ProtocolError, Welcome, WireFrame, PROTOCOL_VERSION},
    service::{publish_status, AgentService},
    session::AgentSession,
    spawn_priority_writer,
};

/// O agente tem este tempo para se apresentar depois do upgrade.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const OUTBOUND_BUFFER: usize = 64;
/// Quadros de ponte na fila de saída. Pequena de propósito: a janela de
/// crédito já limita o que cada ponte manda, e o controle passa na frente.
const TUNNEL_BUFFER: usize = 16;
/// Fila de eventos com escrita em banco (`MetricsRollup`, `MonitorResult`,
/// `DiscoveryResult`), processados fora do laço do WebSocket.
const INBOUND_WORKER_BUFFER: usize = 128;
/// Prazo para gravar o que já está na fila quando a conexão fecha: o agente
/// já tirou esses eventos do buffer dele, então descartá-los é perda.
const INBOUND_DRAIN_GRACE: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HandshakeError {
    #[error("o agente não se apresentou a tempo")]
    Timeout,
    #[error("o primeiro quadro precisa ser Hello")]
    NotHello,
    #[error(
        "o agente fala o protocolo {0} e a central o {PROTOCOL_VERSION} — atualize o lado mais antigo"
    )]
    Protocol(u16),
}

/// Serializa transições `online` ↔ `offline` por `probe_id` para evitar que a
/// saída de uma conexão antiga sobrescreva uma reconexão nova no banco/SSE.
fn probe_transition_lock(probe_id: i64) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<i64, Weak<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let mut map = LOCKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(existing) = map.get(&probe_id).and_then(Weak::upgrade) {
        return existing;
    }
    if map.len() > 256 {
        map.retain(|_, weak| weak.strong_count() > 0);
    }
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    map.insert(probe_id, Arc::downgrade(&lock));
    lock
}

async fn read_hello(
    incoming: &mut mpsc::Receiver<WireFrame>,
) -> Result<super::protocol::Hello, HandshakeError> {
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, incoming.recv()).await {
        Ok(Some(WireFrame::Text(text))) => match Envelope::decode(&text) {
            Ok(envelope) => match envelope.body {
                Body::Hello(hello) => hello,
                _ => return Err(HandshakeError::NotHello),
            },
            Err(ProtocolError::UnsupportedVersion(version)) => {
                return Err(HandshakeError::Protocol(version));
            }
            Err(_) => return Err(HandshakeError::NotHello),
        },
        _ => return Err(HandshakeError::Timeout),
    };
    if hello.protocol != PROTOCOL_VERSION {
        return Err(HandshakeError::Protocol(hello.protocol));
    }
    Ok(hello)
}

/// Atende a conexão até ela cair.
///
/// # Errors
///
/// Falha de apresentação; depois dela a função só retorna quando o canal cai.
pub async fn serve(
    ctx: AppContext,
    probe: probes::Model,
    mut incoming: mpsc::Receiver<WireFrame>,
    outgoing: mpsc::Sender<WireFrame>,
) -> Result<(), HandshakeError> {
    let hello = read_hello(&mut incoming).await?;
    let Ok(hub) = AgentHub::from_context(&ctx) else {
        return Ok(());
    };

    let (envelopes, to_send) = mpsc::channel::<Envelope>(OUTBOUND_BUFFER);
    let (tunnel_out, tunnel_frames) = mpsc::channel::<WireFrame>(TUNNEL_BUFFER);
    let probe_id = probe.id;
    let probe_name = probe.name.clone();
    let session = Arc::new(
        AgentSession::new(
            probe_id,
            probe_name.clone(),
            hello.clone(),
            envelopes.clone(),
        )
        .with_tunnel_out(tunnel_out),
    );
    let close_token = session.close_token();
    let loop_stop = close_token.child_token();

    let transition = probe_transition_lock(probe_id);
    let (probe, writer) = {
        let _guard = transition.lock().await;
        let service = AgentService::new(&ctx.db);
        let probe = match service.mark_connected(probe, &hello).await {
            Ok(probe) => probe,
            Err(error) => {
                tracing::warn!(%error, probe_id, "falha ao registrar conexão do agente");
                return Ok(());
            }
        };
        // Enfileira `Welcome` ANTES de `hub.register`: qualquer pedido disparado
        // assim que `wait_for` acordar vai estritamente atrás do `Welcome`.
        let _ = envelopes
            .send(Envelope::new(Body::Welcome(Welcome {
                probe_id: probe.id,
                name: probe.name.clone(),
                live_interval_ms: hub.live_interval_ms(),
            })))
            .await;
        drop(envelopes);
        // Controle primeiro: um dump pela ponte não pode atrasar monitores e Docker.
        let writer = spawn_priority_writer(to_send, tunnel_frames, outgoing, loop_stop.clone());
        hub.register(session.clone());
        publish_status(&ctx, &probe).await;
        (probe, writer)
    };
    tracing::info!(probe_id = probe.id, name = %probe.name, "agente conectado");

    // Eventos que gravam em banco saem do laço do WebSocket para nunca travar
    // respostas de RPC nem `Ack`s binários das pontes. O worker não segue o
    // `close_token` da sessão: takeover, revogação ou queda fecham o canal,
    // mas o que já chegou é gravado até a fila esvaziar (ver o dreno abaixo).
    let (inbound_tx, mut inbound_rx) = mpsc::channel::<AgentEvent>(INBOUND_WORKER_BUFFER);
    let mut inbound_worker = {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            while let Some(event) = inbound_rx.recv().await {
                inbound::handle(&ctx, probe_id, event).await;
            }
        })
    };

    loop {
        tokio::select! {
            biased;
            () = loop_stop.cancelled() => break,
            frame = incoming.recv() => {
                let text = match frame {
                    Some(WireFrame::Text(text)) => text,
                    Some(WireFrame::Binary(bytes)) => {
                        session.handle_tunnel_frame_owned(bytes);
                        continue;
                    }
                    None => break,
                };
                match Envelope::decode(&text) {
                    Ok(envelope) => {
                        if let Some(event) = session.handle(envelope) {
                            dispatch_inbound(&ctx, probe_id, &inbound_tx, &loop_stop, event).await;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, probe_id, "quadro inválido do agente");
                    }
                }
            }
        }
    }

    drop(inbound_tx);
    if tokio::time::timeout(INBOUND_DRAIN_GRACE, &mut inbound_worker)
        .await
        .is_err()
    {
        inbound_worker.abort();
        tracing::warn!(
            probe_id,
            "worker de eventos do agente excedeu o prazo de dreno; eventos restantes descartados"
        );
    }

    if hub.unregister(&session) {
        let _guard = transition.lock().await;
        if hub.get(probe_id).is_none() {
            disconnected(&ctx, probe_id).await;
        }
    }
    writer.abort();
    tracing::info!(probe_id, "agente desconectado");
    Ok(())
}

/// Snapshots Docker são só memória (SSE) e seguem na hora; eventos com banco
/// vão para a fila do worker. Fila cheia segura a leitura (contrapressão)
/// em vez de descartar: o agente já tirou o evento do buffer offline dele.
async fn dispatch_inbound(
    ctx: &AppContext,
    probe_id: i64,
    worker: &mpsc::Sender<AgentEvent>,
    stop: &tokio_util::sync::CancellationToken,
    event: AgentEvent,
) {
    match event {
        AgentEvent::DockerLive { .. } | AgentEvent::DockerInventory { .. } => {
            inbound::handle(ctx, probe_id, event).await;
        }
        other => {
            let queued = match worker.try_send(other) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(event)) => tokio::select! {
                    biased;
                    () = stop.cancelled() => false,
                    sent = worker.send(event) => sent.is_ok(),
                },
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            };
            if !queued {
                tracing::warn!(probe_id, "conexão do agente encerrada; evento descartado");
            }
        }
    }
}

async fn disconnected(ctx: &AppContext, probe_id: i64) {
    match AgentService::new(&ctx.db).mark_disconnected(probe_id).await {
        Ok(Some(probe)) => publish_status(ctx, &probe).await,
        Ok(None) => {}
        Err(error) => tracing::warn!(%error, probe_id, "falha ao marcar agente offline"),
    }
    if let Ok(bus) = EventBus::from_context(ctx) {
        bus.forget_snapshots(&HostKey::agent(probe_id).to_string());
    }
}
