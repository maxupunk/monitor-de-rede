//! CRUD de redes e enfileiramento persistente de discovery.

use axum::{http::HeaderMap, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};

use crate::{
    dtos::resources::NetworkInput,
    models::{networks, sites},
    services::{
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        discovery::{cidr_range::parse_cidr_range, queue::enqueue_network_scan},
        shared::errors::{AppError, AppResult},
    },
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteReference {
    id: i64,
    name: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkResponse {
    id: i64,
    site_id: Option<i64>,
    probe_id: Option<i64>,
    name: String,
    cidr: String,
    gateway: Option<String>,
    vlan: Option<i32>,
    dns_servers: Option<serde_json::Value>,
    scan_enabled: bool,
    scan_interval: i32,
    active: bool,
    last_scan_at: Option<String>,
    next_scan_at: Option<String>,
    created_at: String,
    updated_at: String,
    site: Option<SiteReference>,
    scannable: bool,
    usable_hosts: u32,
}

async fn present(
    db: &sea_orm::DatabaseConnection,
    network: networks::Model,
) -> AppResult<NetworkResponse> {
    let site = match network.site_id {
        Some(id) => sites::Entity::find_by_id(id)
            .one(db)
            .await?
            .map(|site| SiteReference {
                id: site.id,
                name: site.name,
            }),
        None => None,
    };
    let scannable = network.scannable();
    let usable_hosts = network.usable_hosts();
    Ok(NetworkResponse {
        id: network.id,
        site_id: network.site_id,
        probe_id: network.probe_id,
        name: network.name,
        cidr: network.cidr,
        gateway: network.gateway,
        vlan: network.vlan,
        dns_servers: network.dns_servers,
        scan_enabled: network.scan_enabled,
        scan_interval: network.scan_interval,
        active: network.active,
        last_scan_at: network.last_scan_at.map(|v| v.to_rfc3339()),
        next_scan_at: network.next_scan_at.map(|v| v.to_rfc3339()),
        created_at: network.created_at.to_rfc3339(),
        updated_at: network.updated_at.to_rfc3339(),
        site,
        scannable,
        usable_hosts,
    })
}

fn validate(input: &NetworkInput) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::validation("Nome da rede é obrigatório"));
    }
    if parse_cidr_range(&input.cidr).is_err() {
        return Err(AppError::validation("CIDR inválido"));
    }
    if input.scan_interval.is_some_and(|v| v <= 0) {
        return Err(AppError::validation(
            "Intervalo de scan deve ser maior que zero",
        ));
    }
    Ok(())
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = networks::Entity::find()
        .order_by_asc(networks::Column::Name)
        .all(&ctx.db)
        .await?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        output.push(present(&ctx.db, row).await?);
    }
    Ok(format::json(output)?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<NetworkInput>,
) -> AppResult<Response> {
    validate(&input)?;
    let row = networks::ActiveModel {
        site_id: Set(input.site_id),
        probe_id: Set(input.probe_id),
        name: Set(input.name.trim().into()),
        cidr: Set(input.cidr.trim().into()),
        gateway: Set(input.gateway),
        vlan: Set(input.vlan),
        dns_servers: Set(input.dns_servers),
        scan_enabled: Set(input.scan_enabled.unwrap_or(true)),
        scan_interval: Set(input.scan_interval.unwrap_or(3600)),
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
                resource_type: ResourceType::Network,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!("Rede '{}' ({}) criada", row.name, row.cidr)),
                changes: None,
            },
        )
        .await;

    Ok((StatusCode::CREATED, Json(present(&ctx.db, row).await?)).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = networks::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Rede não encontrada"))?;
    Ok(format::json(present(&ctx.db, row).await?)?)
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<NetworkInput>,
) -> AppResult<Response> {
    validate(&input)?;
    let old = networks::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Rede não encontrada"))?;
    let old_response = present(&ctx.db, old.clone()).await?;
    let row = networks::ActiveModel {
        id: Set(old.id),
        site_id: Set(input.site_id),
        probe_id: Set(input.probe_id),
        name: Set(input.name.trim().into()),
        cidr: Set(input.cidr.trim().into()),
        gateway: Set(input.gateway),
        vlan: Set(input.vlan),
        dns_servers: Set(input.dns_servers),
        scan_enabled: Set(input.scan_enabled.unwrap_or(old.scan_enabled)),
        scan_interval: Set(input.scan_interval.unwrap_or(old.scan_interval)),
        active: Set(input.active.unwrap_or(old.active)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;

    let new_response = present(&ctx.db, row.clone()).await?;
    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::Network,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!("Rede '{}' ({}) atualizada", row.name, row.cidr)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(old_response).ok(),
                    new: serde_json::to_value(&new_response).ok(),
                }),
            },
        )
        .await;

    Ok(format::json(new_response)?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let old = networks::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Rede não encontrada"))?;
    let old_response = present(&ctx.db, old.clone()).await?;
    old.clone().delete(&ctx.db).await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::Network,
                resource_id: Some(id),
                resource_label: Some(old.name.clone()),
                description: Some(format!("Rede '{}' ({}) excluída", old.name, old.cidr)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(old_response).ok(),
                    new: None,
                }),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn scan(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let network = networks::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Rede não encontrada"))?;
    let range = parse_cidr_range(&network.cidr)
        .map_err(|_| AppError::validation("CIDR inválido para varredura"))?;
    let (run, already_queued) = enqueue_network_scan(&ctx.db, &network).await?;
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "message": "Varredura de rede enfileirada", "alreadyQueued": already_queued,
        "run": { "id": run.id, "status": run.status }, "usableHosts": range.usable_hosts, "truncated": range.truncated }))).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/networks")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/scan", post(scan))
}
