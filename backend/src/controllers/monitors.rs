//! CRUD e acionamento manual dos monitores.

use axum::{extract::Query, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::{
    dtos::resources::{MonitorInput, PaginationQuery},
    models::{devices, monitor_results, monitors},
    services::{
        alerts::recovery,
        maintenance::resource_cleanup::ResourceCleanupService,
        monitoring::{
            device_status,
            execution_guard::{
                calculate_smart_timeout_seconds, try_acquire_monitor, try_acquire_snmp_device,
            },
            presenter::{present_monitors, MonitorResultPresentation, RECENT_RESULTS_LIMIT},
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        preferences,
        shared::{
            errors::{AppError, AppResult},
            pagination::paginate_compat,
        },
        snmp::service as snmp_service,
    },
};

fn build_configuration(
    kind: &str,
    supplied: Option<serde_json::Value>,
    target: Option<&str>,
    port: Option<i64>,
    fallback: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut config = supplied
        .or_else(|| fallback.cloned())
        .unwrap_or_else(|| serde_json::json!({}));
    let serde_json::Value::Object(ref mut object) = config else {
        return serde_json::json!({});
    };
    // O limite de resposta é definido automaticamente a partir do tipo e do
    // intervalo do monitor. Remove também valores salvos por versões antigas.
    object.remove("timeoutMs");
    if let Some(target) = target.filter(|target| !target.trim().is_empty()) {
        match kind.to_lowercase().as_str() {
            "ping" | "snmp" => {
                object.entry("host").or_insert_with(|| target.into());
            }
            "http" | "https" => {
                object.entry("url").or_insert_with(|| {
                    if target.starts_with("http") {
                        target.into()
                    } else {
                        format!("http://{target}").into()
                    }
                });
            }
            "tcp" => {
                object.entry("host").or_insert_with(|| target.into());
            }
            "dns" => {
                object.entry("domain").or_insert_with(|| target.into());
            }
            _ => {}
        }
    }
    if let Some(port) = port.filter(|port| (1..=65535).contains(port)) {
        object.entry("port").or_insert_with(|| port.into());
    }
    config
}

const SUPPORTED_MONITOR_TYPES: &[&str] = &[
    "ping",
    "http",
    "https",
    "tcp",
    "dns",
    "snmp",
    "ssl",
    "port_scan",
];

fn require_kind_name(input: &MonitorInput) -> AppResult<(&str, &str)> {
    let kind = input
        .monitor_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Tipo do monitor é obrigatório"))?;
    if !SUPPORTED_MONITOR_TYPES.contains(&kind.to_lowercase().as_str()) {
        return Err(AppError::validation(format!(
            "Tipo de monitor não suportado: '{kind}'."
        )));
    }
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Nome do monitor é obrigatório"))?;
    if name.contains('\n') || name.contains('\r') {
        return Err(AppError::validation(
            "Nome do monitor não pode conter quebra de linha.",
        ));
    }
    if let Some(interval) = input.interval_seconds {
        if interval < 1 {
            return Err(AppError::validation(
                "O intervalo deve ser de pelo menos 1 segundo.",
            ));
        }
    }
    Ok((kind, name))
}

async fn canonical_snmp_interval(
    ctx: &AppContext,
    device_id: Option<i64>,
    supplied_interval: Option<i32>,
) -> AppResult<Option<i32>> {
    let Some(device_id) = device_id else {
        return Ok(None);
    };
    let device = devices::Entity::find_by_id(device_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    if let Some(supplied_interval) = supplied_interval.map(|value| value.max(1)) {
        if supplied_interval != device.snmp_poll_interval_seconds {
            return Err(AppError::validation(
                "O intervalo de coleta SNMP é definido no dispositivo e se aplica a todos os itens SNMP vinculados a ele",
            ));
        }
    }
    Ok(Some(device.snmp_poll_interval_seconds))
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = monitors::Entity::find()
        .order_by_asc(monitors::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        present_monitors(&ctx.db, rows, RECENT_RESULTS_LIMIT).await?,
    )?)
}

async fn store(
    State(ctx): State<AppContext>,
    Json(input): Json<MonitorInput>,
) -> AppResult<Response> {
    let (kind, name) = require_kind_name(&input)?;
    let kind = kind.to_string();
    let name = name.to_string();
    let enabled = input.enabled.or(input.is_enabled).unwrap_or(true);
    // O padrão vem das preferências, não de um literal: é este o ponto de
    // consumo que faz "Intervalo padrão de coleta por Ping" significar alguma
    // coisa. Monitor SNMP vinculado a dispositivo continua herdando o intervalo
    // do próprio dispositivo — a preferência não o atropela.
    let intervalo_padrao = preferences::load(&ctx.db)
        .await?
        .default_ping_interval_seconds;
    let interval_seconds = if kind.eq_ignore_ascii_case("snmp") {
        canonical_snmp_interval(&ctx, input.device_id, input.interval_seconds)
            .await?
            .unwrap_or_else(|| input.interval_seconds.unwrap_or(intervalo_padrao).max(1))
    } else {
        input.interval_seconds.unwrap_or(intervalo_padrao).max(1)
    };
    let timeout_seconds = calculate_smart_timeout_seconds(&kind, interval_seconds)
        .min((interval_seconds - 1).max(1))
        .max(1);
    let config = build_configuration(
        &kind,
        input.configuration,
        input.target.as_deref(),
        input.port,
        None,
    );
    let row = monitors::ActiveModel {
        device_id: Set(input.device_id),
        probe_id: Set(input.probe_id),
        r#type: Set(kind),
        name: Set(name),
        configuration: Set(config),
        interval_seconds: Set(interval_seconds),
        timeout_seconds: Set(timeout_seconds),
        retry_count: Set(input.retry_count.unwrap_or(3).max(0)),
        enabled: Set(enabled),
        status: Set(input.status.unwrap_or_else(|| "unknown".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    let mut response = present_monitors(&ctx.db, vec![row], RECENT_RESULTS_LIMIT).await?;
    Ok((StatusCode::CREATED, Json(response.remove(0))).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = monitors::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Monitor não encontrado"))?;
    let results = monitor_results::Entity::find()
        .filter(monitor_results::Column::MonitorId.eq(id))
        .order_by_desc(monitor_results::Column::StartedAt)
        .limit(100)
        .all(&ctx.db)
        .await?;
    let latencies: Vec<f64> = results
        .iter()
        .filter_map(|result| result.latency_ms)
        .collect();
    let total_checks = results.len();
    let up_checks = results
        .iter()
        .filter(|result| result.status == "up")
        .count();
    let mut view = serde_json::to_value(present_monitors(&ctx.db, vec![row], 100).await?.remove(0))
        .map_err(|error| AppError::Internal(error.into()))?;
    view["stats"] = serde_json::json!({
        "avgLatency": (!latencies.is_empty()).then(|| (latencies.iter().sum::<f64>() / latencies.len() as f64).round()),
        "minLatency": latencies.iter().copied().reduce(f64::min), "maxLatency": latencies.iter().copied().reduce(f64::max),
        "lastLatency": latencies.first(), "uptimePercentage": if total_checks == 0 {100.0} else {(up_checks as f64 * 1000.0 / total_checks as f64).round() / 10.0},
        "totalChecks": total_checks, "upChecks": up_checks });
    Ok(format::json(view)?)
}

async fn update(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<MonitorInput>,
) -> AppResult<Response> {
    let current = monitors::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Monitor não encontrado"))?;
    if let Some(kind_val) = input
        .monitor_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if !SUPPORTED_MONITOR_TYPES.contains(&kind_val.to_lowercase().as_str()) {
            return Err(AppError::validation(format!(
                "Tipo de monitor não suportado: '{kind_val}'."
            )));
        }
    }
    let kind = input
        .monitor_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.r#type)
        .to_string();
    if let Some(name_val) = input.name.as_deref() {
        if name_val.contains('\n') || name_val.contains('\r') {
            return Err(AppError::validation(
                "Nome do monitor não pode conter quebra de linha.",
            ));
        }
    }
    if let Some(interval) = input.interval_seconds {
        if interval < 1 {
            return Err(AppError::validation(
                "O intervalo deve ser de pelo menos 1 segundo.",
            ));
        }
    }
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.name)
        .to_string();
    let enabled = input
        .enabled
        .or(input.is_enabled)
        .unwrap_or(current.enabled);
    let device_id = input.device_id.or(current.device_id);
    let interval_seconds = if kind.eq_ignore_ascii_case("snmp") {
        canonical_snmp_interval(&ctx, device_id, input.interval_seconds)
            .await?
            .unwrap_or_else(|| {
                input
                    .interval_seconds
                    .unwrap_or(current.interval_seconds)
                    .max(1)
            })
    } else {
        input
            .interval_seconds
            .unwrap_or(current.interval_seconds)
            .max(1)
    };
    let timeout_seconds = calculate_smart_timeout_seconds(&kind, interval_seconds)
        .min((interval_seconds - 1).max(1))
        .max(1);
    let row = monitors::ActiveModel {
        id: Set(id),
        device_id: Set(device_id),
        probe_id: Set(input.probe_id.or(current.probe_id)),
        r#type: Set(kind.clone()),
        name: Set(name),
        configuration: Set(build_configuration(
            &kind,
            input.configuration,
            input.target.as_deref(),
            input.port,
            Some(&current.configuration),
        )),
        interval_seconds: Set(interval_seconds),
        timeout_seconds: Set(timeout_seconds),
        retry_count: Set(input.retry_count.unwrap_or(current.retry_count).max(0)),
        enabled: Set(enabled),
        status: Set(input.status.unwrap_or(current.status)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    // Desabilitar um monitor cala a fonte do alerta: manter o evento aberto
    // deixaria a Central de Alertas apontando para algo que ninguém mais mede.
    if !enabled {
        recovery::resolve_alerts_for_monitor(&ctx, id, "Monitor desativado").await?;
    }
    let mut output = present_monitors(&ctx.db, vec![row], RECENT_RESULTS_LIMIT).await?;
    Ok(format::json(output.remove(0))?)
}

async fn destroy(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    monitors::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Monitor não encontrado"))?;
    // Resolver **antes** de apagar: o cleanup remove os `alert_events` do
    // monitor, e sem esta passagem a notificação de normalização nunca sairia.
    recovery::resolve_alerts_for_monitor(&ctx, id, "Monitor removido").await?;
    ResourceCleanupService::delete_monitor(&ctx.db, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn run(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let monitor = monitors::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Monitor não encontrado"))?;
    if monitor.r#type == "snmp" {
        if let Some(device_id) = monitor.device_id {
            let _guard = try_acquire_snmp_device(device_id).ok_or_else(|| {
                AppError::conflict("Já existe uma coleta SNMP em andamento para este dispositivo")
            })?;
            let device = devices::Entity::find_by_id(device_id)
                .one(&ctx.db)
                .await?
                .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
            let related = monitors::Entity::find()
                .filter(monitors::Column::DeviceId.eq(Some(device_id)))
                .filter(monitors::Column::Type.eq("snmp"))
                .all(&ctx.db)
                .await?;
            let results = snmp_service::poll_device_monitors(&ctx, &device, &related).await;
            let selected = results
                .iter()
                .find(|(monitor_id, _)| *monitor_id == monitor.id)
                .map(|(_, result)| result.clone())
                .ok_or_else(|| AppError::not_found("Monitor SNMP não encontrado no dispositivo"))?;
            for (monitor_id, result) in results {
                process_result(&ctx, monitor_id, &result, None).await?;
            }
            return Ok(format::json(serde_json::json!({
                "message": "Coleta SNMP consolidada do dispositivo concluída com sucesso",
                "result": selected,
            }))?);
        }
    }
    let _guard = try_acquire_monitor(monitor.id).ok_or_else(|| {
        AppError::conflict("Uma verificação para este monitor já está em andamento")
    })?;
    let result = run_monitor(
        &ctx,
        &monitor.r#type,
        &monitor.configuration,
        RunOptions {
            timeout_ms: Some(
                calculate_smart_timeout_seconds(&monitor.r#type, monitor.interval_seconds) as u64
                    * 1000,
            ),
        },
    )
    .await?;
    process_result(&ctx, monitor.id, &result, monitor.probe_id).await?;
    Ok(format::json(
        serde_json::json!({"message": format!("Execução manual do monitor #{} concluída com sucesso", monitor.id), "result": result}),
    )?)
}

async fn toggle(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    enabled: bool,
) -> AppResult<Response> {
    let current = monitors::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Monitor não encontrado"))?;
    let mut active: monitors::ActiveModel = current.clone().into();
    active.enabled = Set(enabled);
    let row = active.update(&ctx.db).await?;
    // Desabilitar cala a fonte do alerta: manter o evento aberto deixaria a
    // Central de Alertas apontando para algo que ninguém mais mede.
    if !enabled {
        recovery::resolve_alerts_for_monitor(&ctx, id, "Monitor desativado").await?;
    }
    if let Some(device_id) = row.device_id {
        if let Some(device) = crate::models::devices::Entity::find_by_id(device_id)
            .one(&ctx.db)
            .await?
        {
            device_status::refresh_from_monitors(&ctx, &device, None, None).await?;
        }
    }
    let mut output = present_monitors(&ctx.db, vec![row], RECENT_RESULTS_LIMIT).await?;
    Ok(format::json(output.remove(0))?)
}

async fn enable(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    toggle(State(ctx), Path(id), true).await
}
async fn disable(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    toggle(State(ctx), Path(id), false).await
}

async fn results(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    monitors::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Monitor não encontrado"))?;
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let data = paginate_compat(
        &ctx.db,
        monitor_results::Entity::find()
            .filter(monitor_results::Column::MonitorId.eq(id))
            .order_by_desc(monitor_results::Column::StartedAt),
        page,
        limit,
        MonitorResultPresentation::from,
    )
    .await?;
    Ok(format::json(data)?)
}

/// Histórico de alertas do monitor (§7.6), sempre paginado.
async fn alerts(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    monitors::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Monitor não encontrado"))?;
    Ok(format::json(
        crate::controllers::alerts::alerts_for_monitor(&ctx, id, &query).await?,
    )?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/monitors")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/run", post(run))
        .add("/{id}/enable", post(enable))
        .add("/{id}/disable", post(disable))
        .add("/{id}/results", get(results))
        .add("/{id}/alerts", get(alerts))
}

#[cfg(test)]
mod tests {
    use super::build_configuration;

    #[test]
    fn configuracao_do_monitor_descarta_timeout_informado() {
        let config = build_configuration(
            "ping",
            Some(serde_json::json!({ "host": "127.0.0.1", "timeoutMs": 60_000 })),
            None,
            None,
            None,
        );

        assert!(config.get("timeoutMs").is_none());
    }
}
