//! Ciclo de vida de uma conexão de agente, do lado da central.
//!
//! Independente de WebSocket: recebe e envia quadros ([`WireFrame`]) por canais. O controller
//! só faz a ponte entre o socket e estes canais, e os testes ligam os canais
//! direto num agente em memória.

use std::{sync::Arc, time::Duration};

use loco_rs::app::AppContext;
use tokio::sync::mpsc;

use crate::{
    models::probes,
    services::{docker::hosts::HostKey, events::EventBus},
};

use super::{
    hub::AgentHub,
    inbound,
    protocol::{Body, Envelope, Welcome, WireFrame, PROTOCOL_VERSION},
    service::{publish_status, AgentService},
    session::AgentSession,
};

/// O agente tem este tempo para se apresentar depois do upgrade.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Frequência com que a conexão aberta renova o `last_seen_at`.
const TOUCH_INTERVAL: Duration = Duration::from_secs(30);
const OUTBOUND_BUFFER: usize = 64;
/// Quadros de ponte na fila de saída. Pequena de propósito: a janela de
/// crédito já limita o que cada ponte manda, e o controle passa na frente.
const TUNNEL_BUFFER: usize = 16;

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
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, incoming.recv()).await {
        Ok(Some(WireFrame::Text(text))) => {
            match Envelope::decode(&text).map(|envelope| envelope.body) {
                Ok(Body::Hello(hello)) => hello,
                _ => return Err(HandshakeError::NotHello),
            }
        }
        _ => return Err(HandshakeError::Timeout),
    };
    if hello.protocol != PROTOCOL_VERSION {
        return Err(HandshakeError::Protocol(hello.protocol));
    }

    let service = AgentService::new(&ctx.db);
    let probe = match service.mark_connected(probe, &hello).await {
        Ok(probe) => probe,
        Err(error) => {
            tracing::warn!(%error, "falha ao registrar conexão do agente");
            return Ok(());
        }
    };
    publish_status(&ctx, &probe).await;
    let Ok(hub) = AgentHub::from_context(&ctx) else {
        return Ok(());
    };

    let (envelopes, mut to_send) = mpsc::channel::<Envelope>(OUTBOUND_BUFFER);
    let (tunnel_out, mut tunnel_frames) = mpsc::channel::<WireFrame>(TUNNEL_BUFFER);
    let session = Arc::new(
        AgentSession::new(probe.id, probe.name.clone(), hello, envelopes.clone())
            .with_tunnel_out(tunnel_out),
    );
    hub.register(session.clone());
    tracing::info!(probe_id = probe.id, name = %probe.name, "agente conectado");

    // Controle primeiro: um dump pela ponte não pode atrasar monitores e Docker.
    let writer = tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                biased;
                envelope = to_send.recv() => match envelope {
                    Some(envelope) => match envelope.encode() {
                        Ok(text) => WireFrame::Text(text),
                        Err(error) => {
                            tracing::warn!(%error, "quadro para o agente descartado");
                            continue;
                        }
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
    let _ = envelopes
        .send(Envelope::new(Body::Welcome(Welcome {
            probe_id: probe.id,
            name: probe.name.clone(),
            live_interval_ms: hub.live_interval_ms(),
        })))
        .await;
    drop(envelopes);

    let mut touch = tokio::time::interval(TOUCH_INTERVAL);
    touch.tick().await;
    loop {
        tokio::select! {
            frame = incoming.recv() => {
                let text = match frame {
                    Some(WireFrame::Text(text)) => text,
                    Some(WireFrame::Binary(bytes)) => {
                        session.handle_tunnel_frame(&bytes);
                        continue;
                    }
                    None => break,
                };
                match Envelope::decode(&text) {
                    Ok(envelope) => {
                        if let Some(event) = session.handle(envelope) {
                            inbound::handle(&ctx, probe.id, event).await;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, probe_id = probe.id, "quadro inválido do agente");
                    }
                }
            }
            _ = touch.tick() => {
                if let Err(error) = service.touch(probe.id).await {
                    tracing::warn!(%error, probe_id = probe.id, "falha ao renovar last_seen_at");
                }
            }
        }
    }

    if hub.unregister(&session) {
        disconnected(&ctx, probe.id).await;
    }
    writer.abort();
    tracing::info!(probe_id = probe.id, "agente desconectado");
    Ok(())
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
