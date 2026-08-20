//! Execução consolidada de múltiplos monitores SNMP de um mesmo dispositivo local.

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::{
    models::{devices, monitors},
    services::{
        monitoring::{execution_guard::try_acquire_snmp_device, result_processor::process_result},
        shared::errors::AppResult,
        snmp::service as snmp_service,
    },
};

/// Monitores SNMP locais do mesmo dispositivo compartilham uma única coleta.
#[must_use]
pub fn local_snmp_device_id(monitor: &monitors::Model) -> Option<i64> {
    (monitor.r#type == "snmp" && monitor.probe_id.is_none())
        .then_some(monitor.device_id)
        .flatten()
}

/// Executa todos os monitores SNMP locais vinculados a um dispositivo em uma única requisição.
pub async fn execute_snmp_device_group(
    ctx: &AppContext,
    device_id: i64,
    scheduled_at: DateTime<Utc>,
) -> AppResult<()> {
    let Some(_guard) = try_acquire_snmp_device(device_id) else {
        tracing::debug!(device_id, "coleta SNMP do dispositivo já está em andamento");
        return Ok(());
    };
    let Some(device) = devices::Entity::find_by_id(device_id).one(&ctx.db).await? else {
        return Ok(());
    };
    let monitored_items = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors::Column::Type.eq("snmp"))
        .filter(monitors::Column::Enabled.eq(true))
        .filter(monitors::Column::ProbeId.is_null())
        .all(&ctx.db)
        .await?;
    let next_run_at = (scheduled_at
        + chrono::Duration::seconds(i64::from(device.snmp_poll_interval_seconds.max(1))))
    .into();
    for monitor in &monitored_items {
        let mut active: monitors::ActiveModel = monitor.clone().into();
        active.next_run_at = Set(Some(next_run_at));
        active.update(&ctx.db).await?;
    }
    for (monitor_id, result) in
        snmp_service::poll_device_monitors(ctx, &device, &monitored_items).await
    {
        process_result(ctx, monitor_id, &result, None).await?;
    }
    Ok(())
}
