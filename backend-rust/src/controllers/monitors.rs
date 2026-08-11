//! CRUD e acionamento manual dos monitores.

use axum::{extract::Query, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::{
    dtos::resources::{MonitorInput, PaginationQuery},
    models::{monitor_results, monitors},
    services::{
        alerts::recovery,
        maintenance::resource_cleanup::ResourceCleanupService,
        monitoring::{
            device_status,
            presenter::{present_monitors, MonitorResultPresentation, RECENT_RESULTS_LIMIT},
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        shared::{
            errors::{AppError, AppResult},
            pagination::paginate_compat,
        },
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

fn require_kind_name(input: &MonitorInput) -> AppResult<(&str, &str)> {
    let kind = input
        .monitor_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Tipo do monitor é obrigatório"))?;
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Nome do monitor é obrigatório"))?;
    Ok((kind, name))
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
        interval_seconds: Set(input.interval_seconds.unwrap_or(15).max(1)),
        timeout_seconds: Set(input.timeout_seconds.unwrap_or(10).max(1)),
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
    let kind = input
        .monitor_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.r#type)
        .to_string();
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
    let row = monitors::ActiveModel {
        id: Set(id),
        device_id: Set(input.device_id.or(current.device_id)),
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
        interval_seconds: Set(input
            .interval_seconds
            .unwrap_or(current.interval_seconds)
            .max(1)),
        timeout_seconds: Set(input
            .timeout_seconds
            .unwrap_or(current.timeout_seconds)
            .max(1)),
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
    let result = run_monitor(
        &ctx,
        &monitor.r#type,
        &monitor.configuration,
        RunOptions {
            timeout_ms: Some(monitor.timeout_seconds.max(1) as u64 * 1000),
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
