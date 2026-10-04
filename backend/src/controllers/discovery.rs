//! Discovery: disparo e cancelamento da varredura, histórico e conflitos.
//!
//! O estado ao vivo da varredura não tem rota própria: viaja no stream global
//! (`discovery:scan`, ver `services::discovery::service`).

use axum::response::IntoResponse;
use loco_rs::prelude::*;
use std::collections::BTreeMap;

use crate::{
    dtos::resources::DiscoveryScanInput,
    services::{
        discovery::{conflicts, service},
        shared::errors::AppResult,
    },
};

async fn scan(
    State(ctx): State<AppContext>,
    Json(input): Json<DiscoveryScanInput>,
) -> AppResult<Response> {
    let started = service::start_scan(&ctx, input.network_id).await?;
    Ok((axum::http::StatusCode::ACCEPTED, axum::Json(started)).into_response())
}

async fn scan_cancel(State(ctx): State<AppContext>) -> AppResult<Response> {
    service::cancel_scan(&ctx).await?;
    Ok(format::json(serde_json::json!({ "status": "cancelled" }))?)
}

async fn runs(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(service::list_runs(&ctx.db).await?)?)
}

async fn run_details(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(service::run_details(&ctx.db, id).await?)?)
}

async fn cleanup(
    State(ctx): State<AppContext>,
    Query(query): Query<BTreeMap<String, String>>,
) -> AppResult<Response> {
    let days = query
        .get("olderThanDays")
        .and_then(|raw| raw.parse::<i64>().ok())
        .unwrap_or(7);
    let removed = service::cleanup_runs(&ctx.db, days).await?;
    Ok(format::json(serde_json::json!({ "removedRuns": removed }))?)
}

async fn environment() -> AppResult<Response> {
    let is_host_mode = crate::services::monitoring::ip_reconciliation::can_inspect_l2();
    let detector = crate::services::syslog::nat::NatDetector::detect();
    Ok(format::json(serde_json::json!({
        "isHostMode": is_host_mode,
        "containerized": detector.containerized,
    }))?)
}

async fn list_conflicts(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(
        conflicts::analyze_network_conflicts(&ctx.db).await?,
    )?)
}

async fn check_conflicts(State(ctx): State<AppContext>) -> AppResult<Response> {
    let list = conflicts::analyze_network_conflicts(&ctx.db).await?;
    if !list.is_empty() {
        conflicts::alert_on_conflicts(&ctx, &list).await?;
    }
    Ok(format::json(serde_json::json!({
        "count": list.len(),
        "conflicts": list,
    }))?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/discovery")
        .add("/scan", post(scan))
        .add("/scan-cancel", post(scan_cancel))
        .add("/runs", get(runs))
        .add("/runs/{id}", get(run_details))
        .add("/cleanup", delete(cleanup))
        .add("/environment", get(environment))
        .add("/conflicts", get(list_conflicts))
        .add("/check-conflicts", post(check_conflicts))
}
