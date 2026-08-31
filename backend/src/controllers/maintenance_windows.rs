//! CRUD de janelas de manutenção (Fase 3 do roadmap).

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::AUTHENTICATED_USER_HEADER,
    dtos::resources::MaintenanceWindowInput,
    models::{maintenance_windows as maintenance_windows_model, users},
    services::{
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        events::EventBus,
        maintenance_windows::{self, MaintenanceWindowInput as ServiceInput},
        shared::errors::{AppError, AppResult},
    },
    views::maintenance_windows::MaintenanceWindowResponse,
};

/// Resolve o `id` numérico do usuário a partir do `pid` que o guarda JWT gravou
/// no cabeçalho interno. A janela guarda quem a criou para auditoria.
async fn actor_id(ctx: &AppContext, headers: &HeaderMap) -> AppResult<Option<i64>> {
    let pid = headers
        .get(AUTHENTICATED_USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("Não autenticado"))?;

    match users::Model::find_by_pid(&ctx.db, pid).await {
        Ok(user) => Ok(Some(user.id)),
        Err(_) => Ok(None),
    }
}

fn into_service_input(input: MaintenanceWindowInput) -> ServiceInput {
    ServiceInput {
        site_id: input.site_id,
        device_id: input.device_id,
        name: input.name,
        description: input.description,
        starts_at: input.starts_at,
        ends_at: input.ends_at,
    }
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = maintenance_windows::list(&ctx.db).await?;
    Ok(format::json(
        rows.into_iter()
            .map(MaintenanceWindowResponse::from)
            .collect::<Vec<_>>(),
    )?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<MaintenanceWindowInput>,
) -> AppResult<Response> {
    let created_by = actor_id(&ctx, &headers).await?;
    let row = maintenance_windows::create(&ctx.db, into_service_input(input), created_by).await?;
    emit_maintenance_windows_updated(&ctx).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::MaintenanceWindow,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!("Janela de manutenção '{}' criada", row.name)),
                changes: None,
            },
        )
        .await;

    Ok((
        StatusCode::CREATED,
        Json(MaintenanceWindowResponse::from(row)),
    )
        .into_response())
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<MaintenanceWindowInput>,
) -> AppResult<Response> {
    let old = maintenance_windows_model::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Janela de manutenção não encontrada"))?;
    let row = maintenance_windows::update(&ctx.db, id, into_service_input(input)).await?;
    emit_maintenance_windows_updated(&ctx).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::MaintenanceWindow,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!("Janela de manutenção '{}' atualizada", row.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(MaintenanceWindowResponse::from(old)).ok(),
                    new: serde_json::to_value(MaintenanceWindowResponse::from(row.clone())).ok(),
                }),
            },
        )
        .await;

    Ok(format::json(MaintenanceWindowResponse::from(row))?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let old = maintenance_windows_model::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Janela de manutenção não encontrada"))?;
    maintenance_windows::delete(&ctx.db, id).await?;
    emit_maintenance_windows_updated(&ctx).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::MaintenanceWindow,
                resource_id: Some(id),
                resource_label: Some(old.name.clone()),
                description: Some(format!("Janela de manutenção '{}' excluída", old.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(MaintenanceWindowResponse::from(old)).ok(),
                    new: None,
                }),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn emit_maintenance_windows_updated(ctx: &AppContext) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(
                &ctx.db,
                "maintenance_windows:updated",
                serde_json::json!({}),
            )
            .await
        {
            tracing::warn!(%error, "falha ao publicar maintenance_windows:updated");
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/maintenance-windows")
        .add("/", get(index).post(store))
        .add("/{id}", put(update).delete(destroy))
}
