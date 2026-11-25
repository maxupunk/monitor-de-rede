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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RangeSpec {
    None,
    Valid(u64, u64),
    Unsatisfiable,
}

/// `bytes=início-fim` (fim opcional, inclusivo) ou o sufixo `bytes=-N`.
///
/// O que não dá para atender como uma faixa só (sintaxe inválida, outra
/// unidade, várias faixas, blob vazio) é ignorado e vira `200` com o blob
/// inteiro, como manda a RFC 9110 (§14.2); `416` só quando a faixa começa
/// depois do fim.
fn range(headers: &HeaderMap, size: u64) -> RangeSpec {
    let Some(raw) = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().strip_prefix("bytes="))
    else {
        return RangeSpec::None;
    };
    if raw.contains(',') {
        return RangeSpec::None;
    }
    let Some((start, end)) = raw.split_once('-') else {
        return RangeSpec::None;
    };
    let Some(last) = size.checked_sub(1) else {
        return RangeSpec::None;
    };
    let (start, end) = (start.trim(), end.trim());
    if start.is_empty() {
        return match end.parse::<u64>() {
            Ok(0) => RangeSpec::Unsatisfiable,
            Ok(suffix) => RangeSpec::Valid(size.saturating_sub(suffix), last),
            Err(_) => RangeSpec::None,
        };
    }
    let Ok(start) = start.parse::<u64>() else {
        return RangeSpec::None;
    };
    let end = match end {
        "" => last,
        value => match value.parse::<u64>() {
            Ok(parsed) if parsed >= start => parsed.min(last),
            _ => return RangeSpec::None,
        },
    };
    if start > last {
        RangeSpec::Unsatisfiable
    } else {
        RangeSpec::Valid(start, end)
    }
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
        RangeSpec::Valid(start, end) => (
            StatusCode::PARTIAL_CONTENT,
            start,
            end - start + 1,
            Some(format!("bytes {start}-{end}/{size}")),
        ),
        RangeSpec::None => (StatusCode::OK, 0, size, None),
        RangeSpec::Unsatisfiable => {
            return Err(OciError::new(
                StatusCode::RANGE_NOT_SATISFIABLE,
                Code::SizeInvalid,
                "faixa de bytes inválida",
            )
            .with_header(header::CONTENT_RANGE, format!("bytes */{size}")));
        }
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
        assert_eq!(range(&headers, 100), RangeSpec::Valid(10, 19));
        headers.insert(header::RANGE, "bytes=90-".parse().unwrap());
        assert_eq!(range(&headers, 100), RangeSpec::Valid(90, 99));
        headers.insert(header::RANGE, "bytes=150-200".parse().unwrap());
        assert_eq!(range(&headers, 100), RangeSpec::Unsatisfiable);
        headers.insert(header::RANGE, "bytes=100-".parse().unwrap());
        assert_eq!(range(&headers, 100), RangeSpec::Unsatisfiable);
        assert_eq!(range(&HeaderMap::new(), 100), RangeSpec::None);
    }

    #[test]
    fn faixas_que_a_rfc_manda_ignorar_ou_atender() {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, "bytes=-30".parse().unwrap());
        assert_eq!(range(&headers, 100), RangeSpec::Valid(70, 99));
        headers.insert(header::RANGE, "bytes=-500".parse().unwrap());
        assert_eq!(range(&headers, 100), RangeSpec::Valid(0, 99));
        headers.insert(header::RANGE, "bytes=-0".parse().unwrap());
        assert_eq!(range(&headers, 100), RangeSpec::Unsatisfiable);
        for ignored in [
            "bytes=50-10",
            "bytes=0-1,5-9",
            "items=0-1",
            "bytes=x-",
            "bytes=5",
        ] {
            headers.insert(header::RANGE, ignored.parse().unwrap());
            assert_eq!(range(&headers, 100), RangeSpec::None, "{ignored}");
        }
        headers.insert(header::RANGE, "bytes=0-10".parse().unwrap());
        assert_eq!(range(&headers, 0), RangeSpec::None, "blob vazio");
    }
}
