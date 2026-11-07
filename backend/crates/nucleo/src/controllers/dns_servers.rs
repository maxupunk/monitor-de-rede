//! Cadastro de resolvedores DNS usado pelos monitores e pelo benchmark.

use axum::{http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};

use crate::{
    dtos::resources::DnsServerInput,
    models::dns_servers,
    services::shared::errors::{AppError, AppResult},
};

fn validate(address: &str, protocol: &str) -> AppResult<()> {
    if !matches!(protocol, "udp" | "tcp" | "doh") {
        return Err(AppError::validation("Protocolo DNS inválido"));
    }
    if address.trim().is_empty() {
        return Err(AppError::validation("Informe o endereço do servidor DNS"));
    }
    if protocol == "doh" && !address.starts_with("https://") {
        return Err(AppError::validation(
            "O endpoint DoH precisa começar com https://",
        ));
    }
    Ok(())
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DnsServerResponse {
    id: i64,
    name: String,
    address: String,
    protocol: String,
    is_default: bool,
    description: Option<String>,
    created_at: String,
    updated_at: String,
}
impl From<dns_servers::Model> for DnsServerResponse {
    fn from(row: dns_servers::Model) -> Self {
        Self {
            id: row.id,
            name: row.name,
            address: row.address,
            protocol: row.protocol,
            is_default: row.is_default,
            description: row.description,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}
async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = dns_servers::Entity::find()
        .order_by_asc(dns_servers::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        rows.into_iter()
            .map(DnsServerResponse::from)
            .collect::<Vec<_>>(),
    )?)
}
async fn store(
    State(ctx): State<AppContext>,
    Json(input): Json<DnsServerInput>,
) -> AppResult<Response> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::validation("Nome do servidor DNS é obrigatório"))?;
    let address = input
        .address
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::validation("Informe o endereço do servidor DNS"))?;
    let protocol = input.protocol.as_deref().unwrap_or("udp").to_lowercase();
    validate(address, &protocol)?;
    if dns_servers::Entity::find_by_address(address, &protocol)
        .one(&ctx.db)
        .await?
        .is_some()
    {
        return Err(AppError::conflict(format!(
            "O servidor {address} ({}) já está cadastrado",
            protocol.to_uppercase()
        )));
    }
    let row = dns_servers::ActiveModel {
        name: Set(name.into()),
        address: Set(address.into()),
        protocol: Set(protocol),
        is_default: Set(input.is_default.unwrap_or(true)),
        description: Set(input.description),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    Ok((StatusCode::CREATED, Json(DnsServerResponse::from(row))).into_response())
}
async fn update(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<DnsServerInput>,
) -> AppResult<Response> {
    let current = dns_servers::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Servidor DNS não encontrado"))?;
    let address = input
        .address
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(&current.address)
        .to_string();
    let protocol = input
        .protocol
        .unwrap_or_else(|| current.protocol.clone())
        .to_lowercase();
    validate(&address, &protocol)?;
    if let Some(other) = dns_servers::Entity::find_by_address(&address, &protocol)
        .one(&ctx.db)
        .await?
    {
        if other.id != id {
            return Err(AppError::conflict(format!(
                "O servidor {address} ({}) já está cadastrado",
                protocol.to_uppercase()
            )));
        }
    }
    let row = dns_servers::ActiveModel {
        id: Set(id),
        name: Set(input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(&current.name)
            .into()),
        address: Set(address),
        protocol: Set(protocol),
        is_default: Set(input.is_default.unwrap_or(current.is_default)),
        description: Set(input.description.or(current.description)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    Ok(format::json(DnsServerResponse::from(row))?)
}
async fn destroy(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = dns_servers::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Servidor DNS não encontrado"))?;
    row.delete(&ctx.db).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/dns/servers")
        .add("/", get(index).post(store))
        .add("/{id}", put(update).delete(destroy))
}
