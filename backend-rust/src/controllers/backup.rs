//! Exportação e restauração das configurações do sistema.
//!
//! O corpo trafega como JSON comum — o arquivo que o operador salva é a própria
//! resposta do `export`, e é ela que volta no `restore`. Não há `multipart`:
//! evita uma dependência de parsing só para reembrulhar um JSON que já está
//! pronto, e deixa o endpoint utilizável por `curl` sem cerimônia.

use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};
use loco_rs::{app::Hooks, prelude::*};

use crate::{
    app::App,
    services::{
        backup::service::{self, BackupFile, TableCounts},
        shared::errors::AppResult,
    },
};

/// `{"tables": [{"table": "...", "rows": n}], "totalRows": n}`.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CountsResponse {
    tables: Vec<TableCount>,
    total_rows: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TableCount {
    table: String,
    rows: usize,
}

impl From<TableCounts> for CountsResponse {
    fn from(counts: TableCounts) -> Self {
        Self {
            total_rows: counts.iter().map(|(_, rows)| rows).sum(),
            tables: counts
                .into_iter()
                .map(|(table, rows)| TableCount { table, rows })
                .collect(),
        }
    }
}

/// Baixa o backup como anexo.
///
/// O `Content-Disposition` traz um nome com carimbo de data para o operador não
/// acumular meia dúzia de `backup.json (3)` na pasta de downloads.
async fn export(State(ctx): State<AppContext>) -> AppResult<Response> {
    let file = service::export(&ctx.db, <App as Hooks>::app_version()).await?;
    let filename = format!(
        "netmonitor-backup-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    Ok((
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )],
        Json(file),
    )
        .into_response())
}

/// Lê o arquivo e diz o que ele contém, sem escrever nada.
async fn preview(Json(file): Json<BackupFile>) -> AppResult<Response> {
    let counts = service::inspect(&file)?;
    Ok(format::json(CountsResponse::from(counts))?)
}

/// Substitui a configuração atual pela do arquivo.
async fn restore(
    State(ctx): State<AppContext>,
    Json(file): Json<BackupFile>,
) -> AppResult<Response> {
    let counts = service::restore(&ctx.db, &file).await?;
    Ok((StatusCode::OK, Json(CountsResponse::from(counts))).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/backup")
        .add("/export", get(export))
        .add("/preview", post(preview))
        .add("/restore", post(restore))
}
