//! CRUD de sites, mantendo a cascata de remoção no serviço de domínio.

use axum::{http::HeaderMap, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};

use crate::{
    dtos::resources::SiteInput,
    models::sites,
    services::{
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        maintenance::resource_cleanup::ResourceCleanupService,
        shared::errors::{AppError, AppResult},
    },
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteResponse {
    id: i64,
    name: String,
    description: Option<String>,
    location: Option<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

impl From<sites::Model> for SiteResponse {
    fn from(site: sites::Model) -> Self {
        Self {
            id: site.id,
            name: site.name,
            description: site.description,
            location: site.location,
            active: site.active,
            created_at: site.created_at.to_rfc3339(),
            updated_at: site.updated_at.to_rfc3339(),
        }
    }
}

fn valid_name(name: &str) -> AppResult<()> {
    if name.trim().is_empty() {
        Err(AppError::validation("Nome do site é obrigatório"))
    } else {
        Ok(())
    }
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = sites::Entity::find()
        .order_by_asc(sites::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        rows.into_iter().map(SiteResponse::from).collect::<Vec<_>>(),
    )?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<SiteInput>,
) -> AppResult<Response> {
    valid_name(&input.name)?;
    let row = sites::ActiveModel {
        name: Set(input.name.trim().to_string()),
        description: Set(input.description),
        location: Set(input.location),
        active: Set(input.active.unwrap_or(true)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::Site,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!("Site '{}' criado", row.name)),
                changes: None,
            },
        )
        .await;

    Ok((StatusCode::CREATED, Json(SiteResponse::from(row))).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = sites::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Site não encontrado"))?;
    Ok(format::json(SiteResponse::from(row))?)
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<SiteInput>,
) -> AppResult<Response> {
    valid_name(&input.name)?;
    let old = sites::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Site não encontrado"))?;
    let updated = sites::ActiveModel {
        id: Set(old.id),
        name: Set(input.name.trim().to_string()),
        description: Set(input.description),
        location: Set(input.location),
        active: Set(input.active.unwrap_or(old.active)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::Site,
                resource_id: Some(updated.id),
                resource_label: Some(updated.name.clone()),
                description: Some(format!("Site '{}' atualizado", updated.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(SiteResponse::from(old)).ok(),
                    new: serde_json::to_value(SiteResponse::from(updated.clone())).ok(),
                }),
            },
        )
        .await;

    Ok(format::json(SiteResponse::from(updated))?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let old = sites::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Site não encontrado"))?;
    ResourceCleanupService::delete_site(&ctx.db, id).await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::Site,
                resource_id: Some(id),
                resource_label: Some(old.name.clone()),
                description: Some(format!("Site '{}' excluído", old.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(SiteResponse::from(old)).ok(),
                    new: None,
                }),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/sites")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
}
