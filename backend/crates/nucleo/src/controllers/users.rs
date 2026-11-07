//! CRUD HTTP de usuários. As regras de segurança ficam em `UserService`.

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;

use netmonitor_core::roles::ACCESS_UPDATED_EVENT;

use crate::{
    controllers::auth_guard::AUTHENTICATED_USER_HEADER,
    dtos::users::{CreateUserInput, UpdateUserInput},
    models::_entities::users,
    services::{
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        shared::errors::{AppError, AppResult},
        users::UserService,
    },
    views::users::UserDetailResponse,
};

fn actor_pid(headers: &HeaderMap) -> AppResult<&str> {
    headers
        .get(AUTHENTICATED_USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Não autenticado"))
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let users = UserService::new(&ctx.db).list().await?;
    Ok(format::json(
        users
            .into_iter()
            .map(UserDetailResponse::from)
            .collect::<Vec<_>>(),
    )?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<CreateUserInput>,
) -> AppResult<Response> {
    let user = UserService::new(&ctx.db).create(&input).await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::User,
                resource_id: Some(user.id),
                resource_label: Some(user.email.clone()),
                description: Some(format!("Usuário {} criado", user.email)),
                changes: None,
            },
        )
        .await;

    Ok((StatusCode::CREATED, Json(UserDetailResponse::from(user))).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let user = UserService::new(&ctx.db).find(id).await?;
    Ok(format::json(UserDetailResponse::from(user))?)
}

async fn update(
    headers: HeaderMap,
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateUserInput>,
) -> AppResult<Response> {
    let old = UserService::new(&ctx.db).find(id).await?;
    let user = UserService::new(&ctx.db)
        .update(actor_pid(&headers)?, id, &input)
        .await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::User,
                resource_id: Some(user.id),
                resource_label: Some(user.email.clone()),
                description: Some(format!("Usuário {} atualizado", user.email)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(UserDetailResponse::from(old)).ok(),
                    new: serde_json::to_value(UserDetailResponse::from(user.clone())).ok(),
                }),
            },
        )
        .await;

    publish_access(&ctx, &user);
    Ok(format::json(UserDetailResponse::from(user))?)
}

/// Avisa a sessão do próprio usuário que o acesso dele mudou (ADR 019): o
/// menu, as rotas e os eventos se ajustam na hora, sem recarregar. O SSE só
/// entrega `user:*` à conexão desse usuário.
fn publish_access(ctx: &AppContext, user: &users::Model) {
    if let Ok(bus) = netmonitor_core::events::EventBus::from_context(ctx) {
        bus.publish_ephemeral(
            ACCESS_UPDATED_EVENT,
            serde_json::json!({
                "userId": user.id,
                "role": user.role,
                "areaRoles": crate::views::users::area_roles_of(user),
            }),
        );
    }
}

async fn destroy(
    headers: HeaderMap,
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let old = UserService::new(&ctx.db).find(id).await?;
    UserService::new(&ctx.db)
        .delete(actor_pid(&headers)?, id)
        .await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::User,
                resource_id: Some(id),
                resource_label: Some(old.email.clone()),
                description: Some(format!("Usuário {} excluído", old.email)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(UserDetailResponse::from(old)).ok(),
                    new: None,
                }),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/users")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
}
