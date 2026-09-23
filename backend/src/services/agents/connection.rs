//! Ciclo de vida de uma conexão de agente, do lado da central.
//!
//! Independente de WebSocket: recebe e envia texto por canais. O controller
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
    protocol::{Body, Envelope, Welcome, PROTOCOL_VERSION},
    service::{publish_status, AgentService},
    session::AgentSession,
};

/// O agente tem este tempo para se apresentar depois do upgrade.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Frequência com que a conexão aberta renova o `last_seen_at`.
const TOUCH_INTERVAL: Duration = Duration::from_secs(30);
const OUTBOUND_BUFFER: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HandshakeError {
    #[error("o agente não se apresentou a tempo")]
    Timeout,
    #[error("o primeiro quadro precisa ser Hello")]
    NotHello,
    #[error("versão de protocolo {0} não suportada (esperada {PROTOCOL_VERSION})")]
    Protocol(u16),
}

/// Atende a conexão até ela cair. `incoming`/`outgoing` são quadros de texto.
///
/// # Errors
///
/// Falha de apresentação; depois dela a função só retorna quando o canal cai.
pub async fn serve(
    ctx: AppContext,
    probe: probes::Model,
    mut incoming: mpsc::Receiver<String>,
    outgoing: mpsc::Sender<String>,
) -> Result<(), HandshakeError> {
    let hello = match tokio::time::timeout(HELLO_TIMEOUT, incoming.recv()).await {
        Ok(Some(text)) => match Envelope::decode(&text).map(|envelope| envelope.body) {
            Ok(Body::Hello(hello)) => hello,
            _ => return Err(HandshakeError::NotHello),
        },
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
    let session = Arc::new(AgentSession::new(
        probe.id,
        probe.name.clone(),
        hello,
        envelopes.clone(),
    ));
    hub.register(session.clone());
    tracing::info!(probe_id = probe.id, name = %probe.name, "agente conectado");

    let writer = tokio::spawn(async move {
        while let Some(envelope) = to_send.recv().await {
            match envelope.encode() {
                Ok(text) => {
                    if outgoing.send(text).await.is_err() {
                        break;
                    }
                }
                Err(error) => tracing::warn!(%error, "quadro para o agente descartado"),
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
                let Some(text) = frame else { break };
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
