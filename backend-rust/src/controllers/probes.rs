//! CRUD administrativo de probes. O protocolo de agentes entra na Fase 7.

use axum::{http::StatusCode, response::IntoResponse};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
use uuid::Uuid;

use crate::{
    dtos::resources::ProbeInput,
    models::probes,
    services::{
        maintenance::resource_cleanup::ResourceCleanupService,
        shared::{
            crypto::sha256_hex,
            errors::{AppError, AppResult},
        },
    },
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeResponse {
    id: i64,
    site_id: Option<i64>,
    name: String,
    status: String,
    version: Option<String>,
    last_seen_at: Option<String>,
    registered_at: Option<String>,
    revoked_at: Option<String>,
    configuration: Option<serde_json::Value>,
    created_at: String,
    updated_at: String,
}
impl From<probes::Model> for ProbeResponse {
    fn from(row: probes::Model) -> Self {
        Self {
            id: row.id,
            site_id: row.site_id,
            name: row.name,
            status: row.status,
            version: row.version,
            last_seen_at: row.last_seen_at.map(|v| v.to_rfc3339()),
            registered_at: row.registered_at.map(|v| v.to_rfc3339()),
            revoked_at: row.revoked_at.map(|v| v.to_rfc3339()),
            configuration: row.configuration,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = probes::Entity::find()
        .order_by_asc(probes::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        rows.into_iter()
            .map(ProbeResponse::from)
            .collect::<Vec<_>>(),
    )?)
}
async fn store(
    State(ctx): State<AppContext>,
    Json(input): Json<ProbeInput>,
) -> AppResult<Response> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::validation("Nome do probe é obrigatório"))?;
    let token_hash = input
        .token_hash
        .unwrap_or_else(|| sha256_hex(&Uuid::new_v4().to_string()));
    let row = probes::ActiveModel {
        site_id: Set(input.site_id),
        name: Set(name.into()),
        token_hash: Set(token_hash),
        status: Set(input.status.unwrap_or_else(|| "pending".into())),
        version: Set(input.version),
        configuration: Set(input.configuration),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    Ok((StatusCode::CREATED, Json(ProbeResponse::from(row))).into_response())
}
async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    Ok(format::json(ProbeResponse::from(row))?)
}
async fn update(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<ProbeInput>,
) -> AppResult<Response> {
    let current = probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    let row = probes::ActiveModel {
        id: Set(id),
        site_id: Set(input.site_id.or(current.site_id)),
        name: Set(input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(&current.name)
            .into()),
        status: Set(input.status.unwrap_or(current.status)),
        version: Set(input.version.or(current.version)),
        configuration: Set(input.configuration.or(current.configuration)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    Ok(format::json(ProbeResponse::from(row))?)
}
async fn destroy(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    ResourceCleanupService::delete_probe(&ctx.db, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
async fn revoke(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    let mut active: probes::ActiveModel = row.into();
    active.status = Set("revoked".into());
    active.revoked_at = Set(Some(Utc::now().into()));
    Ok(format::json(ProbeResponse::from(
        active.update(&ctx.db).await?,
    ))?)
}
async fn test(Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(
        serde_json::json!({"message":format!("Teste de conectividade enviado para o probe ID {id}")}),
    )?)
}
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/probes")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/revoke", post(revoke))
        .add("/{id}/test", post(test))
}
