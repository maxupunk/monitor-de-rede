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
use loco_rs::prelude::*;

use crate::services::{package_mirror, shared::errors::AppResult};

async fn openwrt(Path((token, path)): Path<(String, String)>) -> AppResult<Response> {
    let url = package_mirror::authorize(&token, &path)?;
    let upstream = package_mirror::fetch(&url).await?;
    let mut headers = vec![(header::CONTENT_TYPE, upstream.content_type)];
    if let Some(length) = upstream.content_length {
        headers.push((header::CONTENT_LENGTH, length.to_string()));
    }
    let mut response = (
        StatusCode::OK,
        Body::from_stream(upstream.response.bytes_stream()),
    )
        .into_response();
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
