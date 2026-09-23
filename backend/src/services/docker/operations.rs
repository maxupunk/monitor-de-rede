//! Operações longas (pull, atualização, compose) com progresso por SSE.
//!
//! A requisição HTTP só **aceita** a operação (`202`) e devolve o id. O
//! andamento chega como `docker:operation` no barramento global — nada de
//! polling nem de endpoint SSE próprio (AGENTS §9). O progresso é
//! desacelerado: um pull relata dezenas de linhas por segundo e a tela só
//! precisa da mais recente.

use std::{future::Future, time::Duration};

use loco_rs::app::AppContext;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    services::events::EventBus,
    views::{
        agents::{DockerOperationAccepted, DockerOperationEvent},
        docker::DockerActionResponse,
    },
};

use super::{hosts::HostKey, maintenance::Progress, realtime, DockerError};

pub const EVENT: &str = "docker:operation";
/// Intervalo mínimo entre dois eventos de progresso da mesma operação.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(300);

pub struct Operation {
    pub host: HostKey,
    pub kind: &'static str,
    pub target: String,
}

impl Operation {
    fn event(
        &self,
        id: &str,
        state: &str,
        line: Option<String>,
        message: Option<String>,
    ) -> DockerOperationEvent {
        DockerOperationEvent {
            operation_id: id.to_string(),
            host_key: self.host.to_string(),
            kind: self.kind.to_string(),
            target: self.target.clone(),
            state: state.to_string(),
            line,
            message,
        }
    }
}

fn publish(ctx: &AppContext, event: &DockerOperationEvent) {
    if let (Ok(bus), Ok(payload)) = (EventBus::from_context(ctx), serde_json::to_value(event)) {
        bus.publish_ephemeral(EVENT, payload);
    }
}

/// Inicia a operação em segundo plano e devolve o que a tela precisa para
/// acompanhá-la.
pub fn start<F, Fut>(ctx: &AppContext, operation: Operation, run: F) -> DockerOperationAccepted
where
    F: FnOnce(Progress) -> Fut + Send + 'static,
    Fut: Future<Output = Result<DockerActionResponse, DockerError>> + Send + 'static,
{
    let id = Uuid::new_v4().to_string();
    let accepted = DockerOperationAccepted {
        operation_id: id.clone(),
        host_key: operation.host.to_string(),
        kind: operation.kind.to_string(),
        target: operation.target.clone(),
    };
    let ctx = ctx.clone();
    tokio::spawn(async move {
        publish(&ctx, &operation.event(&id, "running", None, None));
        let (progress, mut lines) = mpsc::unbounded_channel::<String>();
        let forwarder = {
            let ctx = ctx.clone();
            let id = id.clone();
            let base = operation.event(&id, "running", None, None);
            tokio::spawn(async move {
                let mut last_sent = tokio::time::Instant::now() - PROGRESS_THROTTLE;
                let mut pending: Option<String> = None;
                loop {
                    let wait = PROGRESS_THROTTLE.saturating_sub(last_sent.elapsed());
                    tokio::select! {
                        line = lines.recv() => match line {
                            Some(line) => pending = Some(line),
                            None => break,
                        },
                        () = tokio::time::sleep(wait), if pending.is_some() => {}
                    }
                    if last_sent.elapsed() >= PROGRESS_THROTTLE {
                        if let Some(line) = pending.take() {
                            let mut event = base.clone();
                            event.line = Some(line);
                            publish(&ctx, &event);
                            last_sent = tokio::time::Instant::now();
                        }
                    }
                }
                if let Some(line) = pending {
                    let mut event = base;
                    event.line = Some(line);
                    publish(&ctx, &event);
                }
            })
        };
        let result = run(progress).await;
        let _ = forwarder.await;
        let event = match &result {
            Ok(response) => operation.event(&id, "succeeded", None, Some(response.message.clone())),
            Err(error) => operation.event(&id, "failed", None, Some(error.to_string())),
        };
        publish(&ctx, &event);
        realtime::refresh(&ctx, operation.host).await;
    });
    accepted
}
