//! Tarefas agendadas: catálogo, conferência, cadastro, pausa, "rodar agora"
//! e histórico.
//!
//! Extrai, valida, delega (`services::scheduled_tasks`) e serializa.
//! Leitura é de todos; escrita é de `operator`/`admin` pela regra padrão de
//! `users::request_is_allowed` — e o serviço confere de novo o papel de quem
//! assina, porque é com ele que a tarefa vai rodar.

use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
};
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    dtos::resources::PaginationQuery,
    services::{
        audit::AuditActor,
        scheduled_tasks::{
            model::{
                ScheduledTaskInput, ScheduledTaskOrigin, ScheduledTaskPreviewInput,
                ScheduledTaskStarted, ScheduledTaskTrigger,
            },
            runner, service, Author,
        },
        shared::errors::AppResult,
    },
};

/// `PUT /api/scheduled-tasks/{id}/enabled`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnabledInput {
    enabled: bool,
}

async fn author(ctx: &AppContext, headers: &HeaderMap) -> AppResult<Author> {
    let actor = AuditActor::from_headers(headers, &ctx.db).await?;
    Author::load(&ctx.db, actor, ScheduledTaskOrigin::User).await
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(service::list(&ctx.db).await?)?)
}

async fn catalog() -> AppResult<Response> {
    Ok(format::json(service::catalog())?)
}

async fn preview(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ScheduledTaskPreviewInput>,
) -> AppResult<Response> {
    let author = author(&ctx, &headers).await?;
    Ok(format::json(
        service::preview(&ctx, &input, Some(&author)).await?,
    )?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ScheduledTaskInput>,
) -> AppResult<Response> {
    let author = author(&ctx, &headers).await?;
    let created = service::create(&ctx, &input, &author).await?;
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(service::get(&ctx.db, id).await?)?)
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ScheduledTaskInput>,
) -> AppResult<Response> {
    let author = author(&ctx, &headers).await?;
    Ok(format::json(
        service::update(&ctx, id, &input, &author).await?,
    )?)
}

async fn set_enabled(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<EnabledInput>,
) -> AppResult<Response> {
    let author = author(&ctx, &headers).await?;
    Ok(format::json(
        service::set_enabled(&ctx, id, input.enabled, &author).await?,
    )?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let author = author(&ctx, &headers).await?;
    service::delete(&ctx, id, &author).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Roda agora, à parte: a tela acompanha pelo SSE.
async fn run(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let author = author(&ctx, &headers).await?;
    let task = service::find(&ctx.db, id).await?;
    let (run_id, _) = runner::start(
        &ctx,
        task,
        ScheduledTaskTrigger::Manual,
        Some(author.user_id),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(ScheduledTaskStarted { run_id })).into_response())
}

async fn runs(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    Ok(format::json(
        service::runs(
            &ctx.db,
            id,
            query.page.unwrap_or(1),
            query.limit.unwrap_or(20),
        )
        .await?,
    )?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/scheduled-tasks")
        .add("/", get(index).post(store))
        .add("/catalog", get(catalog))
        .add("/preview", post(preview))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/enabled", put(set_enabled))
        .add("/{id}/run", post(run))
        .add("/{id}/runs", get(runs))
}
