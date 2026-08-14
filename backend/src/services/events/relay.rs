use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::{
    models::{_entities::event_outbox as event_outbox_entity, event_outbox},
    services::{
        events::{DomainEvent, EventBus},
        shared::errors::AppResult,
    },
};

/// Replica eventos de outros processos somente quando ha cliente SSE conectado.
/// O `origin` evita que a instancia que persistiu o evento o publique duas vezes.
pub async fn relay_pending(ctx: &AppContext) -> AppResult<u64> {
    let bus = EventBus::from_context(ctx)?;
    if !bus.has_subscribers() {
        return Ok(0);
    }
    let last_id = bus.last_relayed_id().await;
    let rows = event_outbox::Entity::find()
        .filter(event_outbox_entity::Column::Id.gt(last_id))
        .order_by_asc(event_outbox_entity::Column::Id)
        .all(&ctx.db)
        .await?;
    let mut relayed = 0;
    for row in rows {
        bus.set_last_relayed_id(row.id).await;
        if row.origin == bus.origin() {
            continue;
        }
        let event = serde_json::from_value::<DomainEvent>(row.payload).unwrap_or(DomainEvent {
            event_type: row.r#type,
            payload: serde_json::json!({}),
            occurred_at: row.created_at.to_rfc3339(),
        });
        bus.publish_local(event);
        relayed += 1;
    }
    Ok(relayed)
}
