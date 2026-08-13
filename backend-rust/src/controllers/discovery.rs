//! Discovery ao vivo, sessão SSE e histórico persistido.

use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse,
};
use chrono::{Duration, Utc};
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use std::{collections::BTreeMap, convert::Infallible};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use crate::{
    dtos::resources::DiscoveryScanInput,
    models::{discovery_results, discovery_runs, networks},
    services::{
        discovery::{
            cidr_range::parse_cidr_range,
            service::{run_discovery, ScanSessionService},
        },
        shared::errors::{AppError, AppResult},
    },
};

async fn scan_state(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(
        serde_json::json!({ "data": ScanSessionService::from_context(&ctx)?.state().await }),
    )?)
}

async fn scan_stream(State(ctx): State<AppContext>) -> AppResult<Response> {
    let session = ScanSessionService::from_context(&ctx)?;
    let initial = session.state().await;
    let mut updates = session.subscribe();
    let (sender, receiver) = mpsc::channel(16);
    tokio::spawn(async move {
        if sender.send(initial).await.is_err() {
            return;
        }
        loop {
            match updates.recv().await {
                Ok(state) => {
                    if sender.send(state).await.is_err() {
                        return;
                    }
                }
                // Cada evento carrega o estado inteiro: ficar para trás custa
                // quadros da animação, não informação. Abortar o stream aqui
                // era o que congelava a barra de progresso na tela até o fim
                // da varredura.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
    let stream = ReceiverStream::new(receiver).map(|state| {
        Ok::<Event, Infallible>(
            Event::default().data(serde_json::to_string(&state).unwrap_or_else(|_| "{}".into())),
        )
    });
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

async fn scan(
    State(ctx): State<AppContext>,
    Json(input): Json<DiscoveryScanInput>,
) -> AppResult<Response> {
    let network = networks::Entity::find_by_id(input.network_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Rede não encontrada"))?;
    let range = parse_cidr_range(&network.cidr).map_err(|_| {
        AppError::business_rule(format!(
            "A rede \"{}\" não tem uma faixa CIDR varredurável (valor atual: \"{}\").",
            network.name, network.cidr
        ))
    })?;
    let run = discovery_runs::ActiveModel {
        network_id: Set(network.id),
        probe_id: Set(network.probe_id),
        status: Set("running".into()),
        started_at: Set(Utc::now().into()),
        configuration: Set(Some(serde_json::json!({ "cidr": network.cidr }))),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    let session = ScanSessionService::from_context(&ctx)?;
    let cancel = session.start(run.id, network.id).await;
    let cidr = network.cidr.clone();
    let ctx_clone = ctx.clone();
    let session_clone = session.clone();
    let run_id = run.id;
    tokio::spawn(async move {
        match run_discovery(&ctx_clone, &cidr, run_id, cancel.clone()).await {
            Ok(_) => {
                let _ = discovery_runs::ActiveModel {
                    id: Set(run_id),
                    status: Set("completed".into()),
                    finished_at: Set(Some(Utc::now().into())),
                    ..Default::default()
                }
                .update(&ctx_clone.db)
                .await;
                session_clone.finish(None).await;
            }
            Err(error) => {
                let message = error.to_string();
                // Cancelamento não é falha: a run interrompida pelo operador
                // precisa aparecer como `cancelled` na lista (§7.7), senão a
                // tela mostra erro onde houve uma decisão deliberada.
                let status = if cancel.is_cancelled() {
                    "cancelled"
                } else {
                    "failed"
                };
                let _ = discovery_runs::ActiveModel {
                    id: Set(run_id),
                    status: Set(status.into()),
                    finished_at: Set(Some(Utc::now().into())),
                    error: Set(Some(message.clone())),
                    ..Default::default()
                }
                .update(&ctx_clone.db)
                .await;
                session_clone.finish(Some(message)).await;
            }
        }
    });
    Ok((axum::http::StatusCode::ACCEPTED, axum::Json(serde_json::json!({ "runId": run.id, "status": "running", "usableHosts": range.usable_hosts, "truncated": range.truncated }))).into_response())
}

async fn scan_cancel(State(ctx): State<AppContext>) -> AppResult<Response> {
    ScanSessionService::from_context(&ctx)?.cancel().await;
    Ok(format::json(serde_json::json!({ "status": "cancelled" }))?)
}

async fn runs(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = discovery_runs::Entity::find()
        .order_by_desc(crate::models::_entities::discovery_runs::Column::Id)
        .all(&ctx.db)
        .await?;
    let mut data = Vec::with_capacity(rows.len());
    for run in rows {
        let found = discovery_results::Entity::find()
            .filter(crate::models::_entities::discovery_results::Column::DiscoveryRunId.eq(run.id))
            .count(&ctx.db)
            .await?;
        data.push(serde_json::json!({ "id":run.id, "networkId":run.network_id, "probeId":run.probe_id, "status":run.status, "devicesFound":found, "cidr":run.configuration.as_ref().and_then(|v| v.get("cidr")).and_then(serde_json::Value::as_str), "startedAt":run.started_at.to_rfc3339(), "finishedAt":run.finished_at.map(|v| v.to_rfc3339()), "error":run.error }));
    }
    Ok(format::json(data)?)
}

async fn run_details(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let run = discovery_runs::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Execução de discovery não encontrada"))?;
    let results = discovery_results::Entity::find()
        .filter(crate::models::_entities::discovery_results::Column::DiscoveryRunId.eq(id))
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        serde_json::json!({ "id": run.id, "networkId":run.network_id, "status":run.status, "configuration":run.configuration, "results":results }),
    )?)
}

async fn cleanup(
    State(ctx): State<AppContext>,
    Query(query): Query<BTreeMap<String, String>>,
) -> AppResult<Response> {
    let days = query
        .get("olderThanDays")
        .and_then(|raw| raw.parse::<i64>().ok())
        .unwrap_or(7)
        .max(1);
    let result = discovery_runs::Entity::delete_many()
        .filter(
            crate::models::_entities::discovery_runs::Column::CreatedAt
                .lt(Utc::now() - Duration::days(days)),
        )
        .exec(&ctx.db)
        .await?;
    Ok(format::json(
        serde_json::json!({ "removedRuns": result.rows_affected }),
    )?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/discovery")
        .add("/scan-state", get(scan_state))
        .add("/scan-stream", get(scan_stream))
        .add("/scan", post(scan))
        .add("/scan-cancel", post(scan_cancel))
        .add("/runs", get(runs))
        .add("/runs/{id}", get(run_details))
        .add("/cleanup", delete(cleanup))
}
