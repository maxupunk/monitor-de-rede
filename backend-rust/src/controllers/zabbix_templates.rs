//! Importação e gerenciamento dos templates Zabbix.

use axum::{http::StatusCode, response::IntoResponse};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};

use crate::{
    dtos::resources::ZabbixImportInput,
    models::_entities::{devices, zabbix_template_items, zabbix_templates},
    services::{
        maintenance::resource_cleanup::ResourceCleanupService,
        shared::errors::{AppError, AppResult},
    },
};

#[derive(Debug)]
struct ParsedItem {
    uuid: Option<String>,
    name: String,
    key: String,
    snmp_oid: String,
    value_type: String,
    units: Option<String>,
    multiplier: Option<f32>,
}
#[derive(Debug)]
struct ParsedTemplate {
    uuid: Option<String>,
    name: String,
    description: Option<String>,
    version: Option<String>,
    items: Vec<ParsedItem>,
    skipped: usize,
}
fn string_at(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .filter(|text| !text.trim().is_empty())
}
fn parse_export(raw: &serde_json::Value) -> AppResult<Vec<ParsedTemplate>> {
    let root = raw.get("zabbix_export").unwrap_or(raw);
    let list = root
        .get("templates")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AppError::validation("O export do Zabbix não contém templates"))?;
    let mut output = Vec::new();
    for value in list {
        let name = string_at(value, &["name", "template"])
            .ok_or_else(|| AppError::validation("Template Zabbix sem nome"))?;
        let mut items = Vec::new();
        let mut skipped = 0;
        for item in value
            .get("items")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(key) = string_at(item, &["key_", "key"]) else {
                skipped += 1;
                continue;
            };
            let Some(oid) = string_at(item, &["snmp_oid", "snmpOid"]) else {
                skipped += 1;
                continue;
            };
            items.push(ParsedItem {
                uuid: string_at(item, &["uuid"]),
                name: string_at(item, &["name"]).unwrap_or_else(|| key.clone()),
                key,
                snmp_oid: oid,
                value_type: string_at(item, &["value_type", "valueType"])
                    .unwrap_or_else(|| "FLOAT".into()),
                units: string_at(item, &["units"]),
                multiplier: item
                    .get("multiplier")
                    .and_then(serde_json::Value::as_f64)
                    .map(|value| value as f32),
            });
        }
        output.push(ParsedTemplate {
            uuid: string_at(value, &["uuid"]),
            name,
            description: string_at(value, &["description"]),
            version: string_at(root, &["version"]),
            items,
            skipped,
        });
    }
    if output.is_empty() {
        return Err(AppError::validation(
            "Nenhum template Zabbix foi encontrado",
        ));
    }
    Ok(output)
}
async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let templates = zabbix_templates::Entity::find()
        .order_by_asc(zabbix_templates::Column::Name)
        .all(&ctx.db)
        .await?;
    let mut output = Vec::new();
    for template in templates {
        let items = zabbix_template_items::Entity::find()
            .filter(zabbix_template_items::Column::TemplateId.eq(template.id))
            .all(&ctx.db)
            .await?;
        let count = devices::Entity::find()
            .filter(devices::Column::ZabbixTemplateId.eq(template.id))
            .count(&ctx.db)
            .await?;
        output.push(serde_json::json!({"id":template.id,"zabbixUuid":template.zabbix_uuid,"name":template.name,"description":template.description,"zabbixVersion":template.zabbix_version,"importedAt":template.imported_at.to_rfc3339(),"deviceCount":count,"items":items.into_iter().map(|item|serde_json::json!({"id":item.id,"name":item.name,"key":item.key,"snmpOid":item.snmp_oid,"valueType":item.value_type,"units":item.units,"multiplier":item.multiplier})).collect::<Vec<_>>() }));
    }
    Ok(format::json(output)?)
}
async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let template = zabbix_templates::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Template Zabbix não encontrado"))?;
    let items = zabbix_template_items::Entity::find()
        .filter(zabbix_template_items::Column::TemplateId.eq(id))
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        serde_json::json!({"id":template.id,"zabbixUuid":template.zabbix_uuid,"name":template.name,"description":template.description,"zabbixVersion":template.zabbix_version,"rawExport":template.raw_export,"importedAt":template.imported_at.to_rfc3339(),"items":items}),
    )?)
}
async fn store(
    State(ctx): State<AppContext>,
    Json(input): Json<ZabbixImportInput>,
) -> AppResult<Response> {
    let raw: serde_json::Value = serde_json::from_str(&input.content)
        .map_err(|_| AppError::business_rule("O conteúdo enviado não é um JSON válido."))?;
    let parsed = parse_export(&raw)?;
    let txn = ctx.db.begin().await?;
    let mut imported = Vec::new();
    for template in parsed {
        let existing = match &template.uuid {
            Some(uuid) => {
                zabbix_templates::Entity::find()
                    .filter(zabbix_templates::Column::ZabbixUuid.eq(uuid))
                    .one(&txn)
                    .await?
            }
            None => None,
        };
        let row = if let Some(existing) = existing {
            zabbix_template_items::Entity::delete_many()
                .filter(zabbix_template_items::Column::TemplateId.eq(existing.id))
                .exec(&txn)
                .await?;
            let mut active: zabbix_templates::ActiveModel = existing.into();
            active.name = Set(template.name.clone());
            active.description = Set(template.description.clone());
            active.zabbix_version = Set(template.version.clone());
            active.raw_export = Set(raw.clone());
            active.imported_at = Set(Utc::now().into());
            active.update(&txn).await?
        } else {
            zabbix_templates::ActiveModel {
                zabbix_uuid: Set(template.uuid.clone()),
                name: Set(template.name.clone()),
                description: Set(template.description.clone()),
                zabbix_version: Set(template.version.clone()),
                raw_export: Set(raw.clone()),
                imported_at: Set(Utc::now().into()),
                ..Default::default()
            }
            .insert(&txn)
            .await?
        };
        let item_count = template.items.len();
        for item in template.items {
            zabbix_template_items::ActiveModel {
                template_id: Set(row.id),
                zabbix_uuid: Set(item.uuid),
                name: Set(item.name),
                key: Set(item.key),
                snmp_oid: Set(item.snmp_oid),
                value_type: Set(item.value_type),
                units: Set(item.units),
                multiplier: Set(item.multiplier),
                ..Default::default()
            }
            .insert(&txn)
            .await?;
        }
        imported.push(serde_json::json!({"id":row.id,"name":row.name,"itemCount":item_count,"skippedItems":template.skipped}));
    }
    txn.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"templates": imported})),
    )
        .into_response())
}
async fn destroy(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    zabbix_templates::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Template Zabbix não encontrado"))?;
    ResourceCleanupService::delete_zabbix_template(&ctx.db, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/zabbix-templates")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).delete(destroy))
}
