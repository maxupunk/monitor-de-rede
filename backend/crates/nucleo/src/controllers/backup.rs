//! Backup do próprio NetMonitor.
//!
//! Dois caminhos para o mesmo arquivo:
//!
//! * **plano** (`/backup/system`) — a cópia vai para um destino, sozinha ou
//!   pelo "Fazer backup agora", e volta de lá pela restauração;
//! * **arquivo** (`/backup/export|full|preview|restore`) — o operador baixa ou
//!   envia o arquivo pelo navegador: a configuração (`.json`) ou os bancos
//!   inteiros (`/backup/full`, `.json.gz`).
//!
//! O corpo do arquivo trafega cru — o que o operador salva é a própria
//! resposta do `export`/`full`, e é ela que volta no `preview`/`restore`, lida
//! em fluxo (sem limite de tamanho e sem carregá-la na memória). Não há
//! `multipart`: deixa o endpoint utilizável por `curl --data-binary`.
//!
//! Só administrador chega aqui (`users::request_is_allowed`).

use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use futures::TryStreamExt;
use loco_rs::prelude::*;
use tokio_util::io::StreamReader;

use crate::{
    dtos::backup::{SystemBackupCopiesQuery, SystemBackupCopyInput, SystemBackupPlanInput},
    services::{
        audit::{self, AuditAction, ResourceType},
        backup::{
            archive,
            catalog::BackupScope,
            copies::{self, RUN_SCOPE},
            plan::{self, PLAN_ID},
            restore as file_restore, schedule, service,
        },
        shared::{
            errors::{AppError, AppResult},
            run_guard::RunGuard,
        },
        storage,
        syslog::db::LogsDb,
    },
    views::backup::{
        BackupCountsResponse, BackupRestoreResponse, SystemBackupCopyResponse,
        SystemBackupPlanResponse, SystemBackupRunResponse,
    },
};

/// Como a auditoria nomeia o recurso.
const RESOURCE: (i64, &str) = (PLAN_ID, "NetMonitor");

/// Baixa o backup como anexo.
///
/// O `Content-Disposition` traz um nome com carimbo de data para o operador não
/// acumular meia dúzia de `backup.json (3)` na pasta de downloads.
async fn export(State(ctx): State<AppContext>) -> AppResult<Response> {
    let file = service::export(&ctx.db, crate::app_version()).await?;
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

/// Baixa os bancos inteiros — principal e de logs — num `.json.gz` só.
///
/// A resposta é um fluxo: o servidor lê cada tabela em pedaços e comprime no
/// caminho, sem montar o arquivo na memória. Sem `Content-Length` (o tamanho
/// só se sabe no fim); a tela mostra quanto já chegou.
async fn export_full(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let logs = LogsDb::from_context(&ctx).ok().map(|logs| logs.0);
    let stream = archive::export_full(ctx.db.clone(), logs, crate::app_version())?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Execute,
        ResourceType::Backup,
        RESOURCE,
        "Backup completo do NetMonitor baixado (banco principal e de logs)",
    )
    .await;
    let filename = format!(
        "netmonitor-backup-completo-{}.json.gz",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from_stream(stream))
        .map_err(|err| AppError::Internal(err.into()))
}

/// O corpo da requisição como leitura em fluxo.
fn body_reader(body: Body) -> impl tokio::io::AsyncRead + Send + Unpin + 'static {
    StreamReader::new(body.into_data_stream().map_err(std::io::Error::other))
}

/// Lê o arquivo e diz o que ele contém, sem escrever nada.
async fn preview(body: Body) -> AppResult<Response> {
    let preview = file_restore::inspect(body_reader(body)).await?;
    Ok(format::json(BackupCountsResponse::from(preview))?)
}

/// Substitui a configuração atual — ou, com o backup completo, os bancos
/// inteiros — pelo que vem no arquivo.
async fn restore(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: Body,
) -> AppResult<Response> {
    let logs = LogsDb::from_context(&ctx).ok();
    let session = headers
        .get(crate::controllers::auth_guard::AUTHENTICATED_USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let restored = file_restore::restore(
        &ctx.db,
        logs.as_ref().map(LogsDb::connection),
        body_reader(body),
        session.as_deref(),
    )
    .await?;
    let what = match restored.scope {
        BackupScope::Config => "Configuração do NetMonitor restaurada a partir de um arquivo enviado",
        BackupScope::Full => {
            "Bancos do NetMonitor (completo, com usuários e histórico) restaurados a partir de um arquivo enviado"
        }
    };
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Execute,
        ResourceType::Backup,
        RESOURCE,
        what,
    )
    .await;
    Ok((StatusCode::OK, Json(BackupRestoreResponse::from(restored))).into_response())
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
    let outcome = schedule::run_plan(&ctx, &current, &destination, "manual").await?;
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
        .add("/full", get(export_full))
        .add("/preview", post(preview))
        .add("/restore", post(restore))
        .add("/system", get(show_plan).put(update_plan))
        .add("/system/run", post(run_now))
        .add("/system/copies", get(list_copies))
        .add("/system/copies/preview", post(preview_copy))
        .add("/system/copies/restore", post(restore_copy))
}
