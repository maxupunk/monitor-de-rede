//! Cadastro de dispositivos e recursos dependentes.

use std::collections::HashMap;

use axum::{extract::Query, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, QueryFilter, QueryOrder, QuerySelect,
    Set,
};

use crate::{
    dtos::resources::{DeviceInput, PaginationQuery},
    models::{
        _entities::{
            alert_events, device_interfaces, discovery_results, metrics as metrics_entity,
            zabbix_templates,
        },
        devices, monitors, sites,
    },
    services::{
        maintenance::resource_cleanup::ResourceCleanupService,
        monitoring::presenter::{present_monitors, RECENT_RESULTS_LIMIT},
        shared::{
            errors::{AppError, AppResult},
            pagination::{paginate_compat, MaybePaged},
        },
    },
};

/// Serialização canônica de um dispositivo para a API.
///
/// É `pub(crate)` porque as telas de VPN devolvem o mesmo objeto dentro de
/// `{peer, device}` (§7.13). Duplicar a lista de campos lá faria as duas
/// versões divergirem no primeiro campo novo — e o `Model` do `sea-orm`
/// serializa em `snake_case`, o que quebraria o contrato camelCase da §5.1.
pub(crate) async fn present(
    db: &sea_orm::DatabaseConnection,
    device: devices::Model,
) -> AppResult<serde_json::Value> {
    let site = match device.site_id {
        Some(id) => sites::Entity::find_by_id(id)
            .one(db)
            .await?
            .map(|row| serde_json::json!({"id": row.id, "name": row.name})),
        None => None,
    };
    let parent = match device.parent_id {
        Some(id) => devices::Entity::find_by_id(id)
            .one(db)
            .await?
            .map(|row| serde_json::json!({"id": row.id, "name": row.name})),
        None => None,
    };
    let zabbix_template = match device.zabbix_template_id { Some(id) => zabbix_templates::Entity::find_by_id(id).one(db).await?.map(|row| serde_json::json!({"id": row.id, "name": row.name, "description": row.description, "zabbixVersion": row.zabbix_version, "items": []})), None => None };
    Ok(serde_json::json!({
        "id": device.id, "siteId": device.site_id, "networkId": device.network_id, "parentId": device.parent_id,
        "zabbixTemplateId": device.zabbix_template_id, "ipAddress": device.ip_address, "name": device.name,
        "type": device.r#type, "vendor": device.vendor, "model": device.model, "serialNumber": device.serial_number,
        "description": device.description, "isMonitored": device.is_monitored, "snmpEnabled": device.snmp_enabled,
        "snmpCommunity": device.snmp_community, "snmpVersion": device.snmp_version, "status": device.status,
        "lastSeenAt": device.last_seen_at.map(|v| v.to_rfc3339()), "createdAt": device.created_at.to_rfc3339(),
        "updatedAt": device.updated_at.to_rfc3339(), "site": site, "parent": parent, "zabbixTemplate": zabbix_template,
        "vpnPeer": serde_json::Value::Null
    }))
}

fn require_name_type(input: &DeviceInput) -> AppResult<(&str, &str)> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| AppError::validation("Nome do dispositivo é obrigatório"))?;
    let kind = input
        .device_type
        .as_deref()
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .ok_or_else(|| AppError::validation("Tipo do dispositivo é obrigatório"))?;
    Ok((name, kind))
}

async fn sync_device_monitor(
    db: &sea_orm::DatabaseConnection,
    device: &devices::Model,
) -> AppResult<()> {
    let existing = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .one(db)
        .await?;
    if device.is_monitored {
        let configuration = serde_json::json!({"host": device.ip_address.clone().unwrap_or_else(|| device.name.clone())});
        if let Some(row) = existing {
            let mut active: monitors::ActiveModel = row.into();
            active.enabled = Set(true);
            active.name = Set(format!("Ping {}", device.name));
            active.configuration = Set(configuration);
            active.update(db).await?;
        } else {
            monitors::ActiveModel {
                device_id: Set(Some(device.id)),
                r#type: Set("ping".into()),
                name: Set(format!("Ping {}", device.name)),
                configuration: Set(configuration),
                interval_seconds: Set(60),
                timeout_seconds: Set(10),
                retry_count: Set(3),
                enabled: Set(true),
                status: Set("unknown".into()),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    } else if let Some(row) = existing {
        let mut active: monitors::ActiveModel = row.into();
        active.enabled = Set(false);
        active.update(db).await?;
    }
    Ok(())
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = devices::Entity::find()
        .order_by_asc(devices::Column::Name)
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
    Json(input): Json<DeviceInput>,
) -> AppResult<Response> {
    let (name, kind) = require_name_type(&input)?;
    let name = name.to_string();
    let kind = kind.to_string();
    let row = devices::ActiveModel {
        site_id: Set(input.site_id),
        network_id: Set(input.network_id),
        parent_id: Set(input.parent_id),
        zabbix_template_id: Set(input.zabbix_template_id),
        ip_address: Set(input.ip_address),
        name: Set(name),
        r#type: Set(kind),
        vendor: Set(input.vendor),
        model: Set(input.model),
        serial_number: Set(input.serial_number),
        description: Set(input.description),
        is_monitored: Set(input.is_monitored.unwrap_or(false)),
        snmp_enabled: Set(input.snmp_enabled.unwrap_or(false)),
        snmp_community: Set(input.snmp_community),
        snmp_version: Set(input.snmp_version),
        status: Set(input.status.unwrap_or_else(|| "unknown".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    sync_device_monitor(&ctx.db, &row).await?;
    if let Some(ip) = &row.ip_address {
        discovery_results::Entity::delete_many()
            .filter(discovery_results::Column::IpAddress.eq(ip))
            .exec(&ctx.db)
            .await?;
    }
    Ok((StatusCode::CREATED, Json(present(&ctx.db, row).await?)).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    Ok(format::json(present(&ctx.db, row).await?)?)
}

async fn update(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<DeviceInput>,
) -> AppResult<Response> {
    let current = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&current.name)
        .to_string();
    let kind = input
        .device_type
        .as_deref()
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .unwrap_or(&current.r#type)
        .to_string();
    let row = devices::ActiveModel {
        id: Set(current.id),
        site_id: Set(input.site_id.or(current.site_id)),
        network_id: Set(input.network_id.or(current.network_id)),
        parent_id: Set(input.parent_id.or(current.parent_id)),
        zabbix_template_id: Set(input.zabbix_template_id.or(current.zabbix_template_id)),
        ip_address: Set(input.ip_address.or(current.ip_address)),
        name: Set(name),
        r#type: Set(kind),
        vendor: Set(input.vendor.or(current.vendor)),
        model: Set(input.model.or(current.model)),
        serial_number: Set(input.serial_number.or(current.serial_number)),
        description: Set(input.description.or(current.description)),
        is_monitored: Set(input.is_monitored.unwrap_or(current.is_monitored)),
        snmp_enabled: Set(input.snmp_enabled.unwrap_or(current.snmp_enabled)),
        snmp_community: Set(input.snmp_community.or(current.snmp_community)),
        snmp_version: Set(input.snmp_version.or(current.snmp_version)),
        status: Set(input.status.unwrap_or(current.status)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    sync_device_monitor(&ctx.db, &row).await?;
    Ok(format::json(present(&ctx.db, row).await?)?)
}

async fn destroy(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    ResourceCleanupService::delete_device(&ctx.db, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn device_monitors(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let rows = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(id))
        .order_by_asc(monitors::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        present_monitors(&ctx.db, rows, RECENT_RESULTS_LIMIT).await?,
    )?)
}

fn metric_json(
    row: metrics_entity::Model,
    interface_names: &HashMap<i64, String>,
) -> serde_json::Value {
    let interface_name = row
        .interface_id
        .and_then(|interface_id| interface_names.get(&interface_id));
    serde_json::json!({"id":row.id,"deviceId":row.device_id,"interfaceId":row.interface_id,"interfaceName":interface_name,"metricName":row.name,"metricValue":row.value,"unit":row.unit,"createdAt":row.recorded_at.format("%d/%m/%Y %H:%M:%S").to_string()})
}

async fn metrics(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    let interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(id))
        .all(&ctx.db)
        .await?;
    let interface_names: HashMap<i64, String> = interfaces
        .iter()
        .filter(|interface| interface.admin_status.as_deref() == Some("up"))
        .map(|interface| (interface.id, interface.name.clone()))
        .collect();
    let visible_interface_ids: Vec<i64> = interface_names.keys().copied().collect();
    let base = metrics_entity::Entity::find()
        .filter(metrics_entity::Column::DeviceId.eq(id))
        .filter(if visible_interface_ids.is_empty() {
            metrics_entity::Column::InterfaceId.is_null()
        } else {
            metrics_entity::Column::InterfaceId
                .is_null()
                .or(metrics_entity::Column::InterfaceId.is_in(visible_interface_ids))
        })
        .order_by_desc(metrics_entity::Column::RecordedAt);
    let body = if let Some(page) = query.page {
        MaybePaged::Page(
            paginate_compat(&ctx.db, base, page, query.limit.unwrap_or(20), |metric| {
                metric_json(metric, &interface_names)
            })
            .await?,
        )
    } else {
        MaybePaged::List(
            base.limit(1000)
                .all(&ctx.db)
                .await?
                .into_iter()
                .map(|metric| metric_json(metric, &interface_names))
                .collect(),
        )
    };
    Ok(format::json(body)?)
}

fn event_json(row: alert_events::Model) -> serde_json::Value {
    serde_json::json!({"id":row.id,"deviceId":row.device_id,"eventType":row.status,"severity":row.severity,"message":row.message.unwrap_or_else(|| "Sem mensagem de detalhes".into()),"createdAt":row.created_at.format("%d/%m/%Y %H:%M:%S").to_string()})
}

async fn events(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    let base = alert_events::Entity::find()
        .filter(alert_events::Column::DeviceId.eq(id))
        .order_by_desc(alert_events::Column::CreatedAt);
    let body = if let Some(page) = query.page {
        MaybePaged::Page(
            paginate_compat(&ctx.db, base, page, query.limit.unwrap_or(20), event_json).await?,
        )
    } else {
        MaybePaged::List(
            base.limit(50)
                .all(&ctx.db)
                .await?
                .into_iter()
                .map(event_json)
                .collect(),
        )
    };
    Ok(format::json(body)?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/devices")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/monitors", get(device_monitors))
        .add("/{id}/metrics", get(metrics))
        .add("/{id}/events", get(events))
}
