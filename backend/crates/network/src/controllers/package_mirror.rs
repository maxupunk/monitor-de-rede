//! Espelho de pacotes do OpenWrt (ADR 015 §10): o `apk`/`opkg` do roteador
//! baixa os índices e pacotes oficiais pela central.
//!
//! **Sem JWT**: quem chama é o gerenciador de pacotes do roteador, que não tem
//! sessão. A autorização é o token emitido para a execução do plugin, conferido
//! no serviço junto com a política de caminhos.

use axum::{
    body::Body,
    http::{header, StatusCode},
    response::IntoResponse,
};
use futures::StreamExt;
use loco_rs::prelude::*;

use crate::package_mirror::{self, fetch::MAX_BYTES};
use netmonitor_core::shared::errors::AppResult;

async fn openwrt(Path((token, path)): Path<(String, String)>) -> AppResult<Response> {
    let url = package_mirror::authorize(&token, &path)?;
    let upstream = package_mirror::fetch(&url).await?;
    let mut headers = vec![(header::CONTENT_TYPE, upstream.content_type)];
    if let Some(length) = upstream.content_length {
        headers.push((header::CONTENT_LENGTH, length.to_string()));
    }
    let mut streamed: u64 = 0;
    let capped = upstream
        .response
        .bytes_stream()
        .map(move |chunk| match chunk {
            Ok(bytes) => {
                streamed = streamed.saturating_add(bytes.len() as u64);
                if streamed > MAX_BYTES {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::FileTooLarge,
                        "arquivo excedeu o limite do espelho de pacotes",
                    ))
                } else {
                    Ok(bytes)
                }
            }
            Err(error) => Err(std::io::Error::other(error)),
        });
    let mut response = (StatusCode::OK, Body::from_stream(capped)).into_response();
    for (name, value) in headers {
        if let Ok(value) = value.parse() {
            response.headers_mut().insert(name, value);
        }
    }
    Ok(response)
}

/// Rotas do espelho — **sem** JWT (ver o topo do módulo).
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/package-mirror")
        .add("/openwrt/{token}/{*path}", get(openwrt))
}
