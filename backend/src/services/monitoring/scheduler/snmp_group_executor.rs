//! Execução consolidada de múltiplos monitores SNMP de um mesmo dispositivo local.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};

use crate::{
    models::{devices, monitors},
    services::{
        monitoring::{execution_guard::try_acquire_snmp_device, result_processor::process_result},
        shared::errors::AppResult,
        snmp::service as snmp_service,
    },
};

use super::cadence::is_device_collection_due;

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

/// Dispositivos com SNMP ligado que nenhum monitor SNMP local agenda e cuja
/// coleta venceu pelo intervalo do próprio dispositivo.
///
/// O scheduler é o **único** dono da cadência SNMP: quem tem monitor é
/// coletado pelo grupo acima no `next_run_at` dele; o resto, por aqui. Os dois
/// conjuntos são disjuntos, então nenhum equipamento é consultado duas vezes
/// no mesmo intervalo — e o histórico deixa de depender de haver uma aba
/// aberta, como dependia quando o coletor da topologia fazia esse papel.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn due_unmonitored_snmp_devices(
    ctx: &AppContext,
    now: DateTime<Utc>,
) -> AppResult<Vec<devices::Model>> {
    let agendados: HashSet<i64> = monitors::Entity::find()
        .select_only()
        .column(monitors::Column::DeviceId)
        .filter(monitors::Column::Type.eq("snmp"))
        .filter(monitors::Column::Enabled.eq(true))
        .filter(monitors::Column::ProbeId.is_null())
        .filter(monitors::Column::DeviceId.is_not_null())
        .distinct()
        .into_tuple::<Option<i64>>()
        .all(&ctx.db)
        .await?
        .into_iter()
        .flatten()
        .collect();
    let candidatos = devices::Entity::find()
        .filter(devices::Column::SnmpEnabled.eq(true))
        .filter(devices::Column::IsMonitored.eq(true))
        .all(&ctx.db)
        .await?;
    Ok(candidatos
        .into_iter()
        .filter(|device| {
            device
                .ip_address
                .as_deref()
                .is_some_and(|ip| !ip.trim().is_empty())
        })
        .filter(|device| !agendados.contains(&device.id))
        .filter(|device| {
            is_device_collection_due(device.id, i64::from(device.snmp_poll_interval_seconds), now)
        })
        .collect())
}

/// Coleta um dispositivo sem monitor SNMP: interfaces, taxas e sistema, com o
/// mesmo `collect_device` dos monitores.
///
/// # Errors
///
/// Propaga erro da coleta.
pub async fn collect_unmonitored_device(
    ctx: &AppContext,
    device: &devices::Model,
) -> AppResult<()> {
    let Some(_guard) = try_acquire_snmp_device(device.id) else {
        tracing::debug!(
            device_id = device.id,
            "coleta SNMP do dispositivo já está em andamento"
        );
        return Ok(());
    };
    snmp_service::collect_device(ctx, device).await?;
    Ok(())
}
