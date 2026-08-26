//! Persistência atômica da observação de um monitor e atualização do dispositivo.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};

use crate::{
    models::{_entities::metrics, devices, monitor_results, monitors},
    services::{
        alerts,
        events::EventBus,
        monitoring::{
            contracts::{CheckMetric, CheckResult, MonitorStatus},
            device_status::{self, DeviceStatus},
            health::series,
        },
        shared::errors::AppResult,
    },
};

/// Extrai a medida que alimenta os gráficos de latência na precedência que
/// evita zerar monitores TCP ou DNS.
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

/// As séries que pertencem ao **dispositivo**, e não à checagem (§3.1).
///
/// A fronteira decide o volume do banco. `monitor_results` guarda o desfecho
/// de uma checagem — status, duração, latência, mensagem — e vive 14 dias;
/// `metrics` guarda a grandeza contínua do equipamento e vive 30. Latência e
/// perda de pacote **ficam onde estão**: `monitor_results.latency_ms` tem
/// índice próprio e alimenta o sparkline, e copiá-las a cada ciclo multiplica
/// a tabela de maior volume do sistema sem acrescentar informação.
///
/// A lista é fechada de propósito. Um checker que invente um nome novo não
/// passa a escrever em `metrics` por acidente: alguém precisa acrescentá-lo
/// aqui, que é onde a decisão de retenção está escrita.
const DEVICE_SERIES: [&str; 8] = [
    series::CPU_USAGE,
    series::MEMORY_USAGE,
    series::STORAGE_USAGE,
    series::LOAD_AVERAGE_1M,
    series::PROCESS_MEMORY_BYTES,
    series::UPTIME_SECONDS,
    series::IN_BPS,
    series::OUT_BPS,
];

/// Verdadeiro para as medidas que viram série do dispositivo.
#[must_use]
pub fn is_device_series(name: &str) -> bool {
    DEVICE_SERIES.contains(&name)
}

/// Grava em `metrics` as medidas de série de dispositivo de um resultado.
///
/// Uma passagem genérica, válida para **qualquer** checker que tenha
/// `device_id` — não um gravador do servidor. É o que faz `/devices/{id}/metrics`
/// e os widgets de CPU e memória aceitarem o servidor sem uma linha de
/// frontend nova.
///
/// Roda dentro da mesma transação da observação: uma coleta cujo resultado
/// ficou gravado e cujas séries se perderam produziria um gráfico com buraco e
/// nenhum sinal de erro.
async fn record_device_series<C>(
    txn: &C,
    monitor: &monitors::Model,
    result: &CheckResult,
) -> AppResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    let Some(device_id) = monitor.device_id else {
        return Ok(());
    };
    let recorded_at = result.finished_at.fixed_offset();
    let linhas: Vec<metrics::ActiveModel> = result
        .metrics
        .iter()
        .filter(|metric| is_device_series(&metric.name) && metric.value.is_finite())
        .map(|metric| metrics::ActiveModel {
            device_id: Set(device_id),
            interface_id: Set(None),
            // A série é do dispositivo, mas saber qual checagem a produziu é o
            // que permite ao histórico do monitor mostrá-la sem adivinhação.
            monitor_id: Set(Some(monitor.id)),
            name: Set(metric.name.clone()),
            value: Set(metric.value),
            unit: Set(metric.unit.clone()),
            recorded_at: Set(recorded_at),
            ..Default::default()
        })
        .collect();
    if linhas.is_empty() {
        return Ok(());
    }
    metrics::Entity::insert_many(linhas).exec(txn).await?;
    Ok(())
}

/// Persiste uma observação e retorna `None` quando o monitor foi apagado entre
/// a execução e a gravação — condição normal em uma operação concorrente.
pub async fn process_result(
    ctx: &AppContext,
    monitor_id: i64,
    result: &CheckResult,
    probe_id: Option<i64>,
) -> AppResult<Option<monitor_results::Model>> {
    let txn = ctx.db.begin().await?;
    let Some(monitor) = monitors::Entity::find_by_id(monitor_id).one(&txn).await? else {
        txn.commit().await?;
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
    .insert(&txn)
    .await?;

    // Séries do dispositivo, na mesma transação da observação.
    record_device_series(&txn, &monitor, result).await?;

    // O histórico aceita toda observação, mas apenas um resultado mais novo
    // pode alterar o estado corrente. A condição fica no UPDATE (e não só num
    // `if` em Rust) para fechar a corrida entre dois probes que respondem ao
    // mesmo tempo.
    let updated = monitors::Entity::update_many()
        .col_expr(
            monitors::Column::Status,
            Expr::value(result.status.as_str()),
        )
        .col_expr(
            monitors::Column::LastRunAt,
            Expr::value(result.finished_at.fixed_offset()),
        )
        .col_expr(monitors::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(monitors::Column::Id.eq(monitor.id))
        .filter(
            Condition::any()
                .add(monitors::Column::LastRunAt.is_null())
                .add(monitors::Column::LastRunAt.lt(result.started_at.fixed_offset())),
        )
        .exec(&txn)
        .await?;
    txn.commit().await?;

    if updated.rows_affected == 0 {
        tracing::debug!(
            monitor_id = monitor.id,
            result_started_at = %result.started_at,
            "resultado histórico não alterou o estado atual"
        );
        return Ok(Some(stored));
    }

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
