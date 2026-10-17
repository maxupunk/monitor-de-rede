//! Teste de enlace (ADR 015 §5): plano, execução, cancelamento, histórico e
//! agendamentos.
//!
//! Extrai, valida, delega (`services::link_tests`) e serializa. Leitura é de
//! todos; escrita (rodar, cancelar, agendar — e pedir o plano, que prepara a
//! execução) é de `operator`/`admin` pela regra padrão de
//! `users::request_is_allowed`.

use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
};
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::authenticated_user,
    services::{
        audit::{self, AuditAction, ResourceType},
        link_tests::{
            model::{LinkTestQuery, LinkTestRequest, LinkTestScheduleInput, LinkTestStarted},
            schedules, service, LinkTestEnv,
        },
        shared::errors::AppResult,
    },
};

fn kind_label(request: &LinkTestRequest) -> &'static str {
    match request.kind {
        crate::services::link_tests::model::LinkTestKind::Link => "do enlace",
        crate::services::link_tests::model::LinkTestKind::Path => "do caminho",
        crate::services::link_tests::model::LinkTestKind::Server => "do servidor ao equipamento",
    }
}

async fn plan(
    State(ctx): State<AppContext>,
    Json(request): Json<LinkTestRequest>,
) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    Ok(format::json(service::plan(&env, &request).await?)?)
}

async fn index(
    State(ctx): State<AppContext>,
    Query(query): Query<LinkTestQuery>,
) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    Ok(format::json(service::list(&env, &query).await?)?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(request): Json<LinkTestRequest>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let env = LinkTestEnv::from_context(&ctx);
    let test = service::start_confirmed(env, &request, Some(user.id)).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Execute,
        ResourceType::LinkTest,
        (test.id, &format!("Teste de enlace #{}", test.id)),
        format!(
            "Iniciou o teste {} ({} perna(s), {} s por sentido)",
            kind_label(&request),
            test.legs.len(),
            test.spec.duration_s
        ),
    )
    .await;
    Ok((StatusCode::ACCEPTED, Json(LinkTestStarted { id: test.id })).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    Ok(format::json(service::detail(&env, id).await?)?)
}

async fn cancel(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    let test = service::cancel(&env, id).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Update,
        ResourceType::LinkTest,
        (id, &format!("Teste de enlace #{id}")),
        "Cancelou o teste de enlace",
    )
    .await;
    Ok(format::json(test)?)
}

async fn schedules_index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    Ok(format::json(schedules::list(&env).await?)?)
}

async fn schedules_show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    let row = schedules::find(&env, id).await?;
    Ok(format::json(crate::services::link_tests::view::schedule(
        &row,
    ))?)
}

async fn schedules_store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<LinkTestScheduleInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let env = LinkTestEnv::from_context(&ctx);
    let created = schedules::create(&env, &input, Some(user.id)).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Create,
        ResourceType::LinkTestSchedule,
        (created.id, &created.name),
        format!(
            "Agendou teste de enlace a cada {} minutos",
            created.interval_minutes
        ),
    )
    .await;
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

async fn schedules_update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<LinkTestScheduleInput>,
) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    let updated = schedules::update(&env, id, &input).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Update,
        ResourceType::LinkTestSchedule,
        (updated.id, &updated.name),
        "Editou o agendamento de teste de enlace",
    )
    .await;
    Ok(format::json(updated)?)
}

async fn schedules_destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let env = LinkTestEnv::from_context(&ctx);
    let deleted = schedules::delete(&env, id).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Delete,
        ResourceType::LinkTestSchedule,
        (deleted.id, &deleted.name),
        "Excluiu o agendamento de teste de enlace",
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn schedules_run(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let env = LinkTestEnv::from_context(&ctx);
    let test_id = schedules::run_now(env, id, Some(user.id)).await?;
    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Execute,
        ResourceType::LinkTestSchedule,
        (id, &format!("Agendamento #{id}")),
        format!("Rodou agora o agendamento (teste de enlace #{test_id})"),
    )
    .await;
    Ok((StatusCode::ACCEPTED, Json(LinkTestStarted { id: test_id })).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/link-tests/plan", post(plan))
        .add("/link-tests", get(index).post(store))
        .add("/link-tests/{id}", get(show))
        .add("/link-tests/{id}/cancel", post(cancel))
        .add(
            "/link-test-schedules",
            get(schedules_index).post(schedules_store),
        )
        .add(
            "/link-test-schedules/{id}",
            get(schedules_show)
                .put(schedules_update)
                .delete(schedules_destroy),
        )
        .add("/link-test-schedules/{id}/run", post(schedules_run))
}
