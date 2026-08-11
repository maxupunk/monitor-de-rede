//! Fila persistente de discovery em `discovery_runs`.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::{
    models::{discovery_runs, networks},
    services::{
        discovery::service::{run_discovery, ScanSessionService},
        shared::errors::AppResult,
    },
};

pub const MIN_SCAN_INTERVAL_SECONDS: i64 = 300;

pub async fn enqueue_network_scan(
    db: &sea_orm::DatabaseConnection,
    network: &networks::Model,
) -> AppResult<(discovery_runs::Model, bool)> {
    if let Some(current) = discovery_runs::Entity::find()
        .filter(crate::models::_entities::discovery_runs::Column::NetworkId.eq(network.id))
        .filter(crate::models::_entities::discovery_runs::Column::Status.eq("pending"))
        .one(db)
        .await?
    {
        // A rede pode ter tido o CIDR corrigido entre o agendamento e o ciclo.
        let run = discovery_runs::ActiveModel {
            id: Set(current.id),
            configuration: Set(Some(serde_json::json!({ "cidr":network.cidr }))),
            ..Default::default()
        }
        .update(db)
        .await?;
        return Ok((run, true));
    }
    let run = discovery_runs::ActiveModel {
        network_id: Set(network.id),
        probe_id: Set(network.probe_id),
        status: Set("pending".into()),
        started_at: Set(Utc::now().into()),
        configuration: Set(Some(serde_json::json!({ "cidr":network.cidr }))),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok((run, false))
}

pub async fn schedule_due_networks(db: &sea_orm::DatabaseConnection) -> AppResult<u64> {
    let now = Utc::now();
    let networks = networks::Entity::find().all(db).await?;
    let mut count = 0;
    for network in networks.into_iter().filter(|network| {
        network.active
            && network.scan_enabled
            && network
                .next_scan_at
                .is_none_or(|next| next.with_timezone(&Utc) <= now)
    }) {
        enqueue_network_scan(db, &network).await?;
        let interval = i64::from(network.scan_interval).max(MIN_SCAN_INTERVAL_SECONDS);
        networks::ActiveModel {
            id: Set(network.id),
            last_scan_at: Set(Some(now.into())),
            next_scan_at: Set(Some((now + chrono::Duration::seconds(interval)).into())),
            ..Default::default()
        }
        .update(db)
        .await?;
        count += 1;
    }
    Ok(count)
}

pub async fn process_pending_runs(ctx: &AppContext) -> AppResult<u64> {
    let Some(run) = discovery_runs::Entity::find()
        .filter(crate::models::_entities::discovery_runs::Column::Status.eq("pending"))
        .order_by_asc(crate::models::_entities::discovery_runs::Column::Id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(0);
    };
    let cidr = run
        .configuration
        .as_ref()
        .and_then(|config| config.get("cidr"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let run = discovery_runs::ActiveModel {
        id: Set(run.id),
        status: Set("running".into()),
        started_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    let session = ScanSessionService::from_context(ctx)?;
    let cancel = session.start(run.id, run.network_id).await;
    match run_discovery(ctx, &cidr, run.id, cancel).await {
        Ok(_) => {
            discovery_runs::ActiveModel {
                id: Set(run.id),
                status: Set("completed".into()),
                finished_at: Set(Some(Utc::now().into())),
                ..Default::default()
            }
            .update(&ctx.db)
            .await?;
            session.finish(None).await;
            Ok(1)
        }
        Err(error) => {
            discovery_runs::ActiveModel {
                id: Set(run.id),
                status: Set("failed".into()),
                finished_at: Set(Some(Utc::now().into())),
                error: Set(Some(error.to_string())),
                ..Default::default()
            }
            .update(&ctx.db)
            .await?;
            session.finish(Some(error.to_string())).await;
            Ok(1)
        }
    }
}
