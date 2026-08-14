//! Persistência atômica da observação de um monitor e atualização do dispositivo.

use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    models::{devices, monitor_results, monitors},
    services::{
        alerts,
        events::EventBus,
        monitoring::{
            contracts::{CheckMetric, CheckResult, MonitorStatus},
            device_status::{self, DeviceStatus},
        },
        shared::errors::AppResult,
    },
};

/// Extrai a medida que alimenta os gráficos de latência, na mesma precedência
/// usada pelo backend anterior para não zerar monitores TCP ou DNS.
#[must_use]
pub fn pick_latency_metric(metrics: &[CheckMetric]) -> Option<&CheckMetric> {
    const PRECEDENCE: [&str; 5] = [
        "latency",
        "response_time",
        "dns_lookup_time",
        "resolution_time",
        "connect_time",
    ];
    PRECEDENCE
        .iter()
        .find_map(|name| metrics.iter().find(|metric| metric.name == *name))
}

/// Persiste uma observação e retorna `None` quando o monitor foi apagado entre
/// a execução e a gravação — condição normal em uma operação concorrente.
pub async fn process_result(
    ctx: &AppContext,
    monitor_id: i64,
    result: &CheckResult,
    probe_id: Option<i64>,
) -> AppResult<Option<monitor_results::Model>> {
    let Some(monitor) = monitors::Entity::find_by_id(monitor_id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(None);
    };
    let latency = pick_latency_metric(&result.metrics).map(|metric| metric.value);
    // Guardado antes da escrita: `statusChanged` no evento SSE compara com o
    // status que a linha tinha, não com o que acabamos de gravar.
    let previous_status = monitor.status.clone();
    let stored = monitor_results::ActiveModel {
        monitor_id: Set(monitor.id),
        probe_id: Set(probe_id.or(monitor.probe_id)),
        status: Set(result.status.as_str().to_string()),
        started_at: Set(result.started_at.into()),
        finished_at: Set(result.finished_at.into()),
        duration_ms: Set(result.duration_ms.clamp(0, i64::from(i32::MAX)) as i32),
        latency_ms: Set(latency),
        message: Set(result.message.clone()),
        data: Set(Some(result.data.clone())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    let mut active: monitors::ActiveModel = monitor.clone().into();
    active.status = Set(result.status.as_str().to_string());
    active.last_run_at = Set(Some(result.finished_at.into()));
    active.update(&ctx.db).await?;

    let mut device_name = None;
    if let Some(device_id) = monitor.device_id {
        if let Some(device) = devices::Entity::find_by_id(device_id).one(&ctx.db).await? {
            let observed = match result.status {
                MonitorStatus::Up => Some(DeviceStatus::Online),
                MonitorStatus::Down => Some(DeviceStatus::Offline),
                MonitorStatus::Warning => Some(DeviceStatus::Warning),
                MonitorStatus::Unknown | MonitorStatus::Disabled => None,
            };
            let seen_at = (result.status == MonitorStatus::Up).then_some(result.finished_at);
            device_name = Some(device.name.clone());
            device_status::refresh_from_monitors(ctx, &device, observed, seen_at).await?;
        }
    }

    // Avaliar alertas é best-effort pelo mesmo motivo da publicação: a
    // observação técnica já está gravada e não pode ser desfeita porque o
    // motor de alertas topou com uma regra corrompida.
    if let Err(error) = alerts::manager::evaluate_monitor_result(ctx, &monitor, result).await {
        tracing::warn!(%error, monitor_id = monitor.id, "falha ao avaliar alertas do monitor");
    }

    // Publicação e persistência de SSE são best-effort: uma falha de relay não
    // pode abortar nem apagar a observação técnica já gravada.
    if let Ok(events) = EventBus::from_context(ctx) {
        // `monitor:result` é o nome que `stores/events.ts` despacha; o payload
        // alimenta a timeline e o sparkline sem esperar um refetch da lista.
        if let Err(error) = events
            .publish(
                &ctx.db,
                "monitor:result",
                serde_json::json!({
                    "monitorId": monitor.id,
                    "id": monitor.id,
                    "name": monitor.name,
                    "type": monitor.r#type,
                    "deviceId": monitor.device_id,
                    "deviceName": device_name,
                    "resultId": stored.id,
                    "status": result.status.as_str(),
                    "previousStatus": previous_status,
                    "statusChanged": previous_status != result.status.as_str(),
                    "latencyMs": latency,
                    "durationMs": result.duration_ms,
                    "message": result.message,
                    "startedAt": result.started_at.to_rfc3339(),
                    "finishedAt": result.finished_at.to_rfc3339(),
                }),
            )
            .await
        {
            tracing::warn!(%error, monitor_id = monitor.id, "falha ao publicar evento de monitor");
        }
    }
    Ok(Some(stored))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedencia_inclui_tcp_e_dns() {
        let metrics = vec![
            CheckMetric {
                name: "connect_time".into(),
                value: 4.0,
                unit: "ms".into(),
            },
            CheckMetric {
                name: "latency".into(),
                value: 2.0,
                unit: "ms".into(),
            },
        ];
        assert_eq!(
            pick_latency_metric(&metrics).map(|metric| metric.value),
            Some(2.0)
        );
        assert_eq!(
            pick_latency_metric(&metrics[..1]).map(|metric| metric.value),
            Some(4.0)
        );
    }
}
