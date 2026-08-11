//! Endpoints SNMP de teste e coleta de dispositivo.

use loco_rs::prelude::*;
use sea_orm::EntityTrait;

use crate::{
    models::devices,
    services::{
        shared::errors::{AppError, AppResult},
        snmp::{
            client::{SnmpConfig, SnmpVersion},
            service,
        },
    },
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnmpTestInput {
    host: String,
    port: Option<u16>,
    version: Option<String>,
    community: Option<String>,
    auto_detect: Option<bool>,
}

fn config(
    host: impl Into<String>,
    port: u16,
    version: Option<&str>,
    community: Option<&str>,
) -> AppResult<SnmpConfig> {
    let mut config = SnmpConfig::v2c(host, community.unwrap_or("public"), port);
    config.version = SnmpVersion::parse(version.unwrap_or("v2c"))
        .ok_or_else(|| AppError::validation("Versão SNMP inválida"))?;
    Ok(config)
}
async fn test(Json(input): Json<SnmpTestInput>) -> AppResult<Response> {
    if input.host.trim().is_empty() {
        return Err(AppError::validation("Host SNMP é obrigatório"));
    }
    let port = input.port.unwrap_or(161);
    if input.auto_detect.unwrap_or(false) {
        return Ok(format::json(
            service::detect_connection(
                &input.host,
                port,
                Some(config(
                    &input.host,
                    port,
                    input.version.as_deref(),
                    input.community.as_deref(),
                )?),
            )
            .await?,
        )?);
    }
    Ok(format::json(
        service::test_connection(config(
            input.host,
            port,
            input.version.as_deref(),
            input.community.as_deref(),
        )?)
        .await?,
    )?)
}
async fn device_config(ctx: &AppContext, id: i64) -> AppResult<SnmpConfig> {
    let device = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    config(
        device.ip_address.unwrap_or(device.name),
        161,
        device.snmp_version.as_deref(),
        device.snmp_community.as_deref(),
    )
}
async fn scan(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(
        service::scan(device_config(&ctx, id).await?).await?,
    )?)
}
async fn poll(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(
        serde_json::json!({ "message":"Varredura SNMP executada com sucesso", "result":service::scan(device_config(&ctx, id).await?).await? }),
    )?)
}
async fn interfaces(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let scan = service::scan(device_config(&ctx, id).await?).await?;
    Ok(format::json(scan.interfaces)?)
}
pub fn routes() -> Routes {
    Routes::new()
        .add("/snmp/test", post(test))
        .add("/devices/{id}/snmp/scan", post(scan))
        .add("/devices/{id}/snmp/poll", post(poll))
        .add("/devices/{id}/interfaces", get(interfaces))
}
