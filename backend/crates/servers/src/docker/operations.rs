//! Operações longas (pull, atualização, compose) com progresso por SSE.
//!
//! A requisição HTTP só **aceita** a operação (`202`) e devolve o id. O
//! andamento chega como `docker:operation` no barramento global — nada de
//! polling nem de endpoint SSE próprio (AGENTS §9). O progresso é
//! desacelerado: um pull relata dezenas de linhas por segundo e a tela só
//! precisa da mais recente.

use std::{
    collections::HashSet,
    future::Future,
    sync::{LazyLock, Mutex},
    time::Duration,
};

use loco_rs::app::AppContext;
use tokio::sync::mpsc;
use uuid::Uuid;

use netmonitor_core::{
    events::EventBus,
    shared::errors::{AppError, AppResult},
};

use crate::views::docker::{DockerActionResponse, DockerOperationAccepted, DockerOperationEvent};

use super::{hosts::HostKey, maintenance::Progress, DockerError};

pub const EVENT: &str = "docker:operation";
/// Intervalo mínimo entre dois eventos de progresso da mesma operação.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(300);

type OpKey = (String, &'static str, String);
static IN_FLIGHT: LazyLock<Mutex<HashSet<OpKey>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

struct OpGuard(OpKey);

impl OpGuard {
    fn acquire(key: OpKey) -> AppResult<Self> {
        let mut active = IN_FLIGHT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !active.insert(key.clone()) {
            return Err(AppError::conflict(
                "Já existe uma operação em andamento para este recurso",
            ));
        }
        Ok(Self(key))
    }
}

impl Drop for OpGuard {
    fn drop(&mut self) {
        let mut active = IN_FLIGHT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        active.remove(&self.0);
    }
}

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

/// Inicia a operação em segundo plano (recusando duplicata simultânea no mesmo
/// `(host, kind, target)` com `409 Conflict`) e devolve o que a tela precisa
/// para acompanhá-la.
///
/// # Errors
///
/// `409 Conflict` se a mesma operação já estiver em andamento.
pub fn try_start<F, Fut>(
    ctx: &AppContext,
    operation: Operation,
    run: F,
) -> AppResult<DockerOperationAccepted>
where
    F: FnOnce(Progress) -> Fut + Send + 'static,
    Fut: Future<Output = Result<DockerActionResponse, DockerError>> + Send + 'static,
{
    let guard = OpGuard::acquire((
        operation.host.to_string(),
        operation.kind,
        operation.target.clone(),
    ))?;
    Ok(spawn_operation(ctx, operation, Some(guard), run))
}

/// Inicia a operação em segundo plano e devolve o que a tela precisa para
/// acompanhá-la.
pub fn start<F, Fut>(ctx: &AppContext, operation: Operation, run: F) -> DockerOperationAccepted
where
    F: FnOnce(Progress) -> Fut + Send + 'static,
    Fut: Future<Output = Result<DockerActionResponse, DockerError>> + Send + 'static,
{
    spawn_operation(ctx, operation, None, run)
}

fn spawn_operation<F, Fut>(
    ctx: &AppContext,
    operation: Operation,
    guard: Option<OpGuard>,
    run: F,
) -> DockerOperationAccepted
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
        let _guard = guard;
        publish(&ctx, &operation.event(&id, "running", None, None));
        let (progress, mut lines) = mpsc::unbounded_channel::<String>();
        let forwarder = {
            let ctx = ctx.clone();
            let id = id.clone();
            let base = operation.event(&id, "running", None, None);
            tokio::spawn(async move {
                let mut last_sent: Option<tokio::time::Instant> = None;
                let mut pending: Option<String> = None;
                loop {
                    let wait = last_sent.map_or(Duration::ZERO, |at| {
                        PROGRESS_THROTTLE.saturating_sub(at.elapsed())
                    });
                    tokio::select! {
                        line = lines.recv() => match line {
                            Some(line) => pending = Some(line),
                            None => break,
                        },
                        () = tokio::time::sleep(wait), if pending.is_some() => {}
                    }
                    if last_sent.is_none_or(|at| at.elapsed() >= PROGRESS_THROTTLE) {
                        if let Some(line) = pending.take() {
                            let mut event = base.clone();
                            event.line = Some(line);
                            publish(&ctx, &event);
                            last_sent = Some(tokio::time::Instant::now());
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
        // O estado novo dos containers vem do agente, que publica o snapshot
        // seguinte a toda mutação.
        publish(&ctx, &event);
    });
    accepted
}
