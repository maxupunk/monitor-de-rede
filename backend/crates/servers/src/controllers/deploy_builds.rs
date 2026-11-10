//! Compilação no servidor (ADR 023): fila, "Compilar agora", cancelar, log,
//! segredos de build, como está o builder de um agente e o aviso de push do
//! GitHub App. A regra mora em `deploys::builds`.

use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;
use serde::Deserialize;
use ts_rs::TS;

use netmonitor_core::{
    agents::HostKey,
    audit::{authenticated_user, AuditAction},
    shared::errors::{AppError, AppResult},
};

use super::deploys::audit;
use crate::{
    deploys::{
        applications,
        builds::{self, push, secrets},
        short_sha,
    },
    docker::hosts,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    #[serde(default)]
    application_id: Option<i64>,
}

/// "Compilar agora": a branch (o commit mais novo dela) ou um commit.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuildInput"
)]
pub struct BuildInput {
    /// Branch (`main`); vazia, a do primeiro alvo.
    #[serde(default)]
    #[ts(optional)]
    pub reference: Option<String>,
    /// Commit exato; vazio, o mais novo da branch.
    #[serde(default)]
    #[ts(optional)]
    pub commit: Option<String>,
    #[serde(default)]
    pub no_cache: bool,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuildSecretInput"
)]
pub struct SecretInput {
    pub name: String,
    pub value: String,
}

async fn list(
    State(ctx): State<AppContext>,
    Query(query): Query<ListQuery>,
) -> AppResult<Response> {
    let mut views = Vec::new();
    for build in builds::recent(&ctx.db, query.application_id, 30).await? {
        views.push(builds::view(&ctx.db, &build).await?);
    }
    Ok(format::json(views)?)
}

async fn log(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let build = builds::find(&ctx.db, id).await?;
    Ok(format::json(
        serde_json::json!({ "log": build.log.unwrap_or_default() }),
    )?)
}

async fn start(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<BuildInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let application = applications::find(&ctx.db, id).await?;
    let build = builds::request_manual(
        &ctx,
        &application,
        &builds::ManualBuild {
            reference: input.reference.as_deref(),
            commit: input.commit.as_deref(),
            no_cache: input.no_cache,
        },
        &user.name,
    )
    .await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (application.id, &application.name),
        format!(
            "Compilação #{} de {} ({}){}",
            build.id,
            short_sha(&build.commit_sha),
            builds::branch_of(&build),
            if input.no_cache { " sem cache" } else { "" }
        ),
    )
    .await;
    Ok((
        StatusCode::ACCEPTED,
        Json(builds::view(&ctx.db, &build).await?),
    )
        .into_response())
}

async fn cancel(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let build = builds::cancel(&ctx, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (build.application_id, &format!("compilação #{id}")),
        format!("Compilação #{id} cancelada"),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn rebuild(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let build = builds::rebuild(&ctx, id, &user.name).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (build.application_id, &format!("compilação #{}", build.id)),
        format!("Recompilação sem cache de {}", short_sha(&build.commit_sha)),
    )
    .await;
    Ok((
        StatusCode::ACCEPTED,
        Json(builds::view(&ctx.db, &build).await?),
    )
        .into_response())
}

async fn list_secrets(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    applications::find(&ctx.db, id).await?;
    Ok(format::json(secrets::list(&ctx.db, id).await?)?)
}

async fn save_secret(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<SecretInput>,
) -> AppResult<Response> {
    let application = applications::find(&ctx.db, id).await?;
    secrets::save(&ctx.db, id, &input.name, &input.value).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (application.id, &application.name),
        format!("Segredo de build {} gravado", input.name.trim()),
    )
    .await;
    Ok(format::json(secrets::list(&ctx.db, id).await?)?)
}

async fn remove_secret(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((id, name)): Path<(i64, String)>,
) -> AppResult<Response> {
    let application = applications::find(&ctx.db, id).await?;
    secrets::remove(&ctx.db, id, &name).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (application.id, &application.name),
        format!("Segredo de build {name} apagado"),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn builder_readiness(
    State(ctx): State<AppContext>,
    Path(probe_id): Path<i64>,
) -> AppResult<Response> {
    let host = hosts::resolve(&ctx, HostKey::agent(probe_id))?;
    let readiness = host
        .images
        .builder_readiness()
        .await
        .map_err(|error| AppError::service_unavailable(error.to_string()))?;
    Ok(format::json(readiness)?)
}

/// O aviso do GitHub App. Sem sessão: a assinatura prova a origem.
async fn github_app_hook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    let signature = header(push::SIGNATURE_HEADER)
        .ok_or_else(|| AppError::unauthorized("Aviso sem assinatura do GitHub"))?;
    let event = header(push::EVENT_HEADER).unwrap_or_default();
    let response =
        push::handle(&ctx, event, header(push::DELIVERY_HEADER), signature, &body).await?;
    tracing::info!(
        event,
        builds = response.builds.len(),
        "aviso do GitHub App recebido"
    );
    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/deploys")
        .add("/builds", get(list))
        .add("/builds/{id}/log", get(log))
        .add("/builds/{id}/cancel", post(cancel))
        .add("/builds/{id}/rebuild", post(rebuild))
        .add("/applications/{id}/builds", post(start))
        .add(
            "/applications/{id}/build-secrets",
            get(list_secrets).post(save_secret),
        )
        .add(
            "/applications/{id}/build-secrets/{name}",
            delete(remove_secret),
        )
        .add("/agents/{id}/builder", get(builder_readiness))
}

/// O aviso público do GitHub App, montado fora do guarda JWT.
pub fn hook_routes() -> Routes {
    Routes::new()
        .prefix("/deploy-hooks")
        .add("/github-app", post(github_app_hook))
}
