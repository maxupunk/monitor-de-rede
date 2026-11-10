//! Envio de blobs: monolítico (`POST ?digest=`) ou em partes (`POST` →
//! `PATCH`… → `PUT ?digest=`), retomável pelo `GET` do status, e a montagem
//! entre repositórios (`POST ?mount=&from=`).
//!
//! As partes vão para um arquivo da área de montagem; no `PUT` o arquivo
//! inteiro é conferido contra o digest antes de virar blob (RS-05). O
//! deslocamento fica no banco, então o envio sobrevive a uma reinicialização.

use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use futures::StreamExt;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use netmonitor_core::models::registry_uploads;

use super::{authorize, blobs::parse_digest, existing, query_value, Code, OciError, OciResult};
use crate::{
    catalog,
    digest::{Digest, Hasher},
    settings,
    storage::{self, Staged},
    tokens::Action,
};

fn location(name: &str, id: &Uuid) -> String {
    format!("/v2/{name}/blobs/uploads/{}", id.simple())
}

fn accepted(name: &str, id: &Uuid, offset: i64) -> Response {
    let mut response = StatusCode::ACCEPTED.into_response();
    let headers = response.headers_mut();
    for (key, value) in [
        (header::LOCATION, location(name, id)),
        (header::RANGE, format!("0-{}", (offset - 1).max(0))),
        (
            header::HeaderName::from_static("docker-upload-uuid"),
            id.simple().to_string(),
        ),
        (header::CONTENT_LENGTH, "0".to_string()),
    ] {
        if let Ok(value) = value.parse() {
            headers.insert(key, value);
        }
    }
    response
}

fn created(name: &str, digest: &Digest) -> Response {
    let mut response = StatusCode::CREATED.into_response();
    let headers = response.headers_mut();
    for (key, value) in [
        (header::LOCATION, format!("/v2/{name}/blobs/{digest}")),
        (
            header::HeaderName::from_static("docker-content-digest"),
            digest.to_string(),
        ),
        (header::CONTENT_LENGTH, "0".to_string()),
    ] {
        if let Ok(value) = value.parse() {
            headers.insert(key, value);
        }
    }
    response
}

/// Grava o corpo no fim do arquivo; devolve quantos bytes entraram.
async fn append(path: &std::path::Path, body: Body, room: u64) -> OciResult<u64> {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .map_err(OciError::internal)?;
    let mut written: u64 = 0;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            OciError::new(
                StatusCode::BAD_REQUEST,
                Code::BlobUploadInvalid,
                "envio interrompido",
            )
        })?;
        written += chunk.len() as u64;
        if written > room {
            return Err(OciError::new(
                StatusCode::INSUFFICIENT_STORAGE,
                Code::Denied,
                "o registry está sem espaço na cota: libere versões antigas ou aumente a cota",
            ));
        }
        file.write_all(&chunk).await.map_err(OciError::internal)?;
    }
    file.flush().await.map_err(OciError::internal)?;
    Ok(written)
}

/// Confere o arquivo inteiro e o torna blob do repositório.
async fn commit(
    ctx: &AppContext,
    repository_id: i64,
    path: &std::path::Path,
    expected: &Digest,
) -> OciResult<()> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(OciError::internal)?;
    let mut hasher = Hasher::new();
    let mut buffer = vec![0_u8; 256 * 1024];
    let mut size: u64 = 0;
    loop {
        let count = file.read(&mut buffer).await.map_err(OciError::internal)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        hasher.update(&buffer[..count]);
    }
    if &hasher.finish() != expected {
        return Err(OciError::new(
            StatusCode::BAD_REQUEST,
            Code::DigestInvalid,
            "o conteúdo não confere com o digest",
        ));
    }
    storage::store()
        .commit(path, expected)
        .await
        .map_err(OciError::internal)?;
    catalog::link_blob(&ctx.db, repository_id, expected, size)
        .await
        .map_err(OciError::internal)
}

async fn room(ctx: &AppContext) -> OciResult<u64> {
    settings::room(&ctx.db).await.map_err(OciError::internal)
}

pub(super) async fn start(
    ctx: &AppContext,
    headers: &HeaderMap,
    name: &str,
    query: &str,
    body: Body,
) -> OciResult<Response> {
    let repo = catalog::ensure_repository(&ctx.db, name, None)
        .await
        .map_err(OciError::from_app)?;

    // Montagem: o blob já está em outro repositório que quem pede pode ler.
    if let (Some(mount), Some(from)) = (query_value(query, "mount"), query_value(query, "from")) {
        let digest = parse_digest(&mount)?;
        if authorize(headers, &from, Action::Pull).is_ok() {
            if let Some(source) = catalog::repository(&ctx.db, &from)
                .await
                .map_err(OciError::internal)?
            {
                if let Some(size) = catalog::linked_blob(&ctx.db, source.id, &digest)
                    .await
                    .map_err(OciError::internal)?
                {
                    catalog::link_blob(&ctx.db, repo.id, &digest, size)
                        .await
                        .map_err(OciError::internal)?;
                    return Ok(created(name, &digest));
                }
            }
        }
        // Sem montagem possível: abre um envio comum (a especificação manda).
    }

    if let Some(digest) = query_value(query, "digest") {
        let digest = parse_digest(&digest)?;
        let mut staged = Staged::new(storage::store().as_ref())
            .await
            .map_err(OciError::internal)?;
        append(&staged.path, body, room(ctx).await?).await?;
        commit(ctx, repo.id, &staged.path, &digest).await?;
        staged.keep();
        return Ok(created(name, &digest));
    }

    let id = Uuid::new_v4();
    let staged = Staged::upload(storage::store().as_ref(), &id)
        .await
        .map_err(OciError::internal)?;
    tokio::fs::File::create(&staged.path)
        .await
        .map_err(OciError::internal)?;
    let now = Utc::now().into();
    registry_uploads::ActiveModel {
        id: Set(id.simple().to_string()),
        repository_id: Set(repo.id),
        offset: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&ctx.db)
    .await
    .map_err(OciError::internal)?;
    Ok(accepted(name, &id, 0))
}

async fn session(
    ctx: &AppContext,
    name: &str,
    id: &str,
) -> OciResult<(registry_uploads::Model, Uuid, Staged)> {
    let unknown = || {
        OciError::new(
            StatusCode::NOT_FOUND,
            Code::BlobUploadUnknown,
            "envio desconhecido",
        )
    };
    let uuid = Uuid::parse_str(id).map_err(|_| unknown())?;
    let repo = existing(ctx, name).await?;
    let upload = registry_uploads::Entity::find_by_id(uuid.simple().to_string())
        .one(&ctx.db)
        .await
        .map_err(OciError::internal)?
        .filter(|upload| upload.repository_id == repo.id)
        .ok_or_else(unknown)?;
    let staged = Staged::upload(storage::store().as_ref(), &uuid)
        .await
        .map_err(OciError::internal)?;
    Ok((upload, uuid, staged))
}

/// `Content-Range: início-fim` precisa começar onde o envio parou.
fn check_range(headers: &HeaderMap, offset: i64) -> OciResult<()> {
    let Some(raw) = headers
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
    else {
        return Ok(());
    };
    let start = raw
        .trim_start_matches("bytes ")
        .split('-')
        .next()
        .and_then(|start| start.trim().parse::<i64>().ok());
    if start == Some(offset) {
        Ok(())
    } else {
        Err(OciError::new(
            StatusCode::RANGE_NOT_SATISFIABLE,
            Code::BlobUploadInvalid,
            format!("a parte precisa começar em {offset}"),
        ))
    }
}

async fn advance(
    ctx: &AppContext,
    upload: registry_uploads::Model,
    written: u64,
) -> OciResult<i64> {
    let offset = upload.offset + i64::try_from(written).unwrap_or(i64::MAX);
    let mut active: registry_uploads::ActiveModel = upload.into();
    active.offset = Set(offset);
    active.updated_at = Set(Utc::now().into());
    active.update(&ctx.db).await.map_err(OciError::internal)?;
    Ok(offset)
}

pub(super) async fn patch(
    ctx: &AppContext,
    headers: &HeaderMap,
    name: &str,
    id: &str,
    body: Body,
) -> OciResult<Response> {
    let (upload, uuid, staged) = session(ctx, name, id).await?;
    check_range(headers, upload.offset)?;
    let room = room(ctx)
        .await?
        .saturating_sub(u64::try_from(upload.offset).unwrap_or(0));
    let written = append(&staged.path, body, room).await?;
    let offset = advance(ctx, upload, written).await?;
    Ok(accepted(name, &uuid, offset))
}

pub(super) async fn finish(
    ctx: &AppContext,
    name: &str,
    id: &str,
    query: &str,
    body: Body,
) -> OciResult<Response> {
    let (upload, _uuid, mut staged) = session(ctx, name, id).await?;
    let digest = parse_digest(&query_value(query, "digest").unwrap_or_default())?;
    let room = room(ctx)
        .await?
        .saturating_sub(u64::try_from(upload.offset).unwrap_or(0));
    append(&staged.path, body, room).await?;
    let repository_id = upload.repository_id;
    commit(ctx, repository_id, &staged.path, &digest).await?;
    staged.keep();
    registry_uploads::Entity::delete_by_id(upload.id)
        .exec(&ctx.db)
        .await
        .map_err(OciError::internal)?;
    Ok(created(name, &digest))
}

pub(super) async fn status(ctx: &AppContext, name: &str, id: &str) -> OciResult<Response> {
    let (upload, uuid, _staged) = session(ctx, name, id).await?;
    let mut response = accepted(name, &uuid, upload.offset);
    *response.status_mut() = StatusCode::NO_CONTENT;
    Ok(response)
}

pub(super) async fn cancel(ctx: &AppContext, name: &str, id: &str) -> OciResult<Response> {
    let (upload, _uuid, staged) = session(ctx, name, id).await?;
    let _ = tokio::fs::remove_file(&staged.path).await;
    registry_uploads::Entity::delete_by_id(upload.id)
        .exec(&ctx.db)
        .await
        .map_err(OciError::internal)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
