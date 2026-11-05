//! Destinos (armazenamentos): cadastro, teste e explorador de arquivos.
//!
//! O que vai para cada destino é dos planos de backup (`/backup/system`,
//! `/databases`). Só administrador chega aqui (`users::request_is_allowed`):
//! o cadastro carrega credenciais de nuvem.

use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;

use crate::{
    dtos::storages::{
        StorageBrowseQuery, StorageDestinationInput, StorageObjectQuery, StorageTestInput,
    },
    services::{
        audit::{AuditAction, ResourceType},
        shared::errors::AppResult,
        storage::{assert_deletable, normalize_path, service},
    },
    views::storages::{
        StorageBrowseResponse, StorageDestinationDetail, StorageDestinationResponse,
        StorageTestResponse, StoragesMetaResponse,
    },
};

/// Registra uma ação sobre um destino.
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
        ResourceType::Storage,
        resource,
        description,
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
            "Destino '{}' ({}) cadastrado",
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
        format!("Destino '{}' atualizado", destination.row.name),
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
            "Destino '{}' removido (os arquivos nele foram mantidos)",
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
            "{} '{key}' excluído do destino '{}'",
            if query.directory { "Pasta" } else { "Arquivo" },
            destination.row.name
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
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
}
