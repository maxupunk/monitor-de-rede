//! Logs em follow entregues pelo barramento SSE global (`docker:log`).
//!
//! Abrir é uma ação explícita do usuário (`POST .../logs/follow`); fechar é
//! explícito (`DELETE`), por tempo (10 min) ou quando não há mais ninguém
//! ouvindo o SSE. As linhas são agrupadas a cada meio segundo. Não existe
//! endpoint SSE paralelo por tela (AGENTS §9).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use loco_rs::app::AppContext;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    services::{
        events::EventBus,
        shared::errors::{AppError, AppResult},
    },
    views::agents::DockerLogStreamEvent,
};

use super::{engine, hosts::DockerHost, source::LogChunk};

pub const EVENT: &str = "docker:log";
const MAX_STREAMS: usize = 20;
const MAX_DURATION: Duration = Duration::from_secs(10 * 60);
const BATCH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Default)]
pub struct LogStreams {
    active: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl LogStreams {
    pub fn install(ctx: &AppContext) {
        if !ctx.shared_store.contains::<Self>() {
            ctx.shared_store.insert(Self::default());
        }
    }

    /// # Errors
    ///
    /// Registro ausente neste processo.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store.get::<Self>().ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "Registro de streams de log não inicializado"
            ))
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, CancellationToken>> {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// # Errors
    ///
    /// Limite de streams simultâneos atingido.
    pub fn start(
        &self,
        ctx: &AppContext,
        host: DockerHost,
        container_id: String,
        tail: String,
    ) -> AppResult<String> {
        let token = CancellationToken::new();
        let id = Uuid::new_v4().to_string();
        {
            let mut active = self.lock();
            if active.len() >= MAX_STREAMS {
                return Err(AppError::rate_limited(
                    "Muitos logs acompanhados ao mesmo tempo; feche algum antes",
                    5,
                ));
            }
            active.insert(id.clone(), token.clone());
        }
        let registry = self.clone();
        let ctx = ctx.clone();
        let stream_id = id.clone();
        tokio::spawn(async move {
            let ended = run(&ctx, &host, &stream_id, &container_id, &tail, token).await;
            registry.lock().remove(&stream_id);
            publish(
                &ctx,
                &event(&stream_id, &host, &container_id, Vec::new(), Some(ended)),
            );
        });
        Ok(id)
    }

    /// Encerra o stream; `false` se ele já não existia.
    pub fn stop(&self, stream_id: &str) -> bool {
        self.lock().remove(stream_id).is_some_and(|token| {
            token.cancel();
            true
        })
    }
}

fn event(
    stream_id: &str,
    host: &DockerHost,
    container_id: &str,
    entries: Vec<crate::views::docker::DockerLogEntry>,
    ended: Option<String>,
) -> DockerLogStreamEvent {
    DockerLogStreamEvent {
        stream_id: stream_id.to_string(),
        host_key: host.key.to_string(),
        container_id: container_id.to_string(),
        entries,
        ended,
    }
}

fn publish(ctx: &AppContext, event: &DockerLogStreamEvent) {
    if let (Ok(bus), Ok(payload)) = (EventBus::from_context(ctx), serde_json::to_value(event)) {
        bus.publish_ephemeral(EVENT, payload);
    }
}

fn has_listeners(ctx: &AppContext) -> bool {
    EventBus::from_context(ctx).is_ok_and(|bus| bus.has_subscribers())
}

/// Devolve o motivo do encerramento, que vai no último evento.
async fn run(
    ctx: &AppContext,
    host: &DockerHost,
    stream_id: &str,
    container_id: &str,
    tail: &str,
    token: CancellationToken,
) -> String {
    let (chunks, mut received) = mpsc::unbounded_channel::<LogChunk>();
    let follow = {
        let engine = host.engine.clone();
        let container_id = container_id.to_string();
        let tail = tail.to_string();
        let token = token.clone();
        tokio::spawn(async move {
            engine
                .follow_logs(&container_id, &tail, chunks, token)
                .await
        })
    };
    let deadline = tokio::time::Instant::now() + MAX_DURATION;
    let mut ticker = tokio::time::interval(BATCH_INTERVAL);
    let mut batch: Vec<LogChunk> = Vec::new();
    let reason = loop {
        tokio::select! {
            chunk = received.recv() => match chunk {
                Some(chunk) => batch.push(chunk),
                None => break "O stream de logs terminou".to_string(),
            },
            _ = ticker.tick() => {
                if !batch.is_empty() {
                    let entries = engine::log_entries(&std::mem::take(&mut batch), true);
                    publish(ctx, &event(stream_id, host, container_id, entries, None));
                }
                if token.is_cancelled() {
                    break "Acompanhamento encerrado".to_string();
                }
                if tokio::time::Instant::now() >= deadline {
                    break "Tempo máximo de acompanhamento atingido".to_string();
                }
                if !has_listeners(ctx) {
                    break "Nenhuma tela acompanhando".to_string();
                }
            }
        }
    };
    token.cancel();
    if !batch.is_empty() {
        let entries = engine::log_entries(&batch, true);
        publish(ctx, &event(stream_id, host, container_id, entries, None));
    }
    match follow.await {
        Ok(Err(error)) => error.to_string(),
        _ => reason,
    }
}
