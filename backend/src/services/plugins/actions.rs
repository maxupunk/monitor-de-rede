//! Os casos de uso que a tela e a IA disparam — o controller só delega.

use loco_rs::app::AppContext;
use sea_orm::EntityTrait;
use serde_json::Value;

use super::{
    auto_accept,
    compat::DeviceFacts,
    credentials,
    effect::Effect,
    runs::{self, Origin, RunSpec, VALIDATION_ACTION},
    service,
};
use crate::{
    dtos::plugins::{AgentRouteOption, AutoAcceptState, DevicePluginsView},
    models::devices,
    services::{
        agents::{hub::AgentHub, policy::Permission, service::AgentService},
        shared::errors::{AppError, AppResult},
    },
};

/// Quantas execuções a aba mostra.
const RECENT_RUNS: u64 = 20;

/// # Errors
///
/// Dispositivo inexistente.
pub async fn device(ctx: &AppContext, device_id: i64) -> AppResult<devices::Model> {
    devices::Entity::find_by_id(device_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado."))
}

/// Tudo que a aba "Plugins" precisa, numa resposta.
///
/// # Errors
///
/// Dispositivo inexistente ou erro do banco.
pub async fn device_view(ctx: &AppContext, device_id: i64) -> AppResult<DevicePluginsView> {
    let device = device(ctx, device_id).await?;
    let facts = DeviceFacts::from_device(&device);
    let hub = AgentHub::from_context(ctx).ok();
    let agents = AgentService::new(&ctx.db)
        .list()
        .await?
        .into_iter()
        .map(|agent| {
            let session = hub.as_ref().and_then(|hub| hub.get(agent.id));
            AgentRouteOption {
                id: agent.id,
                name: agent.name,
                connected: session.is_some(),
                allows_device_io: session
                    .is_some_and(|session| session.allows(Permission::DeviceIo)),
            }
        })
        .collect();
    Ok(DevicePluginsView {
        device_id,
        platform: facts.platform,
        firmware: facts.firmware,
        plugins: service::for_device(&ctx.db, &device).await?,
        credentials: credentials::list(&ctx.db, device_id).await?,
        runs: runs::recent(&ctx.db, device_id, RECENT_RUNS).await?,
        agents,
    })
}

/// O operador dispara uma ação na aba. Roda em segundo plano.
///
/// # Errors
///
/// Escrita sem confirmação explícita, ou o que [`runs::prepare`] recusar.
pub async fn start_from_user(
    ctx: &AppContext,
    device_id: i64,
    plugin_id: i64,
    action: &str,
    params: Value,
    confirm_write: bool,
    user_id: i64,
) -> AppResult<i64> {
    let device = device(ctx, device_id).await?;
    let plugin = service::find(&ctx.db, plugin_id).await?;
    if !service::is_installed(&ctx.db, device_id, plugin_id).await? {
        return Err(AppError::business_rule(
            "Instale o plugin neste equipamento antes de executar as ações dele.",
        ));
    }
    let package = service::package_of(&plugin)?;
    let declared = package
        .manifest
        .action(action)
        .ok_or_else(|| AppError::not_found(format!("O plugin não tem a ação `{action}`.")))?;
    if declared.effect == Effect::Write && !confirm_write {
        return Err(AppError::business_rule(
            "Esta ação altera o equipamento e precisa de confirmação explícita.",
        ));
    }
    let approval = runs::approval_for(&plugin, Origin::User, false);
    let prepared = runs::prepare(
        ctx,
        RunSpec {
            plugin,
            device,
            action: action.to_owned(),
            params,
            origin: Origin::User,
            user_id: Some(user_id),
            reason: None,
            approval,
        },
    )
    .await?;
    Ok(runs::spawn(ctx.clone(), prepared))
}

/// Roda a suíte funcional no equipamento. Em segundo plano.
///
/// # Errors
///
/// O que [`runs::prepare`] recusar.
pub async fn start_validation(
    ctx: &AppContext,
    device_id: i64,
    plugin_id: i64,
    user_id: i64,
) -> AppResult<i64> {
    let device = device(ctx, device_id).await?;
    let plugin = service::find(&ctx.db, plugin_id).await?;
    let approval = runs::approval_for(&plugin, Origin::Validation, false);
    let prepared = runs::prepare(
        ctx,
        RunSpec {
            plugin,
            device,
            action: VALIDATION_ACTION.to_owned(),
            params: Value::Null,
            origin: Origin::Validation,
            user_id: Some(user_id),
            reason: None,
            approval,
        },
    )
    .await?;
    Ok(runs::spawn(ctx.clone(), prepared))
}

/// O estado do modo automático para a tela.
///
/// # Errors
///
/// Erro do banco.
pub async fn auto_accept_state(
    ctx: &AppContext,
    conversation_key: &str,
    device_id: i64,
) -> AppResult<AutoAcceptState> {
    let active = auto_accept::active(&ctx.db, conversation_key, device_id).await?;
    Ok(AutoAcceptState {
        active: active.is_some(),
        expires_at: active.map(|row| row.expires_at.to_rfc3339()),
        terms_version: auto_accept::TERMS_VERSION.to_owned(),
        terms: auto_accept::TERMS.to_owned(),
    })
}
