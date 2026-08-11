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
    username: Option<String>,
    auth_protocol: Option<String>,
    auth_key: Option<String>,
    priv_protocol: Option<String>,
    priv_key: Option<String>,
    auto_detect: Option<bool>,
}
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnmpApplyInput {
    enable_cpu_monitor: Option<bool>,
    enable_memory_monitor: Option<bool>,
    monitored_if_indexes: Option<Vec<i32>>,
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
fn config_from_test_input(input: &SnmpTestInput, port: u16) -> AppResult<SnmpConfig> {
    let mut config = config(
        input.host.clone(),
        port,
        input.version.as_deref(),
        input.community.as_deref(),
    )?;
    config.username = input.username.clone();
    config.auth_protocol = input.auth_protocol.clone();
    config.auth_key = input.auth_key.clone();
    config.priv_protocol = input.priv_protocol.clone();
    config.priv_key = input.priv_key.clone();
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
                Some(config_from_test_input(&input, port)?),
            )
            .await?,
        )?);
    }
    Ok(format::json(
        service::test_connection(config_from_test_input(&input, port)?).await?,
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
    let device = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let device_config = config(
        device
            .ip_address
            .clone()
            .unwrap_or_else(|| device.name.clone()),
        161,
        device.snmp_version.as_deref(),
        device.snmp_community.as_deref(),
    )?;
    let result = service::poll_device(&ctx, &device, device_config).await?;
    Ok(format::json(
        serde_json::json!({ "message":"Varredura SNMP executada com sucesso", "result":result }),
    )?)
}
async fn apply_monitors(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<SnmpApplyInput>,
) -> AppResult<Response> {
    let device = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let config = config(
        device
            .ip_address
            .clone()
            .unwrap_or_else(|| device.name.clone()),
        161,
        device.snmp_version.as_deref(),
        device.snmp_community.as_deref(),
    )?;
    service::apply_monitors(
        &ctx,
        &device,
        config,
        service::SnmpApplyOptions {
            enable_cpu_monitor: input.enable_cpu_monitor,
            enable_memory_monitor: input.enable_memory_monitor,
            monitored_if_indexes: input.monitored_if_indexes.unwrap_or_default(),
        },
    )
    .await?;
    Ok(format::json(serde_json::json!({
        "message": "Configurações de monitoramento atualizadas com sucesso",
    }))?)
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
        .add("/devices/{id}/snmp/apply-monitors", post(apply_monitors))
        .add("/devices/{id}/interfaces", get(interfaces))
}
