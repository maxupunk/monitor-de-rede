//! Remoção explícita de agregados e de seus históricos.
//!
//! As FKs protegem a integridade, mas não cobrem todos os dados históricos que
//! pertencem a um recurso. Centralizar a sequência aqui impede que cada
//! controller apague um subconjunto diferente do agregado.

use sea_orm::{
    sea_query::Expr, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, ExprTrait,
    QueryFilter,
};

use crate::{
    models::{
        _entities::{
            alert_events, alert_rules, device_interfaces, device_links, metrics, monitor_results,
            monitor_results_hourly,
        },
        devices, monitors, probes,
    },
    shared::{errors::AppResult, transaction::begin_write},
};

/// Mantém cada `IN (...)` abaixo do limite de 999 variáveis do SQLite.
const DELETE_CHUNK_SIZE: usize = 400;

/// Serviço responsável pelas cinco cascatas manuais do contrato HTTP.
pub struct ResourceCleanupService;

impl ResourceCleanupService {
    /// Remove um monitor e todo histórico que não é coberto pela FK.
    ///
    /// Genérico sobre a conexão para poder rodar dentro de uma transação: são
    /// vários `DELETE` que só fazem sentido juntos, e quem chama em lote (a
    /// aplicação de monitores SNMP) precisa que a falha de um desfaça os
    /// anteriores.
    pub async fn delete_monitor<C: ConnectionTrait>(db: &C, monitor_id: i64) -> AppResult<()> {
        Self::delete_monitors_batch(db, &[monitor_id]).await
    }

    async fn delete_monitors_batch<C: ConnectionTrait>(
        db: &C,
        monitor_ids: &[i64],
    ) -> AppResult<()> {
        for chunk in monitor_ids.chunks(DELETE_CHUNK_SIZE) {
            monitor_results_hourly::Entity::delete_many()
                .filter(monitor_results_hourly::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(db)
                .await?;
            monitor_results::Entity::delete_many()
                .filter(monitor_results::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(db)
                .await?;
            metrics::Entity::delete_many()
                .filter(metrics::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(db)
                .await?;
            alert_events::Entity::delete_many()
                .filter(alert_events::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(db)
                .await?;
            alert_rules::Entity::delete_many()
                .filter(alert_rules::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(db)
                .await?;
            monitors::Entity::delete_many()
                .filter(monitors::Column::Id.is_in(chunk.iter().copied()))
                .exec(db)
                .await?;
        }
        Ok(())
    }

    async fn delete_device_in_txn<C: ConnectionTrait>(db: &C, device_id: i64) -> AppResult<()> {
        let monitor_ids: Vec<i64> = monitors::Entity::find()
            .filter(monitors::Column::DeviceId.eq(device_id))
            .all(db)
            .await?
            .into_iter()
            .map(|monitor| monitor.id)
            .collect();
        if !monitor_ids.is_empty() {
            Self::delete_monitors_batch(db, &monitor_ids).await?;
        }

        let interface_ids: Vec<i64> = device_interfaces::Entity::find()
            .filter(device_interfaces::Column::DeviceId.eq(device_id))
            .all(db)
            .await?
            .into_iter()
            .map(|item| item.id)
            .collect();
        for chunk in interface_ids.chunks(DELETE_CHUNK_SIZE) {
            metrics::Entity::delete_many()
                .filter(metrics::Column::InterfaceId.is_in(chunk.iter().copied()))
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

    /// Remove um dispositivo, seus monitores, interfaces, links e histórico.
    pub async fn delete_device(db: &DatabaseConnection, device_id: i64) -> AppResult<()> {
        let txn = begin_write(db).await?;
        Self::delete_device_in_txn(&txn, device_id).await?;
        txn.commit().await?;
        Ok(())
    }

    /// Apaga todo o histórico (métricas, resultados de monitor, rollup horário, eventos de alerta e logs)
    /// pertencente a um dispositivo, preservando o cadastro do dispositivo e seus monitores/interfaces.
    pub async fn clear_device_history(
        db: &DatabaseConnection,
        logs_db: Option<&DatabaseConnection>,
        device_id: i64,
    ) -> AppResult<()> {
        let txn = begin_write(db).await?;

        let monitor_ids: Vec<i64> = monitors::Entity::find()
            .filter(monitors::Column::DeviceId.eq(device_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|m| m.id)
            .collect();
        for chunk in monitor_ids.chunks(DELETE_CHUNK_SIZE) {
            monitor_results_hourly::Entity::delete_many()
                .filter(monitor_results_hourly::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(&txn)
                .await?;
            monitor_results::Entity::delete_many()
                .filter(monitor_results::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(&txn)
                .await?;
            metrics::Entity::delete_many()
                .filter(metrics::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(&txn)
                .await?;
            alert_events::Entity::delete_many()
                .filter(alert_events::Column::MonitorId.is_in(chunk.iter().copied()))
                .exec(&txn)
                .await?;
        }

        let interface_ids: Vec<i64> = device_interfaces::Entity::find()
            .filter(device_interfaces::Column::DeviceId.eq(device_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|item| item.id)
            .collect();
        for chunk in interface_ids.chunks(DELETE_CHUNK_SIZE) {
            metrics::Entity::delete_many()
                .filter(metrics::Column::InterfaceId.is_in(chunk.iter().copied()))
                .exec(&txn)
                .await?;
        }

        metrics::Entity::delete_many()
            .filter(metrics::Column::DeviceId.eq(device_id))
            .exec(&txn)
            .await?;

        alert_events::Entity::delete_many()
            .filter(alert_events::Column::DeviceId.eq(device_id))
            .exec(&txn)
            .await?;

        txn.commit().await?;

        if let Some(logs_conn) = logs_db {
            if let Err(error) = crate::models::logs::device_logs::Entity::delete_many()
                .filter(crate::models::logs::device_logs::Column::DeviceId.eq(device_id))
                .exec(logs_conn)
                .await
            {
                tracing::warn!(%error, device_id, "falha ao limpar device_logs do dispositivo");
            }
        }

        Ok(())
    }

    /// Remove um site e os dispositivos que fazem parte dele.
    pub async fn delete_site(db: &DatabaseConnection, site_id: i64) -> AppResult<()> {
        let txn = begin_write(db).await?;
        let device_rows = devices::Entity::find()
            .filter(devices::Column::SiteId.eq(site_id))
            .all(&txn)
            .await?;
        for device in device_rows {
            Self::delete_device_in_txn(&txn, device.id).await?;
        }
        alert_rules::Entity::delete_many()
            .filter(alert_rules::Column::SiteId.eq(site_id))
            .exec(&txn)
            .await?;
        probes::Entity::update_many()
            .col_expr(probes::Column::SiteId, Expr::value(None::<i64>))
            .filter(probes::Column::SiteId.eq(site_id))
            .exec(&txn)
            .await?;
        crate::models::sites::Entity::delete_by_id(site_id)
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(())
    }

    /// Remove um probe e os monitores atribuídos a ele, sem deixar resultados órfãos.
    pub async fn delete_probe(db: &DatabaseConnection, probe_id: i64) -> AppResult<()> {
        let txn = begin_write(db).await?;
        let monitor_ids: Vec<i64> = monitors::Entity::find()
            .filter(monitors::Column::ProbeId.eq(probe_id))
            .all(&txn)
            .await?
            .into_iter()
            .map(|monitor| monitor.id)
            .collect();
        if !monitor_ids.is_empty() {
            Self::delete_monitors_batch(&txn, &monitor_ids).await?;
        }
        monitor_results::Entity::delete_many()
            .filter(monitor_results::Column::ProbeId.eq(probe_id))
            .exec(&txn)
            .await?;
        probes::Entity::delete_by_id(probe_id).exec(&txn).await?;
        txn.commit().await?;
        Ok(())
    }
}
