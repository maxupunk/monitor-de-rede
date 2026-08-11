use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::{
    models::{
        _entities::{
            monitors as monitors_entity, zabbix_template_items as zabbix_template_items_entity,
        },
        devices, metrics, monitors, zabbix_template_items,
    },
    services::{shared::errors::AppResult, snmp::client::SnmpClient},
};

pub const OID_BATCH_SIZE: usize = 6;
pub const ZABBIX_TEMPLATE_MONITOR_NAME: &str = "Coleta de Template Zabbix";

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZabbixTemplateItemReading {
    pub key: String,
    pub value: Option<f64>,
    pub unit: Option<String>,
}

pub async fn preview(
    db: &sea_orm::DatabaseConnection,
    device: &devices::Model,
    client: &SnmpClient,
) -> AppResult<Vec<ZabbixTemplateItemReading>> {
    let Some(template_id) = device.zabbix_template_id else {
        return Ok(vec![]);
    };
    let items = zabbix_template_items::Entity::find()
        .filter(zabbix_template_items_entity::Column::TemplateId.eq(template_id))
        .all(db)
        .await?;
    let mut readings = Vec::with_capacity(items.len());
    for batch in items.chunks(OID_BATCH_SIZE) {
        let oids: Vec<_> = batch.iter().map(|item| item.snmp_oid.as_str()).collect();
        let values = client.get(&oids).await.ok();
        readings.extend(batch.iter().map(|item| {
            ZabbixTemplateItemReading {
                key: item.key.clone(),
                value: values
                    .as_ref()
                    .and_then(|values| values.get(&item.snmp_oid))
                    .and_then(Option::as_ref)
                    .and_then(|value| value.number())
                    .map(|value| value as f64 * item.multiplier.unwrap_or(1.0) as f64),
                unit: item.units.clone(),
            }
        }));
    }
    Ok(readings)
}

pub async fn collect(
    db: &sea_orm::DatabaseConnection,
    device: &devices::Model,
    client: &SnmpClient,
) -> AppResult<u64> {
    let readings = preview(db, device, client).await?;
    let recorded_at = Utc::now();
    let mut stored = 0;
    for reading in readings {
        let Some(value) = reading.value else {
            continue;
        };
        metrics::ActiveModel {
            device_id: Set(device.id),
            interface_id: Set(None),
            monitor_id: Set(None),
            name: Set(reading.key),
            value: Set(value),
            unit: Set(reading.unit.unwrap_or_default()),
            recorded_at: Set(recorded_at.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
        stored += 1;
    }
    Ok(stored)
}

/// Mantem exatamente um monitor de coleta para cada dispositivo com template.
/// O poll tambem chama esta funcao, portanto uma exclusao ou edicao manual e
/// corrigida no proximo ciclo sem duplicar monitores.
pub async fn sync_zabbix_template_monitor(
    db: &sea_orm::DatabaseConnection,
    device: &devices::Model,
) -> AppResult<()> {
    let existing = monitors::Entity::find()
        .filter(monitors_entity::Column::DeviceId.eq(Some(device.id)))
        .filter(monitors_entity::Column::Name.eq(ZABBIX_TEMPLATE_MONITOR_NAME))
        .one(db)
        .await?;
    let Some(template_id) = device.zabbix_template_id else {
        if let Some(monitor) = existing {
            monitors::Entity::delete_by_id(monitor.id).exec(db).await?;
        }
        return Ok(());
    };
    let configuration = serde_json::json!({ "templateId": template_id });
    if let Some(existing) = existing {
        monitors::ActiveModel {
            id: Set(existing.id),
            configuration: Set(configuration),
            enabled: Set(true),
            ..Default::default()
        }
        .update(db)
        .await?;
    } else {
        monitors::ActiveModel {
            device_id: Set(Some(device.id)),
            probe_id: Set(None),
            r#type: Set("zabbix".into()),
            name: Set(ZABBIX_TEMPLATE_MONITOR_NAME.into()),
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
    Ok(())
}
