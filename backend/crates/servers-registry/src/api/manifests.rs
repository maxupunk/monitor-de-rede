//! Manifests, tags e referrers.

use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures::StreamExt;
use loco_rs::app::AppContext;
use serde_json::{json, Value};

use super::{blobs::parse_digest, existing, query_value, Code, OciError, OciResult};
use netmonitor_servers::deploys::images::OriginKind;

use crate::{
    catalog::{self, Push},
    digest::Digest,
    events,
    manifest::{self, MAX_MANIFEST_BYTES, OCI_INDEX},
    names, scan,
};

fn with_digest(
    mut response: Response,
    digest: &str,
    media_type: Option<&str>,
    length: usize,
) -> Response {
    let headers = response.headers_mut();
    let mut pairs = vec![
        (
            header::HeaderName::from_static("docker-content-digest"),
            digest.to_string(),
        ),
        (header::CONTENT_LENGTH, length.to_string()),
    ];
    if let Some(media_type) = media_type {
        pairs.push((header::CONTENT_TYPE, media_type.to_string()));
    }
    for (key, value) in pairs {
        if let Ok(value) = value.parse() {
            headers.insert(key, value);
        }
    }
    response
}

pub(super) async fn get(
    ctx: &AppContext,
    name: &str,
    reference: &str,
    head: bool,
) -> OciResult<Response> {
    let repo = existing(ctx, name).await?;
    let found = catalog::resolve(&ctx.db, repo.id, reference)
        .await
        .map_err(OciError::internal)?
        .ok_or_else(|| {
            OciError::new(
                StatusCode::NOT_FOUND,
                Code::ManifestUnknown,
                "manifest desconhecido",
            )
        })?;
    let length = found.content.len();
    let body = if head {
        Body::empty()
    } else {
        Body::from(found.content)
    };
    Ok(with_digest(
        (StatusCode::OK, body).into_response(),
        &found.digest,
        Some(&found.media_type),
        length,
    ))
}

async fn read_body(body: Body) -> OciResult<Vec<u8>> {
    let mut stream = body.into_data_stream();
    let mut data = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            OciError::new(
                StatusCode::BAD_REQUEST,
                Code::ManifestInvalid,
                "envio interrompido",
            )
        })?;
        data.extend_from_slice(&chunk);
        if data.len() > MAX_MANIFEST_BYTES {
            return Err(OciError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                Code::SizeInvalid,
                "manifest acima de 4 MiB",
            ));
        }
    }
    Ok(data)
}

pub(super) async fn put(
    ctx: &AppContext,
    headers: &HeaderMap,
    name: &str,
    reference: &str,
    body: Body,
    who: &str,
) -> OciResult<Response> {
    let repo = catalog::ensure_repository(&ctx.db, name, None)
        .await
        .map_err(OciError::from_app)?;
    let bytes = read_body(body).await?;
    let digest = Digest::of(&bytes);
    if let Ok(asked) = reference.parse::<Digest>() {
        if asked != digest {
            return Err(OciError::new(
                StatusCode::BAD_REQUEST,
                Code::DigestInvalid,
                "o digest da URL não confere com o manifest",
            ));
        }
    } else if !names::valid_tag(reference) {
        return Err(OciError::new(
            StatusCode::BAD_REQUEST,
            Code::ManifestInvalid,
            "tag inválida",
        ));
    }
    let content = String::from_utf8(bytes).map_err(|_| {
        OciError::new(
            StatusCode::BAD_REQUEST,
            Code::ManifestInvalid,
            "manifest não é UTF-8",
        )
    })?;
    let declared = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let parsed = manifest::parse(content.as_bytes(), declared).map_err(|error| {
        OciError::new(
            StatusCode::BAD_REQUEST,
            Code::ManifestInvalid,
            error.to_string(),
        )
    })?;
    let push = Push {
        pushed_by: Some(who.to_string()),
        provenance: Some(
            json!({ "source": OriginKind::ManualPush, "by": who, "at": chrono::Utc::now().to_rfc3339() }),
        ),
    };
    let stored = catalog::put_manifest(&ctx.db, repo.id, &content, &parsed, &push)
        .await
        .map_err(OciError::from_app)?;
    if reference.parse::<Digest>().is_err() {
        catalog::set_tag(&ctx.db, repo.id, reference, &digest)
            .await
            .map_err(OciError::from_app)?;
        // Imagem nova com tag: analisa em segundo plano.
        scan::schedule(ctx, repo.id, stored.digest.clone());
    }
    events::publish_changed(ctx);
    let mut response = StatusCode::CREATED.into_response();
    let response_headers = response.headers_mut();
    let mut pairs = vec![
        (header::LOCATION, format!("/v2/{name}/manifests/{digest}")),
        (
            header::HeaderName::from_static("docker-content-digest"),
            digest.to_string(),
        ),
    ];
    if let Some(subject) = &parsed.subject {
        pairs.push((
            header::HeaderName::from_static("oci-subject"),
            subject.to_string(),
        ));
    }
    for (key, value) in pairs {
        if let Ok(value) = value.parse() {
            response_headers.insert(key, value);
        }
    }
    Ok(response)
}

pub(super) async fn delete(ctx: &AppContext, name: &str, reference: &str) -> OciResult<Response> {
    let repo = existing(ctx, name).await?;
    let removed = crate::browse::remove_reference(&ctx.db, &repo, reference)
        .await
        .map_err(OciError::from_app)?;
    if removed {
        events::publish_changed(ctx);
        Ok(StatusCode::ACCEPTED.into_response())
    } else {
        Err(OciError::new(
            StatusCode::NOT_FOUND,
            Code::ManifestUnknown,
            "manifest desconhecido",
        ))
    }
}

pub(super) async fn tags(ctx: &AppContext, name: &str, query: &str) -> OciResult<Response> {
    let repo = existing(ctx, name).await?;
    let limit = query_value(query, "n").and_then(|n| n.parse::<u64>().ok());
    let last = query_value(query, "last");
    let list = catalog::tags(&ctx.db, repo.id, limit, last.as_deref())
        .await
        .map_err(OciError::internal)?;
    let mut response = Json(json!({ "name": name, "tags": list })).into_response();
    if let (Some(limit), Some(tail)) = (limit, list.last()) {
        if list.len() as u64 == limit {
            if let Ok(value) =
                format!("</v2/{name}/tags/list?n={limit}&last={tail}>; rel=\"next\"").parse()
            {
                response.headers_mut().insert(header::LINK, value);
            }
        }
    }
    Ok(response)
}

pub(super) async fn referrers(
    ctx: &AppContext,
    name: &str,
    digest: &str,
    query: &str,
) -> OciResult<Response> {
    let subject = parse_digest(digest)?;
    // A especificação proíbe 404 aqui: repositório sem referrers devolve vazio.
    let rows = match catalog::repository(&ctx.db, name)
        .await
        .map_err(OciError::internal)?
    {
        Some(repo) => catalog::referrers(
            &ctx.db,
            repo.id,
            &subject,
            query_value(query, "artifactType").as_deref(),
        )
        .await
        .map_err(OciError::internal)?,
        None => Vec::new(),
    };
    let manifests: Vec<Value> = rows
        .iter()
        .map(|row| {
            let annotations = serde_json::from_str::<Value>(&row.content)
                .ok()
                .and_then(|value| value.get("annotations").cloned());
            let mut descriptor = json!({
                "mediaType": row.media_type,
                "digest": row.digest,
                "size": row.size,
            });
            if let Some(kind) = &row.artifact_type {
                descriptor["artifactType"] = json!(kind);
            }
            if let Some(annotations) = annotations {
                descriptor["annotations"] = annotations;
            }
            descriptor
        })
        .collect();
    let mut response = (
        [(header::CONTENT_TYPE, OCI_INDEX)],
        Json(json!({ "schemaVersion": 2, "mediaType": OCI_INDEX, "manifests": manifests })),
    )
        .into_response();
    if query_value(query, "artifactType").is_some() {
        if let Ok(value) = "artifactType".parse() {
            response.headers_mut().insert(
                header::HeaderName::from_static("oci-filters-applied"),
                value,
            );
        }
    }
    Ok(response)
}
