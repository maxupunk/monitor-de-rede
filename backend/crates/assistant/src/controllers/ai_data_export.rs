//! Exportação dos dados do assistente (conversas, comandos e execuções em
//! equipamentos) de todos os usuários — só para administradores.

use axum::{
    extract::Query,
    http::{header, HeaderMap, HeaderValue},
};
use loco_rs::prelude::*;

use crate::{data_export, dtos::AiDataExportQuery};
use netmonitor_core::{
    audit::require_admin,
    shared::errors::{AppError, AppResult},
};

const FORBIDDEN: &str = "Apenas administradores podem exportar os dados do Assistente IA.";

/// `GET /api/ai/data-export/summary?days=` — o que o pacote levaria.
async fn summary(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<AiDataExportQuery>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, FORBIDDEN).await?;
    let period = data_export::Period::last(query.days, chrono::Utc::now())?;
    Ok(format::json(data_export::summary(&ctx.db, period).await?)?)
}

/// `GET /api/ai/data-export?days=` — o pacote `.zip`, como anexo.
async fn download(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<AiDataExportQuery>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, FORBIDDEN).await?;
    let now = chrono::Utc::now();
    let period = data_export::Period::last(query.days, now)?;
    let export = data_export::export(&ctx.db, period, now).await?;

    let mut response = export.bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::try_from(format!("attachment; filename=\"{}\"", export.filename))
            .map_err(|_| AppError::validation("Nome de arquivo inválido."))?,
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/ai/data-export")
        .add("/", get(download))
        .add("/summary", get(summary))
}
