//! `GET`/`HEAD`/`DELETE /v2/<name>/blobs/<digest>`, com `Range` no pull.

use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;
use tokio::io::AsyncReadExt;

use super::{existing, Code, OciError, OciResult};
use crate::{catalog, digest::Digest, storage};

pub(super) fn parse_digest(raw: &str) -> OciResult<Digest> {
    raw.parse().map_err(|_| {
        OciError::new(
            StatusCode::BAD_REQUEST,
            Code::DigestInvalid,
            "digest inválido",
        )
    })
}

/// `bytes=início-fim` (fim opcional, inclusivo).
fn range(headers: &HeaderMap, size: u64) -> Option<(u64, u64)> {
    let raw = headers
        .get(header::RANGE)?
        .to_str()
        .ok()?
        .strip_prefix("bytes=")?;
    let (start, end) = raw.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = match end.trim() {
        "" => size.checked_sub(1)?,
        value => value.parse::<u64>().ok()?.min(size.checked_sub(1)?),
    };
    (start <= end).then_some((start, end))
}

pub(super) async fn get(
    ctx: &AppContext,
    headers: &HeaderMap,
    name: &str,
    digest: &str,
    head: bool,
) -> OciResult<Response> {
    let digest = parse_digest(digest)?;
    let repo = existing(ctx, name).await?;
    let size = catalog::linked_blob(&ctx.db, repo.id, &digest)
        .await
        .map_err(OciError::internal)?
        .ok_or_else(|| {
            OciError::new(
                StatusCode::NOT_FOUND,
                Code::BlobUnknown,
                "blob desconhecido",
            )
        })?;
    let common = [
        (header::CONTENT_TYPE, "application/octet-stream".to_string()),
        (
            header::HeaderName::from_static("docker-content-digest"),
            digest.to_string(),
        ),
        (header::ACCEPT_RANGES, "bytes".to_string()),
    ];
    let (status, offset, length, content_range) = match range(headers, size) {
        Some((start, end)) => (
            StatusCode::PARTIAL_CONTENT,
            start,
            end - start + 1,
            Some(format!("bytes {start}-{end}/{size}")),
        ),
        None => (StatusCode::OK, 0, size, None),
    };
    let mut response = if head {
        (status, Body::empty()).into_response()
    } else {
        let reader = storage::store()
            .open(&digest, offset)
            .await
            .map_err(OciError::internal)?;
        let stream = tokio_util::io::ReaderStream::new(reader.take(length));
        (status, Body::from_stream(stream)).into_response()
    };
    let response_headers = response.headers_mut();
    for (name, value) in common {
        if let Ok(value) = value.parse() {
            response_headers.insert(name, value);
        }
    }
    if let Ok(value) = length.to_string().parse() {
        response_headers.insert(header::CONTENT_LENGTH, value);
    }
    if let Some(content_range) = content_range.and_then(|value| value.parse().ok()) {
        response_headers.insert(header::CONTENT_RANGE, content_range);
    }
    Ok(response)
}

/// Tira o blob do repositório (o arquivo sai na coleta, se ninguém mais o usa).
pub(super) async fn delete(ctx: &AppContext, name: &str, digest: &str) -> OciResult<Response> {
    let digest = parse_digest(digest)?;
    let repo = existing(ctx, name).await?;
    if catalog::unlink_blob(&ctx.db, repo.id, &digest)
        .await
        .map_err(OciError::internal)?
    {
        Ok(StatusCode::ACCEPTED.into_response())
    } else {
        Err(OciError::new(
            StatusCode::NOT_FOUND,
            Code::BlobUnknown,
            "blob desconhecido",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faixas() {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=10-19".parse().unwrap());
        assert_eq!(range(&headers, 100), Some((10, 19)));
        headers.insert(header::RANGE, "bytes=90-".parse().unwrap());
        assert_eq!(range(&headers, 100), Some((90, 99)));
        headers.insert(header::RANGE, "bytes=50-10".parse().unwrap());
        assert_eq!(range(&headers, 100), None);
    }
}
