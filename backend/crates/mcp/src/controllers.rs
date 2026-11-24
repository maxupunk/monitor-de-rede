//! API REST de gestão do servidor MCP (`/api/mcp/*`).
//!
//! Extrai, valida, delega para [`crate::settings`], [`crate::tokens`] e
//! [`crate::scopes`], audita e publica `mcp:changed` via SSE.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use loco_rs::prelude::*;

use netmonitor_core::{
    ai::tools::all_handlers,
    areas::{self, Area, AreaSet},
    audit::{self, authenticated_user, require_admin, AuditAction, ResourceType},
    mcp::deploy_app_catalog,
    roles::{AreaAccess, Role},
    shared::errors::AppResult,
};

use crate::{
    dtos::{
        CreateMcpTokenInput, McpOverview, McpScopeDescriptor, McpSettingsView, McpTokenCreated,
        McpTokenView, UpdateMcpSettingsInput,
    },
    events,
    scopes::{self, McpScope},
    settings::{self, McpSettings},
    tokens,
};

async fn overview(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let access = AreaAccess::of_user(&user)?;
    let loaded_settings = settings::load(&ctx.db).await?;
    let enabled_areas = AreaSet::from_enabled(&areas::load(&ctx.db).await?);

    let scope_descriptors = McpScope::ALL
        .into_iter()
        .map(|scope| McpScopeDescriptor::for_scope(scope, scope.granted_by_access(&access)))
        .collect();

    let deploy_applications =
        if enabled_areas.contains(Area::Servers) && access.role_in(Area::Servers).is_some() {
            match deploy_app_catalog(&ctx) {
                Some(catalog) => catalog.list_applications(&ctx.db).await?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };

    let handlers = all_handlers();
    let tools_count = scopes::available_tools_count(&handlers, user.active, &access, enabled_areas);

    let body = McpOverview {
        settings: McpSettingsView::from(loaded_settings),
        scopes: scope_descriptors,
        deploy_applications,
        tools_count,
        can_manage_settings: access.global().can_manage_users(),
    };
    Ok(format::json(body)?)
}

async fn update_settings(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<UpdateMcpSettingsInput>,
) -> AppResult<Response> {
    require_admin(
        &ctx,
        &headers,
        "Apenas administradores podem alterar as configurações do servidor MCP.",
    )
    .await?;

    let saved = settings::save(
        &ctx.db,
        &McpSettings {
            enabled: input.enabled,
            allow_public_origin: input.allow_public_origin,
        },
    )
    .await?;

    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Update,
        ResourceType::SystemSetting,
        (0, "mcp"),
        format!(
            "Servidor MCP: ativo={}, borda pública={}",
            saved.enabled, saved.allow_public_origin
        ),
    )
    .await;
    events::publish_changed(&ctx);

    Ok(format::json(McpSettingsView::from(saved))?)
}

async fn list_tokens(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let access = AreaAccess::of_user(&user)?;
    let filter_user = (access.global() != Role::Admin).then_some(user.id);
    let list = tokens::list(&ctx.db, filter_user).await?;
    Ok(format::json(list)?)
}

async fn create_token(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<CreateMcpTokenInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let access = AreaAccess::of_user(&user)?;
    let (row, secret) = tokens::create(&ctx.db, &user, &access, &input).await?;

    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Create,
        ResourceType::McpToken,
        (row.id, &row.name),
        format!(
            "Token MCP '{}' criado (escopos: {})",
            row.name,
            tokens::scopes_of(&row)
                .into_iter()
                .map(McpScope::id)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
    .await;
    events::publish_changed(&ctx);

    Ok((
        StatusCode::CREATED,
        Json(McpTokenCreated {
            token: McpTokenView::from_model(&row, tokens::format_owner(&user)),
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
    let access = AreaAccess::of_user(&user)?;
    let owner = (access.global() != Role::Admin).then_some(user.id);
    let row = tokens::revoke(&ctx.db, id, owner).await?;

    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Delete,
        ResourceType::McpToken,
        (row.id, &row.name),
        format!("Token MCP '{}' revogado", row.name),
    )
    .await;
    events::publish_changed(&ctx);

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Rotas de gestão sob `/api/mcp/*` (protegidas por `business_auth`).
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/mcp")
        .add("/overview", get(overview))
        .add("/settings", put(update_settings))
        .add("/tokens", get(list_tokens).post(create_token))
        .add("/tokens/{id}", delete(revoke_token))
}
