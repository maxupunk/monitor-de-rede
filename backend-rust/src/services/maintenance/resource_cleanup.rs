//! Remoção explícita de agregados e de seus históricos.
//!
//! As FKs protegem a integridade, mas não cobrem todos os dados históricos que
//! pertencem a um recurso. Centralizar a sequência aqui impede que cada
//! controller apague um subconjunto diferente do agregado.

use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, ExprTrait, QueryFilter,
};

use crate::{
    models::{
        _entities::{
            alert_events, alert_rules, device_interfaces, device_links, metrics, monitor_results,
            zabbix_template_items,
        },
        devices, monitors, probes, zabbix_templates,
    },
    services::shared::errors::AppResult,
};

/// Serviço responsável pelas cinco cascatas manuais do contrato HTTP.
pub struct ResourceCleanupService;

impl ResourceCleanupService {
    /// Remove um monitor e todo histórico que não é coberto pela FK.
    pub async fn delete_monitor(db: &DatabaseConnection, monitor_id: i64) -> AppResult<()> {
        monitor_results::Entity::delete_many()
            .filter(monitor_results::Column::MonitorId.eq(monitor_id))
            .exec(db)
            .await?;
        metrics::Entity::delete_many()
            .filter(metrics::Column::MonitorId.eq(monitor_id))
            .exec(db)
            .await?;
        alert_events::Entity::delete_many()
            .filter(alert_events::Column::MonitorId.eq(monitor_id))
            .exec(db)
            .await?;
        alert_rules::Entity::delete_many()
            .filter(alert_rules::Column::MonitorId.eq(monitor_id))
            .exec(db)
            .await?;
        monitors::Entity::delete_by_id(monitor_id).exec(db).await?;
        Ok(())
    }

    /// Remove um dispositivo, seus monitores, interfaces, links e histórico.
    pub async fn delete_device(db: &DatabaseConnection, device_id: i64) -> AppResult<()> {
        let monitor_rows = monitors::Entity::find()
            .filter(monitors::Column::DeviceId.eq(device_id))
            .all(db)
            .await?;
        for monitor in monitor_rows {
            Self::delete_monitor(db, monitor.id).await?;
        }

        let interfaces = device_interfaces::Entity::find()
            .filter(device_interfaces::Column::DeviceId.eq(device_id))
            .all(db)
            .await?;
        let interface_ids: Vec<i64> = interfaces.into_iter().map(|item| item.id).collect();
        if !interface_ids.is_empty() {
            metrics::Entity::delete_many()
                .filter(metrics::Column::InterfaceId.is_in(interface_ids))
                .exec(db)
                .await?;
        }
        device_interfaces::Entity::delete_many()
            .filter(device_interfaces::Column::DeviceId.eq(device_id))
            .exec(db)
            .await?;
        metrics::Entity::delete_many()
            .filter(metrics::Column::DeviceId.eq(device_id))
            .exec(db)
            .await?;
        alert_events::Entity::delete_many()
            .filter(alert_events::Column::DeviceId.eq(device_id))
            .exec(db)
            .await?;
        alert_rules::Entity::delete_many()
            .filter(alert_rules::Column::DeviceId.eq(device_id))
            .exec(db)
            .await?;
        device_links::Entity::delete_many()
            .filter(
                device_links::Column::SourceDeviceId
                    .eq(device_id)
                    .or(device_links::Column::TargetDeviceId.eq(device_id)),
            )
            .exec(db)
            .await?;
        devices::Entity::delete_by_id(device_id).exec(db).await?;
        Ok(())
    }

    /// Remove um site e os dispositivos que fazem parte dele.
    pub async fn delete_site(db: &DatabaseConnection, site_id: i64) -> AppResult<()> {
        let device_rows = devices::Entity::find()
            .filter(devices::Column::SiteId.eq(site_id))
            .all(db)
            .await?;
        for device in device_rows {
            Self::delete_device(db, device.id).await?;
        }
        alert_rules::Entity::delete_many()
            .filter(alert_rules::Column::SiteId.eq(site_id))
            .exec(db)
            .await?;
        probes::Entity::update_many()
            .col_expr(probes::Column::SiteId, Expr::value(None::<i64>))
            .filter(probes::Column::SiteId.eq(site_id))
            .exec(db)
            .await?;
        crate::models::sites::Entity::delete_by_id(site_id)
            .exec(db)
            .await?;
        Ok(())
    }

    /// Remove um probe e os monitores atribuídos a ele, sem deixar resultados órfãos.
    pub async fn delete_probe(db: &DatabaseConnection, probe_id: i64) -> AppResult<()> {
        let monitor_rows = monitors::Entity::find()
            .filter(monitors::Column::ProbeId.eq(probe_id))
            .all(db)
            .await?;
        for monitor in monitor_rows {
            Self::delete_monitor(db, monitor.id).await?;
        }
        monitor_results::Entity::delete_many()
            .filter(monitor_results::Column::ProbeId.eq(probe_id))
            .exec(db)
            .await?;
        probes::Entity::delete_by_id(probe_id).exec(db).await?;
        Ok(())
    }

    /// Remove um template, seus itens e desvincula os dispositivos existentes.
    pub async fn delete_zabbix_template(
        db: &DatabaseConnection,
        template_id: i64,
    ) -> AppResult<()> {
        devices::Entity::update_many()
            .col_expr(devices::Column::ZabbixTemplateId, Expr::value(None::<i64>))
            .filter(devices::Column::ZabbixTemplateId.eq(template_id))
            .exec(db)
            .await?;
        zabbix_template_items::Entity::delete_many()
            .filter(zabbix_template_items::Column::TemplateId.eq(template_id))
            .exec(db)
            .await?;
        zabbix_templates::Entity::delete_by_id(template_id)
            .exec(db)
            .await?;
        Ok(())
    }
}
