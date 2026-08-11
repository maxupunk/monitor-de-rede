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

/// Agrupa os OIDs dos itens em lotes de [`OID_BATCH_SIZE`].
///
/// O tamanho não é arbitrário e não deve ser "otimizado": é o mesmo do backend
/// anterior. Um `GET` SNMP com varbinds demais estoura o PDU de agentes
/// embarcados, que respondem `tooBig` e derrubam a coleta **inteira** — em vez
/// de devolver o que caberia.
#[must_use]
pub fn oid_batches(items: &[zabbix_template_items::Model]) -> Vec<Vec<&str>> {
    items
        .chunks(OID_BATCH_SIZE)
        .map(|batch| batch.iter().map(|item| item.snmp_oid.as_str()).collect())
        .collect()
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
    let batches = oid_batches(&items);
    for (batch, oids) in items.chunks(OID_BATCH_SIZE).zip(batches) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(indice: u32) -> zabbix_template_items::Model {
        zabbix_template_items::Model {
            id: i64::from(indice),
            template_id: 1,
            zabbix_uuid: None,
            name: format!("Item {indice}"),
            key: format!("item.{indice}"),
            snmp_oid: format!("1.3.6.1.4.1.9.{indice}"),
            value_type: "FLOAT".into(),
            units: None,
            multiplier: None,
            created_at: Utc::now().into(),
        }
    }

    #[test]
    fn os_oids_saem_em_lote_de_seis() {
        let items: Vec<_> = (1..=13).map(item).collect();
        let batches = oid_batches(&items);

        // 13 itens = 6 + 6 + 1.
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), OID_BATCH_SIZE);
        assert_eq!(batches[1].len(), OID_BATCH_SIZE);
        assert_eq!(batches[2], vec!["1.3.6.1.4.1.9.13"]);

        // Nenhum OID se perde nem se repete no fatiamento.
        let todos: Vec<_> = batches.concat();
        assert_eq!(todos.len(), items.len());
        assert_eq!(todos[0], "1.3.6.1.4.1.9.1");
    }

    #[test]
    fn lote_exato_nao_cria_requisicao_vazia() {
        let items: Vec<_> = (1..=6).map(item).collect();
        assert_eq!(oid_batches(&items).len(), 1);
        assert!(oid_batches(&[]).is_empty());
    }
}
