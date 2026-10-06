//! Armazenamentos: cadastro, teste, explorador e backups guardados.
//!
//! Só administrador chega aqui (`users::request_is_allowed`): o cadastro
//! carrega credenciais de nuvem, e restaurar substitui a configuração inteira.

use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::{app::Hooks, prelude::*};

use crate::{
    app::App,
    dtos::storages::{
        StorageBackupKeyInput, StorageBrowseQuery, StorageDestinationInput, StorageObjectQuery,
        StorageTestInput,
    },
    services::{
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        shared::errors::AppResult,
        storage::{assert_deletable, backups, normalize_path, service},
    },
    views::{
        backup::BackupCountsResponse,
        storages::{
            StorageBackupResponse, StorageBackupRunResponse, StorageBrowseResponse,
            StorageDestinationDetail, StorageDestinationResponse, StorageTestResponse,
            StoragesMetaResponse,
        },
    },
};

async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    resource: (i64, &str),
    description: String,
) {
    let actor = AuditActor::from_headers(headers, &ctx.db)
        .await
        .unwrap_or_default();
    let _ = AuditService::new(&ctx.db)
        .log(
            actor,
            AuditEntryInput {
                action,
                resource_type: ResourceType::Storage,
                resource_id: Some(resource.0),
                resource_label: Some(resource.1.to_owned()),
                description: Some(description),
                changes: None,
            },
        )
        .await;
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = service::list(&ctx.db).await?;
    let list: Vec<StorageDestinationResponse> = rows
        .into_iter()
        .map(|row| match service::open(row.clone()) {
            Ok(destination) => StorageDestinationResponse::new(&row, Some(&destination.config)),
            // Credencial ilegível não pode esconder o destino da lista: é na
            // lista que o operador descobre que precisa reinformá-la.
            Err(_) => StorageDestinationResponse::new(&row, None),
        })
        .collect();
    Ok(format::json(list)?)
}

async fn meta() -> AppResult<Response> {
    Ok(format::json(StoragesMetaResponse::current())?)
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    Ok(format::json(StorageDestinationDetail::from(&destination))?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<StorageDestinationInput>,
) -> AppResult<Response> {
    let row = service::create(&ctx.db, input.into()).await?;
    let destination = service::open(row)?;
    service::publish_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (destination.row.id, &destination.row.name),
        format!(
            "Armazenamento '{}' ({}) cadastrado",
            destination.row.name,
            destination.provider.label()
        ),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(StorageDestinationDetail::from(&destination)),
    )
        .into_response())
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StorageDestinationInput>,
) -> AppResult<Response> {
    let row = service::update(&ctx.db, id, input.into()).await?;
    let destination = service::open(row)?;
    service::publish_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (destination.row.id, &destination.row.name),
        format!("Armazenamento '{}' atualizado", destination.row.name),
    )
    .await;
    Ok(format::json(StorageDestinationDetail::from(&destination))?)
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
            "Armazenamento '{}' removido (os arquivos no destino foram mantidos)",
            row.name
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn test_draft(
    State(ctx): State<AppContext>,
    Json(input): Json<StorageTestInput>,
) -> AppResult<Response> {
    let outcome = service::test_draft(&ctx.db, input.id, input.provider, input.config).await?;
    Ok(format::json(StorageTestResponse::from(outcome))?)
}

async fn test_saved(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let outcome = service::test_saved(&ctx.db, id).await?;
    Ok(format::json(StorageTestResponse::from(outcome))?)
}

async fn browse(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<StorageBrowseQuery>,
) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    let path = normalize_path(&query.path);
    let page = destination
        .explorer()?
        .list_objects(&path, &query.options())
        .await?;
    Ok(format::json(StorageBrowseResponse::new(path, page))?)
}

async fn download(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<StorageObjectQuery>,
) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    let key = normalize_path(&query.key);
    let reader = destination.explorer()?.read_object(&key).await?;
    // Aspas e quebras de linha no nome quebrariam o cabeçalho.
    let filename: String = crate::services::storage::leaf_name(&key)
        .chars()
        .filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n'))
        .collect();
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(r#"attachment; filename="{filename}""#),
            ),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(reader)),
    )
        .into_response())
}

async fn delete_object(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<StorageObjectQuery>,
) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    let key = assert_deletable(&query.key)?;
    destination
        .explorer()?
        .delete_object(&key, query.directory)
        .await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (destination.row.id, &destination.row.name),
        format!(
            "{} '{key}' excluído do armazenamento '{}'",
            if query.directory { "Pasta" } else { "Arquivo" },
            destination.row.name
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn list_backups(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    let entries = backups::list(&destination).await?;
    Ok(format::json(
        entries
            .into_iter()
            .map(StorageBackupResponse::from)
            .collect::<Vec<_>>(),
    )?)
}

async fn run_backup(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    let result = backups::run(&ctx.db, &destination, App::app_version()).await;
    // O resultado — inclusive a falha — já foi gravado no cadastro: a lista
    // aberta em outras abas precisa ver o novo status nos dois casos.
    service::publish_updated(&ctx).await;
    let outcome = result?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (destination.row.id, &destination.row.name),
        format!(
            "Backup '{}' enviado ao armazenamento '{}'",
            outcome.entry.name, destination.row.name
        ),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(StorageBackupRunResponse::from(outcome)),
    )
        .into_response())
}

async fn preview_backup(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<StorageBackupKeyInput>,
) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    let counts = backups::preview(&destination, &input.key).await?;
    Ok(format::json(BackupCountsResponse::from(counts))?)
}

async fn restore_backup(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StorageBackupKeyInput>,
) -> AppResult<Response> {
    let destination = service::load(&ctx.db, id).await?;
    let counts = backups::restore(&ctx.db, &destination, &input.key).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (destination.row.id, &destination.row.name),
        format!(
            "Configuração restaurada a partir de '{}' do armazenamento '{}'",
            crate::services::storage::leaf_name(&input.key),
            destination.row.name
        ),
    )
    .await;
    Ok(format::json(BackupCountsResponse::from(counts))?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/storages")
        .add("/", get(index).post(store))
        .add("/meta", get(meta))
        .add("/test", post(test_draft))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/test", post(test_saved))
        .add("/{id}/browse", get(browse))
        .add("/{id}/download", get(download))
        .add("/{id}/objects", delete(delete_object))
        .add("/{id}/backups", get(list_backups).post(run_backup))
        .add("/{id}/backups/preview", post(preview_backup))
        .add("/{id}/backups/restore", post(restore_backup))
}
