//! Do arquivo OCI que o agente compilou ao registry (ADR 023, RF-04a/b).
//!
//! O arquivo chega em fluxo: cada `blobs/sha256/<hex>` vai para a área de
//! montagem enquanto o hash é calculado e só vira blob se o hash bater com o
//! nome. O `index.json` diz qual é a imagem; os manifests (que também são
//! blobs do arquivo) entram no catálogo de baixo para cima — filhos antes do
//! índice —, e a imagem ganha as tags pedidas. A origem gravada é a do
//! pedido de compilação, nunca algo que o arquivo diga sobre si.

use std::{collections::HashMap, io};

use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use netmonitor_core::shared::errors::{AppError, AppResult};

use crate::{
    catalog::{self, Push},
    digest::{Digest, Hasher},
    manifest::{self, MAX_MANIFEST_BYTES},
    storage::{BlobStore, Staged},
    tarstream,
};
use sea_orm::DatabaseConnection;

/// Profundidade máxima de índices dentro de índices.
const MAX_DEPTH: usize = 4;

fn io_error(error: io::Error) -> AppError {
    AppError::validation(format!("O arquivo da imagem chegou quebrado: {error}"))
}

#[derive(Debug, Deserialize)]
struct Layout {
    manifests: Vec<LayoutEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutEntry {
    digest: String,
}

/// O que veio no arquivo.
struct Unpacked {
    /// digest → tamanho, já no storage.
    blobs: HashMap<Digest, u64>,
    top: Vec<Digest>,
}

/// Lê o tar e guarda os blobs conferidos.
async fn unpack<R: AsyncRead + Unpin>(
    store: &dyn BlobStore,
    archive: R,
    limit: u64,
) -> AppResult<Unpacked> {
    let mut reader = tarstream::Reader::new(archive);
    let mut blobs = HashMap::new();
    let mut index = None;
    let mut total: u64 = 0;
    let mut buffer = vec![0_u8; 256 * 1024];
    while let Some(entry) = reader.next().await.map_err(io_error)? {
        if !entry.regular {
            continue;
        }
        if entry.path == "index.json" {
            index = Some(
                reader
                    .read_all(MAX_MANIFEST_BYTES as u64)
                    .await
                    .map_err(io_error)?,
            );
            continue;
        }
        let Some(hex) = entry.path.strip_prefix("blobs/sha256/") else {
            continue;
        };
        let expected: Digest = format!("sha256:{hex}")
            .parse()
            .map_err(|_| AppError::validation(format!("Blob com nome inválido: {}", entry.path)))?;
        total = total.saturating_add(entry.size);
        if total > limit {
            return Err(AppError::business_rule(
                "A imagem não cabe na cota do registry: libere espaço em Registry ou aumente a cota",
            ));
        }
        let mut staged = Staged::new(store).await.map_err(io_error)?;
        let mut file = tokio::fs::File::create(&staged.path)
            .await
            .map_err(io_error)?;
        let mut hasher = Hasher::new();
        loop {
            let count = reader.read(&mut buffer).await.map_err(io_error)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
            file.write_all(&buffer[..count]).await.map_err(io_error)?;
        }
        file.flush().await.map_err(io_error)?;
        drop(file);
        if hasher.finish() != expected {
            return Err(AppError::validation(format!(
                "O conteúdo de {expected} não confere com o digest"
            )));
        }
        store
            .commit(&staged.path, &expected)
            .await
            .map_err(io_error)?;
        staged.keep();
        blobs.insert(expected, entry.size);
    }
    let index = index.ok_or_else(|| {
        AppError::validation("O arquivo não é uma imagem OCI (falta o index.json)")
    })?;
    let layout: Layout = serde_json::from_slice(&index)
        .map_err(|error| AppError::validation(format!("index.json inválido: {error}")))?;
    let top = layout
        .manifests
        .iter()
        .map(|entry| entry.digest.parse::<Digest>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::validation("index.json com digest inválido"))?;
    if top.is_empty() {
        return Err(AppError::validation("index.json sem imagem"));
    }
    Ok(Unpacked { blobs, top })
}

/// O texto de um manifest guardado como blob.
async fn read_text(store: &dyn BlobStore, digest: &Digest) -> AppResult<String> {
    let mut reader = store.open(digest, 0).await.map_err(io_error)?;
    let mut data = Vec::new();
    (&mut reader)
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut data)
        .await
        .map_err(io_error)?;
    if data.len() > MAX_MANIFEST_BYTES {
        return Err(AppError::validation("Manifest grande demais"));
    }
    String::from_utf8(data).map_err(|_| AppError::validation("Manifest não é UTF-8"))
}

/// Grava o manifest `digest` e, antes, os que ele referencia.
async fn put_tree(
    db: &DatabaseConnection,
    store: &dyn BlobStore,
    repository_id: i64,
    digest: &Digest,
    push: &Push,
    depth: usize,
) -> AppResult<()> {
    if depth > MAX_DEPTH {
        return Err(AppError::validation("Índices aninhados demais"));
    }
    let content = read_text(store, digest).await?;
    let parsed = manifest::parse(content.as_bytes(), None)
        .map_err(|error| AppError::validation(error.to_string()))?;
    for child in parsed.children() {
        Box::pin(put_tree(
            db,
            store,
            repository_id,
            &child.digest,
            push,
            depth + 1,
        ))
        .await?;
    }
    catalog::put_manifest(db, repository_id, &content, &parsed, push).await?;
    Ok(())
}

/// Guarda o arquivo em `repository` com as `tags`. Devolve o digest da
/// imagem (o índice, quando há attestations).
///
/// # Errors
///
/// Arquivo inválido, digest que não confere, cota, erro do banco.
pub async fn ingest<R: AsyncRead + Unpin>(
    db: &DatabaseConnection,
    store: &dyn BlobStore,
    repository_id: i64,
    archive: R,
    tags: &[String],
    push: &Push,
    room: u64,
) -> AppResult<Digest> {
    let unpacked = unpack(store, archive, room).await?;
    for (digest, size) in &unpacked.blobs {
        catalog::link_blob(db, repository_id, digest, *size).await?;
    }
    for digest in &unpacked.top {
        put_tree(db, store, repository_id, digest, push, 0).await?;
    }
    let top = unpacked.top[0].clone();
    for tag in tags {
        catalog::set_tag(db, repository_id, tag, &top).await?;
    }
    Ok(top)
}
