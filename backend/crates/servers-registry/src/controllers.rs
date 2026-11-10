//! A tela do registry (`/api/registry`): extrai, valida, delega e audita.
//! A regra mora em [`crate::browse`], [`crate::tokens`] e
//! [`crate::settings`].

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use loco_rs::prelude::*;
use serde::Deserialize;
use ts_rs::TS;

use netmonitor_core::{
    audit::{authenticated_user, AuditAction, ResourceType},
    roles::Role,
    shared::errors::{AppError, AppResult},
};

use crate::{
    auth, browse, catalog, events, scan, settings,
    settings::Settings,
    tokens::{self, Scope},
    views::{RegistryTokenCreated, RegistryTokenView},
};

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryTokenInput {
    pub name: String,
    pub scope: Scope,
    /// Validade em dias (1 a 366).
    #[ts(type = "number")]
    pub days: i64,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryPinInput {
    pub pinned: bool,
}

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RegistryExceptionInput {
    /// Nula: vale para todas as aplicações.
    #[ts(type = "number | null")]
    pub application_id: Option<i64>,
    pub advisory: String,
    pub reason: String,
    #[ts(type = "number")]
    pub days: i64,
}

async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    resource: (i64, &str),
    description: String,
) {
    netmonitor_core::audit::record(
        &ctx.db,
        headers,
        action,
        ResourceType::Registry,
        resource,
        description,
    )
    .await;
}

/// Só o administrador da área (apagar, exceções, configurações, tokens de
/// outros).
async fn require_admin(
    ctx: &AppContext,
    headers: &HeaderMap,
) -> AppResult<netmonitor_core::models::users::Model> {
    let user = authenticated_user(ctx, headers).await?;
    if auth::servers_role(&user) == Some(Role::Admin) {
        Ok(user)
    } else {
        Err(AppError::forbidden(
            "Só o administrador de Servidores faz isso no registry",
        ))
    }
}

async fn overview(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(browse::overview(&ctx.db).await?)?)
}

async fn repository(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(browse::detail(&ctx.db, id).await?)?)
}

async fn findings(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(browse::findings(&ctx.db, id).await?)?)
}

async fn pin(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<RegistryPinInput>,
) -> AppResult<Response> {
    let row = browse::pin(&ctx.db, id, input.pinned).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (row.id, &row.digest),
        format!(
            "Versão {} {}",
            netmonitor_servers::deploys::short_digest(&row.digest),
            if input.pinned { "fixada" } else { "solta" }
        ),
    )
    .await;
    events::publish_changed(&ctx);
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn remove(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;
    let row = browse::remove(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (row.id, &row.digest),
        format!("Versão {} apagada do registry", row.digest),
    )
    .await;
    events::publish_changed(&ctx);
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn rescan(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = browse::version(&ctx.db, id).await?;
    scan::run(&ctx, &row).await?;
    Ok(format::json(browse::findings(&ctx.db, id).await?)?)
}

async fn save_settings(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<Settings>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;
    settings::save(&ctx.db, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (0, "registry"),
        format!(
            "Registry: guardar {} versões, cota de {} GiB",
            input.keep_versions, input.quota_gib
        ),
    )
    .await;
    events::publish_changed(&ctx);
    Ok(format::json(input)?)
}

async fn list_tokens(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let all = auth::servers_role(&user) == Some(Role::Admin);
    Ok(format::json(
        browse::token_views(&ctx.db, (!all).then_some(user.id)).await?,
    )?)
}

async fn create_token(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<RegistryTokenInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    if auth::servers_role(&user).is_none() {
        return Err(AppError::forbidden("Você não tem acesso à área Servidores"));
    }
    let (row, secret) =
        tokens::create(&ctx.db, user.id, &input.name, &input.scope, input.days).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (row.id, &row.name),
        format!(
            "Token do registry '{}' criado ({} dias)",
            row.name, input.days
        ),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(RegistryTokenCreated {
            token: RegistryTokenView::new(&row, user.name),
            secret,
        }),
    )
        .into_response())
}

async fn revoke_token(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let owner = (auth::servers_role(&user) != Some(Role::Admin)).then_some(user.id);
    let row = tokens::revoke(&ctx.db, id, owner).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (row.id, &row.name),
        format!("Token do registry '{}' revogado", row.name),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn list_exceptions(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(browse::exception_views(&ctx.db).await?)?)
}

async fn add_exception(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<RegistryExceptionInput>,
) -> AppResult<Response> {
    let user = require_admin(&ctx, &headers).await?;
    let row = browse::add_exception(
        &ctx.db,
        input.application_id,
        &input.advisory,
        &input.reason,
        input.days,
        &user.name,
    )
    .await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (row.id, &row.advisory),
        format!(
            "Exceção para {} por {} dia(s): {}",
            row.advisory, input.days, row.reason
        ),
    )
    .await;
    events::publish_changed(&ctx);
    Ok(StatusCode::CREATED.into_response())
}

async fn remove_exception(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;
    browse::remove_exception(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (id, "exceção"),
        format!("Exceção #{id} removida"),
    )
    .await;
    events::publish_changed(&ctx);
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn sync_advisories(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;
    let guard = scan::refresh_guard()?;
    let ctx_bg = ctx.clone();
    tokio::spawn(async move {
        let _guard = guard;
        if let Err(error) = scan::refresh(&ctx_bg).await {
            tracing::warn!(%error, "atualização manual da base de CVE falhou");
        }
        events::publish_changed(&ctx_bg);
    });
    Ok(StatusCode::ACCEPTED.into_response())
}

async fn collect(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;
    let freed = crate::retention::collect(&ctx).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (0, "registry"),
        format!("Limpeza do registry: {freed} bytes liberados"),
    )
    .await;
    Ok(format::json(
        serde_json::json!({ "freedBytes": freed, "repositories": catalog::count_repositories(&ctx.db).await? }),
    )?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/registry")
        .add("/", get(overview))
        .add("/settings", put(save_settings))
        .add("/collect", post(collect))
        .add("/advisories/sync", post(sync_advisories))
        .add("/repositories/{id}", get(repository))
        .add("/versions/{id}", delete(remove))
        .add("/versions/{id}/findings", get(findings))
        .add("/versions/{id}/pin", post(pin))
        .add("/versions/{id}/scan", post(rescan))
        .add("/tokens", get(list_tokens).post(create_token))
        .add("/tokens/{id}", delete(revoke_token))
        .add("/exceptions", get(list_exceptions).post(add_exception))
        .add("/exceptions/{id}", delete(remove_exception))
}
