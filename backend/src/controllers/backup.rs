//! Backup do próprio NetMonitor.
//!
//! Dois caminhos para o mesmo arquivo:
//!
//! * **plano** (`/backup/system`) — a cópia vai para um destino, sozinha ou
//!   pelo "Fazer backup agora", e volta de lá pela restauração;
//! * **arquivo** (`/backup/export|preview|restore`) — o operador baixa ou envia
//!   o JSON pelo navegador.
//!
//! O corpo do arquivo trafega como JSON comum — o que o operador salva é a
//! própria resposta do `export`, e é ela que volta no `restore`. Não há
//! `multipart`: evita uma dependência de parsing só para reembrulhar um JSON
//! que já está pronto, e deixa o endpoint utilizável por `curl`.
//!
//! Só administrador chega aqui (`users::request_is_allowed`).

use axum::{
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::{app::Hooks, prelude::*};

use crate::{
    app::App,
    dtos::backup::{SystemBackupCopiesQuery, SystemBackupCopyInput, SystemBackupPlanInput},
    services::{
        audit::{self, AuditAction, ResourceType},
        backup::{
            copies::{self, RUN_SCOPE},
            plan::{self, PLAN_ID},
            service::{self, BackupFile},
        },
        shared::{
            errors::{AppError, AppResult},
            run_guard::RunGuard,
        },
        storage,
    },
    views::backup::{
        BackupCountsResponse, SystemBackupCopyResponse, SystemBackupPlanResponse,
        SystemBackupRunResponse,
    },
};

/// Como a auditoria nomeia o recurso.
const RESOURCE: (i64, &str) = (PLAN_ID, "NetMonitor");

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
    Ok(format::json(BackupCountsResponse::from(counts))?)
}

/// Substitui a configuração atual pela do arquivo.
async fn restore(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(file): Json<BackupFile>,
) -> AppResult<Response> {
    let counts = service::restore(&ctx.db, &file).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Execute,
        ResourceType::Backup,
        RESOURCE,
        "Configuração do NetMonitor restaurada a partir de um arquivo enviado",
    )
    .await;
    Ok((StatusCode::OK, Json(BackupCountsResponse::from(counts))).into_response())
}

// --- Plano ---------------------------------------------------------------------

async fn plan_response(ctx: &AppContext) -> AppResult<SystemBackupPlanResponse> {
    let row = plan::get(&ctx.db).await?;
    let storage_name = match row.storage_destination_id {
        Some(id) => storage::service::find(&ctx.db, id)
            .await
            .ok()
            .map(|destination| destination.name),
        None => None,
    };
    Ok(SystemBackupPlanResponse::new(
        &row,
        storage_name,
        RunGuard::is_running(RUN_SCOPE, PLAN_ID),
    ))
}

async fn show_plan(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(plan_response(&ctx).await?)?)
}

async fn update_plan(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<SystemBackupPlanInput>,
) -> AppResult<Response> {
    plan::update(&ctx.db, input.into()).await?;
    plan::publish_updated(&ctx).await;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Update,
        ResourceType::Backup,
        RESOURCE,
        "Plano de backup do NetMonitor alterado",
    )
    .await;
    Ok(format::json(plan_response(&ctx).await?)?)
}

/// "Fazer backup agora". Síncrono: a configuração é pequena e a cópia leva
/// segundos — a resposta já traz o arquivo criado.
async fn run_now(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let current = plan::get(&ctx.db).await?;
    let destination_id = current.storage_destination_id.ok_or_else(|| {
        AppError::validation("Escolha para onde vão as cópias do NetMonitor antes de fazer backup")
    })?;
    let destination = storage::service::load(&ctx.db, destination_id).await?;
    let result = copies::run(&ctx.db, &current, &destination, App::app_version()).await;
    // O resultado — inclusive a falha — já foi gravado no plano: as outras
    // abas precisam ver o novo status nos dois casos.
    plan::publish_updated(&ctx).await;
    let outcome = result?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Execute,
        ResourceType::Backup,
        RESOURCE,
        format!(
            "Backup do NetMonitor '{}' enviado para '{}'",
            outcome.entry.name, destination.row.name
        ),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(SystemBackupRunResponse::new(outcome, destination.row.name)),
    )
        .into_response())
}

/// O destino de onde ler: o pedido ou, sem ele, o do plano.
async fn source_destination(
    ctx: &AppContext,
    requested: Option<i64>,
) -> AppResult<storage::service::Destination> {
    let id = match requested {
        Some(id) => id,
        None => plan::get(&ctx.db)
            .await?
            .storage_destination_id
            .ok_or_else(|| AppError::validation("Escolha de qual destino listar as cópias"))?,
    };
    storage::service::load(&ctx.db, id).await
}

async fn list_copies(
    State(ctx): State<AppContext>,
    Query(query): Query<SystemBackupCopiesQuery>,
) -> AppResult<Response> {
    let destination = source_destination(&ctx, query.storage_destination_id).await?;
    let entries = copies::list(&destination).await?;
    Ok(format::json(
        entries
            .into_iter()
            .map(SystemBackupCopyResponse::from)
            .collect::<Vec<_>>(),
    )?)
}

async fn preview_copy(
    State(ctx): State<AppContext>,
    Json(input): Json<SystemBackupCopyInput>,
) -> AppResult<Response> {
    let destination = source_destination(&ctx, Some(input.storage_destination_id)).await?;
    let counts = copies::preview(&destination, &input.key).await?;
    Ok(format::json(BackupCountsResponse::from(counts))?)
}

async fn restore_copy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<SystemBackupCopyInput>,
) -> AppResult<Response> {
    let destination = source_destination(&ctx, Some(input.storage_destination_id)).await?;
    let counts = copies::restore(&ctx.db, &destination, &input.key).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Execute,
        ResourceType::Backup,
        RESOURCE,
        format!(
            "Configuração do NetMonitor restaurada a partir de '{}' em '{}'",
            storage::leaf_name(&input.key),
            destination.row.name
        ),
    )
    .await;
    Ok(format::json(BackupCountsResponse::from(counts))?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/backup")
        .add("/export", get(export))
        .add("/preview", post(preview))
        .add("/restore", post(restore))
        .add("/system", get(show_plan).put(update_plan))
        .add("/system/run", post(run_now))
        .add("/system/copies", get(list_copies))
        .add("/system/copies/preview", post(preview_copy))
        .add("/system/copies/restore", post(restore_copy))
}
