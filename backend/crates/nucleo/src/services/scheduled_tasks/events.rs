//! Evento `scheduled_task:updated` no barramento global (SSE).
//!
//! Vai pelo `event_outbox` (`EventBus::publish`): a execução roda no
//! processo do servidor, mas o CRUD também é chamado por tarefas de linha de
//! comando e testes — o relay entrega de qualquer origem.

use loco_rs::prelude::AppContext;
use serde_json::json;

use super::model::{ScheduledTaskEvent, ScheduledTaskStatus};
use crate::services::events::EventBus;

pub const UPDATED: &str = "scheduled_task:updated";

/// Avisa as telas que a tarefa mudou (ou rodou). Falha vira só log.
pub async fn publish(ctx: &AppContext, id: i64, name: &str, status: Option<ScheduledTaskStatus>) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    if let Err(error) = bus
        .publish(
            &ctx.db,
            UPDATED,
            json!(ScheduledTaskEvent {
                id,
                name: name.to_string(),
                status
            }),
        )
        .await
    {
        tracing::warn!(%error, task_id = id, "falha ao publicar {UPDATED}");
    }
}
