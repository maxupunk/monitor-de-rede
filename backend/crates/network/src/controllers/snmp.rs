//! Endpoints SNMP de teste e coleta de dispositivo, e o cadastro das
//! credenciais SNMP (`/api/snmp/credentials`).

use axum::{http::HeaderMap, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::EntityTrait;

use crate::{
    dtos::snmp_credentials::{SnmpCredentialInput, SnmpCredentialTestInput, SnmpTargetInput},
    snmp::{
        client::SnmpConfig,
        credentials,
        service::{self, DetectCandidate},
    },
};
use netmonitor_core::{
    audit::{AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType},
    models::devices,
    monitoring::execution_guard::{try_acquire_snmp_device, DeviceSnmpExecutionGuard},
    shared::errors::{AppError, AppResult},
};
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnmpApplyInput {
    enable_cpu_monitor: Option<bool>,
    enable_memory_monitor: Option<bool>,
    monitored_if_indexes: Option<Vec<i32>>,
    monitored_sensors: Option<Vec<String>>,
    clear_removed_history: Option<bool>,
}
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct InterfaceMonitoringInput {
    enabled: bool,
}

/// `POST /api/snmp/test` — testa a credencial (perfil, avulsa ou padrão)
/// contra um alvo; com `autoDetect`, tenta também os perfis cadastrados e diz
/// qual respondeu (`profileId`).
async fn test(
    State(ctx): State<AppContext>,
    Json(input): Json<SnmpTargetInput>,
) -> AppResult<Response> {
    let config = credentials::target_config(&ctx.db, &input).await?;
    if input.auto_detect.unwrap_or(false) {
        let preferred = credentials::target_candidate(&ctx.db, &input).await?;
        let candidates = DetectCandidate::load(&ctx.db).await?;
        return Ok(format::json(
            service::detect_connection(&config.host, config.port, Some(preferred), &candidates)
                .await?,
        )?);
    }
    Ok(format::json(service::test_connection(config).await?)?)
}
/// Quanto uma mudança de configuração espera a coleta em curso do mesmo
/// dispositivo terminar antes de desistir com 409: o poll agendado de um
/// switch grande dura alguns segundos, e o operador não deveria ver erro só
/// por ter clicado durante ele.
const CONFIG_GUARD_WAIT: std::time::Duration = std::time::Duration::from_secs(10);
const CONFIG_GUARD_RETRY: std::time::Duration = std::time::Duration::from_millis(100);

async fn wait_snmp_device(id: i64) -> AppResult<DeviceSnmpExecutionGuard> {
    let deadline = tokio::time::Instant::now() + CONFIG_GUARD_WAIT;
    loop {
        if let Some(guard) = try_acquire_snmp_device(id) {
            return Ok(guard);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::conflict(
                "Já existe uma consulta SNMP em andamento para este dispositivo",
            ));
        }
        tokio::time::sleep(CONFIG_GUARD_RETRY).await;
    }
}

async fn device_with_config(ctx: &AppContext, id: i64) -> AppResult<(devices::Model, SnmpConfig)> {
    let device = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let config = service::device_config(&ctx.db, &device).await?;
    Ok((device, config))
}
async fn scan(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let _guard = try_acquire_snmp_device(id).ok_or_else(|| {
        AppError::conflict("Já existe uma consulta SNMP em andamento para este dispositivo")
    })?;
    let (device, config) = device_with_config(&ctx, id).await?;
    Ok(format::json(
        service::scan_device(&ctx, &device, config).await?,
    )?)
}
async fn poll(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let _guard = try_acquire_snmp_device(id).ok_or_else(|| {
        AppError::conflict("Já existe uma consulta SNMP em andamento para este dispositivo")
    })?;
    let (device, device_config) = device_with_config(&ctx, id).await?;
    let result = service::poll_device(&ctx, &device, device_config).await?;
    Ok(format::json(
        serde_json::json!({ "message":"Varredura SNMP executada com sucesso", "result":result }),
    )?)
}
async fn apply_monitors(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<SnmpApplyInput>,
) -> AppResult<Response> {
    let _guard = wait_snmp_device(id).await?;
    let (device, config) = device_with_config(&ctx, id).await?;
    service::apply_monitors(
        &ctx,
        &device,
        config,
        service::SnmpApplyOptions {
            enable_cpu_monitor: input.enable_cpu_monitor,
            enable_memory_monitor: input.enable_memory_monitor,
            monitored_if_indexes: input.monitored_if_indexes.unwrap_or_default(),
            monitored_sensors: input.monitored_sensors.unwrap_or_default(),
            clear_removed_history: input.clear_removed_history,
        },
    )
    .await?;
    Ok(format::json(serde_json::json!({
        "message": "Configurações de monitoramento atualizadas com sucesso",
    }))?)
}
/// Só a lista de interfaces já conhecidas — sem tocar no agente SNMP. Quem
/// descobre porta nova é `scan`/`poll`.
async fn interfaces(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(service::list_interfaces(&ctx, id).await?)?)
}
async fn set_interface_monitoring(
    State(ctx): State<AppContext>,
    Path((id, interface_id)): Path<(i64, i64)>,
    Json(input): Json<InterfaceMonitoringInput>,
) -> AppResult<Response> {
    let _guard = wait_snmp_device(id).await?;
    let (device, config) = device_with_config(&ctx, id).await?;
    service::set_interface_monitoring(&ctx, &device, config, interface_id, input.enabled).await?;
    Ok(format::json(serde_json::json!({
        "message": if input.enabled {
            "Interface adicionada ao monitoramento"
        } else {
            "Interface removida do monitoramento"
        },
    }))?)
}
/// Consulta a lista de interfaces de um alvo SNMP sem precisar que o dispositivo
/// já esteja salvo no banco. Útil para preenchimento de seletores na criação.
async fn query_interfaces(
    State(ctx): State<AppContext>,
    Json(input): Json<SnmpTargetInput>,
) -> AppResult<Response> {
    let cfg: SnmpConfig = credentials::target_config(&ctx.db, &input).await?;
    let ifaces = service::query_interfaces(cfg).await?;
    Ok(format::json(ifaces)?)
}

async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    id: i64,
    label: &str,
    description: String,
    changes: Option<AuditChanges>,
) {
    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action,
                resource_type: ResourceType::SnmpCredential,
                resource_id: Some(id),
                resource_label: Some(label.to_owned()),
                description: Some(description),
                changes,
            },
        )
        .await;
}

async fn list_credentials(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(credentials::list(&ctx.db).await?)?)
}

async fn show_credential(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    Ok(format::json(credentials::show(&ctx.db, id).await?)?)
}

async fn create_credential(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<SnmpCredentialInput>,
) -> AppResult<Response> {
    let created = credentials::create(&ctx.db, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        created.id,
        &created.name,
        format!(
            "Credencial SNMP '{}' ({}) criada",
            created.name, created.version
        ),
        Some(AuditChanges {
            old: None,
            new: serde_json::to_value(&created).ok(),
        }),
    )
    .await;
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

async fn update_credential(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(input): Json<SnmpCredentialInput>,
) -> AppResult<Response> {
    let (before, after) = credentials::update(&ctx.db, id, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        after.id,
        &after.name,
        format!("Credencial SNMP '{}' atualizada", after.name),
        Some(AuditChanges {
            old: serde_json::to_value(before).ok(),
            new: serde_json::to_value(&after).ok(),
        }),
    )
    .await;
    Ok(format::json(after)?)
}

async fn delete_credential(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let removed = credentials::delete(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        removed.id,
        &removed.name,
        format!("Credencial SNMP '{}' excluída", removed.name),
        Some(AuditChanges {
            old: serde_json::to_value(&removed).ok(),
            new: None,
        }),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `POST /api/snmp/credentials/{id}/test` — a credencial gravada contra um
/// alvo, sem o segredo passar pela tela.
async fn test_credential(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<SnmpCredentialTestInput>,
) -> AppResult<Response> {
    Ok(format::json(
        credentials::test_against(&ctx.db, id, &input.host, input.port).await?,
    )?)
}

/// `POST /api/snmp/interfaces/suggestions` — o Decisor indica o uplink e as
/// interfaces que valem monitorar. Sem Decisor, `available: false`.
async fn interface_suggestions(
    State(ctx): State<AppContext>,
    Json(input): Json<crate::dtos::snmp::InterfaceSuggestionsInput>,
) -> AppResult<Response> {
    let suggestions = crate::snmp::decider_interfaces::suggest(&ctx, &input).await;
    Ok(format::json(suggestions)?)
}

async fn list_profiles(State(ctx): State<AppContext>) -> AppResult<Response> {
    let profiles = crate::snmp::profiles::load_all_profiles(&ctx.db).await?;
    Ok(format::json(profiles)?)
}

async fn save_profile(
    State(ctx): State<AppContext>,
    Json(profile): Json<crate::snmp::profiles::SnmpDeviceProfile>,
) -> AppResult<Response> {
    let saved = crate::snmp::profiles::save_custom_profile(&ctx.db, profile).await?;
    Ok(format::json(saved)?)
}

async fn delete_profile(
    State(ctx): State<AppContext>,
    Path(profile_id): Path<String>,
) -> AppResult<Response> {
    crate::snmp::profiles::delete_custom_profile(&ctx.db, &profile_id).await?;
    Ok(format::json(serde_json::json!({
        "message": "Perfil removido com sucesso",
    }))?)
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/snmp/test", post(test))
        .add("/snmp/interfaces-query", post(query_interfaces))
        .add("/snmp/interfaces/suggestions", post(interface_suggestions))
        .add("/snmp/profiles", get(list_profiles))
        .add("/snmp/profiles", post(save_profile))
        .add("/snmp/profiles/{id}", delete(delete_profile))
        .add("/snmp/credentials", get(list_credentials))
        .add("/snmp/credentials", post(create_credential))
        .add("/snmp/credentials/{id}", get(show_credential))
        .add("/snmp/credentials/{id}", put(update_credential))
        .add("/snmp/credentials/{id}", delete(delete_credential))
        .add("/snmp/credentials/{id}/test", post(test_credential))
        .add("/devices/{id}/snmp/scan", post(scan))
        .add("/devices/{id}/snmp/poll", post(poll))
        .add("/devices/{id}/snmp/apply-monitors", post(apply_monitors))
        .add("/devices/{id}/interfaces", get(interfaces))
        .add(
            "/devices/{id}/interfaces/{interface_id}/monitoring",
            patch(set_interface_monitoring),
        )
}
