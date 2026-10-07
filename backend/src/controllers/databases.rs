//! Bancos de dados: conexões, backup sob demanda, histórico e restauração.
//!
//! Só administrador chega aqui (`users::request_is_allowed`). Backup e
//! restauração respondem `202` com o andamento inicial; o resto vem pelo SSE
//! (`database_jobs:updated`).

use std::collections::HashMap;

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;

use crate::{
    dtos::databases::{DatabaseConnectionInput, DatabaseProbeInput, DatabaseRestoreInput},
    models::database_connections,
    services::{
        audit::{AuditAction, ResourceType},
        databases::{
            backups::{self, Trigger},
            service, DatabaseEngine,
        },
        shared::errors::AppResult,
        storage,
    },
    views::databases::{DatabaseBackupResponse, DatabaseConnectionResponse, DatabaseProbeResponse},
};

/// Registra uma ação sobre uma conexão de banco.
async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    resource: (i64, &str),
    description: String,
) {
    crate::services::audit::record(
        &ctx.db,
        headers,
        action,
        ResourceType::DatabaseConnection,
        resource,
        description,
    )
    .await;
}

/// Nome de cada armazenamento, para a linha mostrar para onde vão as cópias.
async fn storage_names(ctx: &AppContext) -> AppResult<HashMap<i64, String>> {
    Ok(storage::service::list(&ctx.db)
        .await?
        .into_iter()
        .map(|row| (row.id, row.name))
        .collect())
}

fn respond(
    row: &database_connections::Model,
    names: &HashMap<i64, String>,
) -> DatabaseConnectionResponse {
    let storage_name = row
        .storage_destination_id
        .and_then(|id| names.get(&id).cloned());
    DatabaseConnectionResponse::new(row, storage_name)
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let names = storage_names(&ctx).await?;
    let rows = service::list(&ctx.db).await?;
    Ok(format::json(
        rows.iter()
            .map(|row| respond(row, &names))
            .collect::<Vec<_>>(),
    )?)
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = service::find(&ctx.db, id).await?;
    Ok(format::json(respond(&row, &storage_names(&ctx).await?))?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DatabaseConnectionInput>,
) -> AppResult<Response> {
    let row = service::create(&ctx.db, input.into()).await?;
    service::publish_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (row.id, &row.name),
        format!(
            "Conexão de banco '{}' ({}) cadastrada",
            row.name,
            DatabaseEngine::parse(&row.engine).map_or("?", DatabaseEngine::label)
        ),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(respond(&row, &storage_names(&ctx).await?)),
    )
        .into_response())
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<DatabaseConnectionInput>,
) -> AppResult<Response> {
    let row = service::update(&ctx.db, id, input.into()).await?;
    service::publish_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (row.id, &row.name),
        format!("Conexão de banco '{}' atualizada", row.name),
    )
    .await;
    Ok(format::json(respond(&row, &storage_names(&ctx).await?))?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let row = service::delete(&ctx.db, id).await?;
    service::publish_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (row.id, &row.name),
        format!(
            "Conexão de banco '{}' removida (as cópias no armazenamento foram mantidas)",
            row.name
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn probe(
    State(ctx): State<AppContext>,
    Json(input): Json<DatabaseProbeInput>,
) -> AppResult<Response> {
    let probe = service::probe(&ctx.db, input.into()).await?;
    Ok(format::json(DatabaseProbeResponse::from(probe))?)
}

async fn history(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    service::find(&ctx.db, id).await?;
    let rows = backups::history(&ctx.db, id).await?;
    Ok(format::json(
        rows.into_iter()
            .map(DatabaseBackupResponse::from)
            .collect::<Vec<_>>(),
    )?)
}

async fn run_backup(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let job = backups::start_backup(&ctx, id, Trigger::Manual).await?;
    let row = service::find(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (row.id, &row.name),
        format!("Backup manual da conexão '{}' iniciado", row.name),
    )
    .await;
    Ok((StatusCode::ACCEPTED, Json(job)).into_response())
}

async fn restore(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(backup_id): Path<i64>,
    Json(input): Json<DatabaseRestoreInput>,
) -> AppResult<Response> {
    let request = input.into_request(backup_id)?;
    let database = request.database.clone();
    let mode = request.mode;
    let target = request.target_connection_id;
    let job = backups::start_restore(&ctx, request).await?;
    let row = service::find(&ctx.db, target).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (row.id, &row.name),
        format!(
            "Restauração da cópia #{backup_id} em '{database}' ({}) da conexão '{}' iniciada",
            match mode {
                crate::services::databases::RestoreMode::NewDatabase => "banco novo",
                crate::services::databases::RestoreMode::Replace => "substituindo o existente",
            },
            row.name
        ),
    )
    .await;
    Ok((StatusCode::ACCEPTED, Json(job)).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/databases")
        .add("/", get(index).post(store))
        .add("/probe", post(probe))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/backups", get(history).post(run_backup))
        .add("/backups/{backup_id}/restore", post(restore))
}
