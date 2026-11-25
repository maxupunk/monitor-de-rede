//! Envio de blobs: monolítico (`POST ?digest=`) ou em partes (`POST` →
//! `PATCH`… → `PUT ?digest=`), retomável pelo `GET` do status, e a montagem
//! entre repositórios (`POST ?mount=&from=`).
//!
//! As partes vão para um arquivo da área de montagem; no `PUT` o arquivo
//! inteiro é conferido contra o digest antes de virar blob (RS-05). O
//! deslocamento fica no banco, então o envio sobrevive a uma reinicialização.

use std::{
    collections::HashMap,
    io::SeekFrom,
    sync::{Arc, Mutex as StdMutex, OnceLock},
};

use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use futures::StreamExt;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
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

/// Serializa PATCH/PUT/DELETE concorrentes sobre o mesmo envio em andamento.
fn upload_locks() -> &'static StdMutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>> {
    static LOCKS: OnceLock<StdMutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// A vez de um envio. Ao sair (por qualquer caminho, inclusive erro ou
/// sessão inexistente), tira a entrada do mapa se ninguém mais a espera: o
/// mapa só guarda envios com requisição em andamento.
struct UploadTurn {
    id: Uuid,
    guard: Option<tokio::sync::OwnedMutexGuard<()>>,
}

impl Drop for UploadTurn {
    fn drop(&mut self) {
        let mut map = upload_locks()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(self.guard.take());
        if map
            .get(&self.id)
            .is_some_and(|lock| Arc::strong_count(lock) == 1)
        {
            map.remove(&self.id);
        }
    }
}

async fn upload_turn(id: Uuid) -> UploadTurn {
    let lock = {
        let mut map = upload_locks()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.entry(id)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    UploadTurn {
        id,
        guard: Some(lock.lock_owned().await),
    }
}

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

async fn update_hasher(mut hasher: Hasher, chunk: bytes::Bytes) -> OciResult<Hasher> {
    if chunk.len() < 16 * 1024 {
        hasher.update(&chunk);
        return Ok(hasher);
    }
    tokio::task::spawn_blocking(move || {
        hasher.update(&chunk);
        hasher
    })
    .await
    .map_err(OciError::internal)
}

/// Grava o corpo a partir de `expected_offset`; em falha no meio, trunca de
/// volta para `expected_offset` para não corromper o envio. Com `hash = true`,
/// calcula o SHA-256 em uma passada só (envio monolítico).
async fn write_at(
    path: &std::path::Path,
    expected_offset: u64,
    body: Body,
    room: u64,
    hash: bool,
) -> OciResult<(u64, Option<Digest>)> {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .await
        .map_err(OciError::internal)?;
    file.set_len(expected_offset)
        .await
        .map_err(OciError::internal)?;
    file.seek(SeekFrom::Start(expected_offset))
        .await
        .map_err(OciError::internal)?;
    let mut written: u64 = 0;
    let mut hasher = hash.then(Hasher::new);
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(_) => {
                let _ = file.set_len(expected_offset).await;
                return Err(OciError::new(
                    StatusCode::BAD_REQUEST,
                    Code::BlobUploadInvalid,
                    "envio interrompido",
                ));
            }
        };
        written = written.saturating_add(chunk.len() as u64);
        if written > room {
            let _ = file.set_len(expected_offset).await;
            return Err(OciError::new(
                StatusCode::INSUFFICIENT_STORAGE,
                Code::Denied,
                "o registry está sem espaço na cota: libere versões antigas ou aumente a cota",
            ));
        }
        if let Err(error) = file.write_all(&chunk).await {
            let _ = file.set_len(expected_offset).await;
            return Err(OciError::internal(error));
        }
        if let Some(current) = hasher.take() {
            hasher = Some(update_hasher(current, chunk).await?);
        }
    }
    if let Err(error) = file.flush().await {
        let _ = file.set_len(expected_offset).await;
        return Err(OciError::internal(error));
    }
    Ok((written, hasher.map(Hasher::finish)))
}

async fn hash_file(path: std::path::PathBuf) -> OciResult<(Digest, u64)> {
    tokio::task::spawn_blocking(move || -> std::io::Result<(Digest, u64)> {
        use std::io::Read as _;
        let mut file = std::fs::File::open(&path)?;
        let mut hasher = Hasher::new();
        let mut buffer = vec![0_u8; 256 * 1024];
        let mut size: u64 = 0;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            size += count as u64;
            hasher.update(&buffer[..count]);
        }
        Ok((hasher.finish(), size))
    })
    .await
    .map_err(OciError::internal)?
    .map_err(OciError::internal)
}

/// Confere o arquivo inteiro (fora do executor assíncrono) e o torna blob do
/// repositório. Quando `verified` vem (envio monolítico), pula a releitura.
async fn commit(
    ctx: &AppContext,
    repository_id: i64,
    path: &std::path::Path,
    expected: &Digest,
    verified: Option<(Digest, u64)>,
) -> OciResult<()> {
    let (actual, size) = match verified {
        Some(pair) => pair,
        None => hash_file(path.to_path_buf()).await?,
    };
    if &actual != expected {
        return Err(OciError::new(
            StatusCode::BAD_REQUEST,
            Code::DigestInvalid,
            "o conteúdo não confere com o digest",
        ));
    }
    // Do arquivo ao vínculo, a coleta não apaga este blob.
    let _push = storage::push_gate().read().await;
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
                    let _push = storage::push_gate().read().await;
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
        let (size, actual) = write_at(&staged.path, 0, body, room(ctx).await?, true).await?;
        let verified = actual.map(|actual| (actual, size));
        commit(ctx, repo.id, &staged.path, &digest, verified).await?;
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
    if let Err(error) = (registry_uploads::ActiveModel {
        id: Set(id.simple().to_string()),
        repository_id: Set(repo.id),
        offset: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
    })
    .insert(&ctx.db)
    .await
    {
        let _ = tokio::fs::remove_file(&staged.path).await;
        return Err(OciError::internal(error));
    }
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
    let uuid = Uuid::parse_str(id).map_err(|_| {
        OciError::new(
            StatusCode::NOT_FOUND,
            Code::BlobUploadUnknown,
            "envio desconhecido",
        )
    })?;
    let _turn = upload_turn(uuid).await;
    let (upload, uuid, staged) = session(ctx, name, id).await?;
    check_range(headers, upload.offset)?;
    let expected_offset = u64::try_from(upload.offset).unwrap_or(0);
    let room = room(ctx).await?.saturating_sub(expected_offset);
    let (written, _) = write_at(&staged.path, expected_offset, body, room, false).await?;
    let offset = advance(ctx, upload, written).await?;
    Ok(accepted(name, &uuid, offset))
}

pub(super) async fn finish(
    ctx: &AppContext,
    headers: &HeaderMap,
    name: &str,
    id: &str,
    query: &str,
    body: Body,
) -> OciResult<Response> {
    let uuid = Uuid::parse_str(id).map_err(|_| {
        OciError::new(
            StatusCode::NOT_FOUND,
            Code::BlobUploadUnknown,
            "envio desconhecido",
        )
    })?;
    let _turn = upload_turn(uuid).await;
    let (upload, _, mut staged) = session(ctx, name, id).await?;
    check_range(headers, upload.offset)?;
    let digest = parse_digest(&query_value(query, "digest").unwrap_or_default())?;
    let expected_offset = u64::try_from(upload.offset).unwrap_or(0);
    let room = room(ctx).await?.saturating_sub(expected_offset);
    write_at(&staged.path, expected_offset, body, room, false).await?;
    let repository_id = upload.repository_id;
    if let Err(error) = commit(ctx, repository_id, &staged.path, &digest, None).await {
        if tokio::fs::try_exists(&staged.path).await.unwrap_or(true) {
            // Restaura o tamanho ao último offset confirmado para manter o
            // envio íntegro.
            if let Ok(file) = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&staged.path)
                .await
            {
                let _ = file.set_len(expected_offset).await;
            }
        } else {
            // O arquivo já virou blob e o vínculo falhou: o envio não tem
            // mais o que retomar (o offset apontaria para um arquivo que não
            // existe). O cliente recomeça; o blob órfão sai na coleta.
            let _ = registry_uploads::Entity::delete_by_id(upload.id)
                .exec(&ctx.db)
                .await;
        }
        return Err(error);
    }
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
    let uuid = Uuid::parse_str(id).map_err(|_| {
        OciError::new(
            StatusCode::NOT_FOUND,
            Code::BlobUploadUnknown,
            "envio desconhecido",
        )
    })?;
    let _turn = upload_turn(uuid).await;
    let (upload, _, staged) = session(ctx, name, id).await?;
    let _ = tokio::fs::remove_file(&staged.path).await;
    registry_uploads::Entity::delete_by_id(upload.id)
        .exec(&ctx.db)
        .await
        .map_err(OciError::internal)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked(id: &Uuid) -> bool {
        upload_locks()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(id)
    }

    #[tokio::test]
    async fn a_vez_do_envio_sai_do_mapa_quando_ninguem_espera() {
        let id = Uuid::new_v4();
        let first = upload_turn(id).await;
        assert!(tracked(&id));
        let waiter = tokio::spawn(async move {
            let _turn = upload_turn(id).await;
        });
        // O segundo pedido já pegou o mutex e espera a vez.
        while upload_locks()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .is_some_and(|lock| Arc::strong_count(lock) < 3)
        {
            tokio::task::yield_now().await;
        }
        drop(first);
        assert!(tracked(&id), "quem espera continua no mesmo mutex");
        waiter.await.unwrap();
        assert!(!tracked(&id), "sem ninguém esperando, a entrada sai");
    }
}
