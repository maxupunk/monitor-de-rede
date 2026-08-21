//! Trilha de auditoria (Fase 3 do roadmap).
//!
//! Endpoint administrativo para consultar os eventos de criação, alteração,
//! exclusão e autenticação registrados pelos demais controllers.

use axum::{extract::Query, http::HeaderMap};
use chrono::{DateTime, Utc};
use loco_rs::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use std::{collections::HashMap, str::FromStr};

use crate::{
    controllers::auth_guard::AUTHENTICATED_USER_HEADER,
    models::{_entities::users::Column as UsersColumn, users},
    services::{
        audit::{AuditFilters, AuditService},
        shared::errors::{AppError, AppResult},
        users::Role,
    },
    views::audit::{AuditLogListResponse, AuditLogResponse},
};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AuditLogQuery {
    page: Option<u64>,
    limit: Option<u64>,
    user_id: Option<i64>,
    resource_type: Option<String>,
    resource_id: Option<i64>,
    action: Option<String>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
}

async fn index(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<AuditLogQuery>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers).await?;

    let filters = AuditFilters {
        user_id: query.user_id,
        resource_type: query.resource_type,
        resource_id: query.resource_id,
        action: query.action,
        from: query.from,
        to: query.to,
    };

    let page = AuditService::new(&ctx.db)
        .list(filters, query.page, query.limit)
        .await?;

    let user_ids: Vec<i64> = page.data.iter().filter_map(|log| log.user_id).collect();
    let emails = if user_ids.is_empty() {
        HashMap::new()
    } else {
        users::Entity::find()
            .filter(UsersColumn::Id.is_in(user_ids))
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|u| (u.id, u.email))
            .collect()
    };

    Ok(format::json(AuditLogListResponse {
        data: page
            .data
            .into_iter()
            .map(|log| {
                let email = log.user_id.and_then(|id| emails.get(&id).cloned());
                AuditLogResponse::from_model(log, email)
            })
            .collect(),
        meta: page.meta,
    })?)
}

async fn require_admin(ctx: &AppContext, headers: &HeaderMap) -> AppResult<()> {
    let pid = headers
        .get(AUTHENTICATED_USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Não autenticado"))?;

    let user = users::Model::find_by_pid(&ctx.db, pid)
        .await
        .map_err(|_| AppError::unauthorized("Não autenticado"))?;

    let role = Role::from_str(&user.role)?;
    if !role.can_manage_users() {
        return Err(AppError::forbidden(
            "Apenas administradores podem consultar a trilha de auditoria.",
        ));
    }

    Ok(())
}

pub fn routes() -> Routes {
    Routes::new().prefix("/audit-logs").add("/", get(index))
}
