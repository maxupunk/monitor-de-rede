use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    events::{DomainEvent, EventBus},
    models::{_entities::event_outbox as event_outbox_entity, event_outbox},
    shared::errors::AppResult,
};

/// Replica eventos de outros processos somente quando ha cliente SSE conectado.
/// O `origin` evita que a instancia que persistiu o evento o publique duas vezes.
/// Uma passada é deliberadamente limitada: backlog não pode virar um vetor
/// proporcional ao tamanho do outbox nem inundar o canal SSE de uma só vez.
pub const RELAY_BATCH_SIZE: u64 = 500;

pub async fn relay_pending(ctx: &AppContext) -> AppResult<u64> {
    let bus = EventBus::from_context(ctx)?;
    if !bus.has_subscribers() {
        return Ok(0);
    }
    // Mantém a trava durante a passada inteira: impede duas passadas
    // concorrentes de retransmitir o mesmo lote e evita travar/destravar por linha.
    let mut last_id = bus.lock_last_relayed_id().await;

    // Consulta O(1) pelo topo do PK: evita varrer o outbox ou transferir JSON
    // próprio quando não há linhas novas.
    let Some(latest_id) = event_outbox::Entity::find()
        .select_only()
        .column(event_outbox_entity::Column::Id)
        .order_by_desc(event_outbox_entity::Column::Id)
        .limit(1)
        .into_tuple::<i64>()
        .one(&ctx.db)
        .await?
    else {
        return Ok(0);
    };
    if latest_id <= *last_id {
        return Ok(0);
    }

    let rows = event_outbox::Entity::find()
        .filter(event_outbox_entity::Column::Id.gt(*last_id))
        .filter(event_outbox_entity::Column::Id.lte(latest_id))
        .filter(event_outbox_entity::Column::Origin.ne(bus.origin()))
        .order_by_asc(event_outbox_entity::Column::Id)
        .limit(RELAY_BATCH_SIZE)
        .all(&ctx.db)
        .await?;

    let mut relayed = 0_u64;
    let mut max_relayed_id = *last_id;
    let batch_full = rows.len() as u64 >= RELAY_BATCH_SIZE;
    for row in rows {
        max_relayed_id = row.id;
        let event = serde_json::from_value::<DomainEvent>(row.payload).unwrap_or(DomainEvent {
            event_type: row.r#type,
            payload: serde_json::json!({}),
            occurred_at: row.created_at.to_rfc3339(),
        });
        bus.publish_local(event);
        relayed += 1;
    }

    // Se o lote veio cheio, ainda podem existir linhas remotas até `latest_id`;
    // caso contrário, toda a faixa `(last_id, latest_id]` (incluindo linhas da
    // própria origem) já foi drenada.
    *last_id = if batch_full {
        max_relayed_id
    } else {
        latest_id
    };
    Ok(relayed)
}
