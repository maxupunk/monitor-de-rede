//! Plugins de dispositivo: biblioteca, aba do equipamento, credenciais,
//! execuções, aprovações e o modo automático da IA.
//!
//! Extrai, valida, delega (`services::plugins`) e serializa. As rotas de
//! escrita são só de administrador (`users::request_is_allowed`).

use axum::{
    extract::Query,
    http::{header, HeaderMap, StatusCode},
};
use loco_rs::prelude::*;

use crate::{
    controllers::auth_guard::authenticated_user,
    dtos::{
        optional_body,
        plugins::{
            ApprovalInput, AutoAcceptInput, AutoAcceptQuery, BatchStarted, FleetIdentifyInput,
            FleetRunInput, PluginSaveInput, ReviewAcceptInput, RunActionInput, RunStarted,
            SessionSecretInput, SettingsInput,
        },
    },
    models::plugins as plugin_rows,
    services::{
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        plugins::{
            actions, auto_accept,
            credentials::{self, CredentialInput},
            fleet, gate,
            manifest::TransportKind,
            runs,
            service::{self, source},
            settings::{self as plugin_settings, Scope},
        },
        shared::errors::{AppError, AppResult},
    },
};

async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    resource_type: ResourceType,
    resource: (i64, &str),
    description: String,
) {
    let actor = AuditActor::from_headers(headers, &ctx.db)
        .await
        .unwrap_or_default();
    let _ = AuditService::new(&ctx.db)
        .log(
            actor,
            AuditEntryInput {
                action,
                resource_type,
                resource_id: Some(resource.0),
                resource_label: Some(resource.1.to_owned()),
                description: Some(description),
                changes: None,
            },
        )
        .await;
}

async fn audit_plugin(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    plugin: &plugin_rows::Model,
    description: &str,
) {
    audit(
        ctx,
        headers,
        action,
        ResourceType::Plugin,
        (plugin.id, &plugin.name),
        format!("{description} (v{})", plugin.version),
    )
    .await;
}

fn kind(value: &str) -> AppResult<TransportKind> {
    TransportKind::parse(value).map_err(AppError::validation)
}

// --- Biblioteca ------------------------------------------------------------

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(service::list(&ctx.db).await?)?)
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(service::detail(&ctx.db, id).await?)?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<PluginSaveInput>,
) -> AppResult<Response> {
    let created = service::create(&ctx.db, &input.package, input.device_id, source::USER).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Create,
        &created,
        "Criou o plugin",
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(service::detail(&ctx.db, created.id).await?),
    )
        .into_response())
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<PluginSaveInput>,
) -> AppResult<Response> {
    let updated = service::update(&ctx.db, id, &input.package).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &updated,
        "Editou o plugin",
    )
    .await;
    Ok(format::json(service::detail(&ctx.db, id).await?)?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let deleted = service::delete(&ctx.db, id).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Delete,
        &deleted,
        "Excluiu o plugin",
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn import(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<PluginSaveInput>,
) -> AppResult<Response> {
    let imported = service::import(&ctx.db, &input.package, input.device_id).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Create,
        &imported,
        "Importou o plugin (em quarentena)",
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(service::detail(&ctx.db, imported.id).await?),
    )
        .into_response())
}

async fn export(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let model = service::find(&ctx.db, id).await?;
    let package = service::package_of(&model)?;
    let body = serde_json::to_string_pretty(&package)
        .map_err(|error| AppError::Internal(anyhow::anyhow!("{error}")))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    r#"attachment; filename="{}-{}.nmplugin.json""#,
                    model.slug, model.version
                ),
            ),
        ],
        body,
    )
        .into_response())
}

async fn review(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    service::review_again(&ctx.db, id).await?;
    Ok(format::json(service::detail(&ctx.db, id).await?)?)
}

async fn accept_review(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    body: String,
) -> AppResult<Response> {
    let input: ReviewAcceptInput = optional_body(&body);
    let accepted = service::accept_review(&ctx.db, id, input.acknowledge_risk).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &accepted,
        "Aceitou a revisão de segurança e tirou o plugin da quarentena",
    )
    .await;
    Ok(format::json(service::detail(&ctx.db, id).await?)?)
}

async fn test(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let (_, report) = service::run_tests(&ctx.db, id).await?;
    Ok(format::json(report)?)
}

async fn promote(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let promoted = service::promote(&ctx.db, id).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &promoted,
        "Ativou o plugin",
    )
    .await;
    Ok(format::json(service::detail(&ctx.db, id).await?)?)
}

async fn enable(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    service::set_enabled(&ctx.db, id, true).await?;
    Ok(format::json(service::detail(&ctx.db, id).await?)?)
}

async fn disable(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let disabled = service::set_enabled(&ctx.db, id, false).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &disabled,
        "Desativou o plugin",
    )
    .await;
    Ok(format::json(service::detail(&ctx.db, id).await?)?)
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DuplicateInput {
    #[serde(default)]
    device_id: Option<i64>,
}

async fn duplicate(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    body: String,
) -> AppResult<Response> {
    let input: DuplicateInput = optional_body(&body);
    let copy = service::duplicate(&ctx.db, id, input.device_id).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Create,
        &copy,
        "Duplicou o plugin",
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(service::detail(&ctx.db, copy.id).await?),
    )
        .into_response())
}

// --- Aba do equipamento -----------------------------------------------------

async fn device_plugins(
    State(ctx): State<AppContext>,
    Path(device_id): Path<i64>,
) -> AppResult<Response> {
    Ok(format::json(actions::device_view(&ctx, device_id).await?)?)
}

async fn run_action(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((device_id, plugin_id, action)): Path<(i64, i64, String)>,
    body: String,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let input: RunActionInput = optional_body(&body);
    let run_id = actions::start_from_user(
        &ctx,
        device_id,
        plugin_id,
        &action,
        input.params,
        input.confirm_write,
        user.id,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(RunStarted { run_id })).into_response())
}

async fn validate(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((device_id, plugin_id)): Path<(i64, i64)>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let run_id = actions::start_validation(&ctx, device_id, plugin_id, user.id).await?;
    Ok((StatusCode::ACCEPTED, Json(RunStarted { run_id })).into_response())
}

async fn install(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((device_id, plugin_id)): Path<(i64, i64)>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let device = actions::device(&ctx, device_id).await?;
    let plugin = service::install(&ctx.db, &device, plugin_id, Some(user.id)).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &plugin,
        &format!("Instalou o plugin em {}", device.name),
    )
    .await;
    Ok(format::json(actions::device_view(&ctx, device_id).await?)?)
}

async fn uninstall(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((device_id, plugin_id)): Path<(i64, i64)>,
) -> AppResult<Response> {
    let device = actions::device(&ctx, device_id).await?;
    let plugin = service::uninstall(&ctx.db, device_id, plugin_id).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &plugin,
        &format!("Desinstalou o plugin de {}", device.name),
    )
    .await;
    Ok(format::json(actions::device_view(&ctx, device_id).await?)?)
}

// --- Frota (Aplicativos) e configuração guardada ----------------------------

async fn apps(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(fleet::apps(&ctx.db).await?)?)
}

async fn fleet_view(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(fleet::view(&ctx.db, id).await?)?)
}

async fn fleet_settings(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<SettingsInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let plugin = service::find(&ctx.db, id).await?;
    let saved =
        plugin_settings::save(&ctx.db, &plugin, Scope::Fleet, input.value, Some(user.id)).await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &plugin,
        "Alterou a configuração da frota",
    )
    .await;
    Ok(format::json(saved)?)
}

async fn device_settings(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((device_id, plugin_id)): Path<(i64, i64)>,
    Json(input): Json<SettingsInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let device = actions::device(&ctx, device_id).await?;
    let plugin = service::find(&ctx.db, plugin_id).await?;
    if !service::is_installed(&ctx.db, device_id, plugin_id).await? {
        return Err(AppError::business_rule(
            "Instale o plugin neste equipamento antes de configurá-lo.",
        ));
    }
    let saved = plugin_settings::save(
        &ctx.db,
        &plugin,
        Scope::Device(device_id),
        input.value,
        Some(user.id),
    )
    .await?;
    audit_plugin(
        &ctx,
        &headers,
        AuditAction::Update,
        &plugin,
        &format!("Alterou a configuração do plugin em {}", device.name),
    )
    .await;
    Ok(format::json(saved)?)
}

async fn fleet_run(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((plugin_id, action)): Path<(i64, String)>,
    body: String,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let input: FleetRunInput = optional_body(&body);
    let batch_id = fleet::start(
        &ctx,
        plugin_id,
        fleet::FleetRun {
            action,
            device_ids: input.device_ids,
            params: input.params,
            device_params: input.device_params,
            confirm_write: input.confirm_write,
            user_id: Some(user.id),
        },
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(BatchStarted { batch_id })).into_response())
}

/// "Verificar sistema": vai aos equipamentos (SSH/SNMP, Laya na dúvida) e
/// devolve a página do aplicativo com a compatibilidade refeita.
async fn fleet_identify(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(plugin_id): Path<i64>,
    body: String,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let input: FleetIdentifyInput = optional_body(&body);
    let view = crate::services::plugins::identity::refresh_fleet(
        &ctx,
        plugin_id,
        &input.device_ids,
        Some(user.id),
    )
    .await?;
    Ok(format::json(view)?)
}

async fn batch_show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(fleet::find_view(&ctx.db, id).await?)?)
}

async fn batch_cancel(Path(id): Path<i64>) -> AppResult<Response> {
    if fleet::cancel(id) {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(AppError::not_found("O lote já terminou."))
    }
}

async fn batch_apply_patch(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let view = fleet::apply_patch(&ctx.db, id, Some(user.id)).await?;
    Ok(format::json(view)?)
}

// --- Credenciais ------------------------------------------------------------

async fn credentials_index(
    State(ctx): State<AppContext>,
    Path(device_id): Path<i64>,
) -> AppResult<Response> {
    actions::device(&ctx, device_id).await?;
    Ok(format::json(credentials::list(&ctx.db, device_id).await?)?)
}

async fn credentials_upsert(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(device_id): Path<i64>,
    Json(input): Json<CredentialInput>,
) -> AppResult<Response> {
    let device = actions::device(&ctx, device_id).await?;
    let saved = credentials::upsert(&ctx.db, device_id, input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        ResourceType::Device,
        (device.id, &device.name),
        format!(
            "Gravou a credencial {} de acesso ({})",
            saved.kind.as_str().to_uppercase(),
            saved.storage.as_str()
        ),
    )
    .await;
    Ok(format::json(saved)?)
}

async fn credentials_delete(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((device_id, credential_kind)): Path<(i64, String)>,
) -> AppResult<Response> {
    let device = actions::device(&ctx, device_id).await?;
    let credential_kind = kind(&credential_kind)?;
    credentials::delete(&ctx.db, device_id, credential_kind).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::Device,
        (device.id, &device.name),
        format!(
            "Removeu a credencial {} de acesso",
            credential_kind.as_str().to_uppercase()
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn credentials_session(
    State(ctx): State<AppContext>,
    Path((device_id, credential_kind)): Path<(i64, String)>,
    Json(input): Json<SessionSecretInput>,
) -> AppResult<Response> {
    let view =
        credentials::open_session(&ctx.db, device_id, kind(&credential_kind)?, &input.secret)
            .await?;
    Ok(format::json(view)?)
}

// --- Execuções e aprovações -------------------------------------------------

async fn run_show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(runs::find_view(&ctx.db, id).await?)?)
}

async fn run_cancel(Path(id): Path<i64>) -> AppResult<Response> {
    if runs::cancel(id) {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(AppError::not_found("A execução já terminou."))
    }
}

async fn approval(
    Path(key): Path<String>,
    Json(input): Json<ApprovalInput>,
) -> AppResult<Response> {
    if gate::resolve(&key, input.approved) {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(AppError::not_found(
            "Este pedido de aprovação não está mais pendente.",
        ))
    }
}

// --- Modo automático da IA --------------------------------------------------

async fn auto_accept_show(
    State(ctx): State<AppContext>,
    Query(query): Query<AutoAcceptQuery>,
) -> AppResult<Response> {
    Ok(format::json(
        actions::auto_accept_state(&ctx, &query.conversation_key, query.device_id).await?,
    )?)
}

async fn auto_accept_store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<AutoAcceptInput>,
) -> AppResult<Response> {
    let user = authenticated_user(&ctx, &headers).await?;
    let device = actions::device(&ctx, input.device_id).await?;
    auto_accept::accept(
        &ctx.db,
        &input.conversation_key,
        input.device_id,
        Some(user.id),
        input.accept_terms,
    )
    .await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        ResourceType::Device,
        (device.id, &device.name),
        format!(
            "Ligou o modo \"Aceitar automaticamente\" da IA (termo v{}) — a IA executa sem pedir confirmação",
            auto_accept::TERMS_VERSION
        ),
    )
    .await;
    Ok(format::json(
        actions::auto_accept_state(&ctx, &input.conversation_key, input.device_id).await?,
    )?)
}

async fn auto_accept_destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<AutoAcceptQuery>,
) -> AppResult<Response> {
    let device = actions::device(&ctx, input.device_id).await?;
    auto_accept::revoke(&ctx.db, &input.conversation_key, input.device_id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        ResourceType::Device,
        (device.id, &device.name),
        "Desligou o modo \"Aceitar automaticamente\" da IA".to_owned(),
    )
    .await;
    Ok(format::json(
        actions::auto_accept_state(&ctx, &input.conversation_key, input.device_id).await?,
    )?)
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/plugins", get(index).post(store))
        .add("/plugins/apps", get(apps))
        .add("/plugins/{id}/fleet", get(fleet_view))
        .add("/plugins/{id}/settings", put(fleet_settings))
        .add("/plugins/{id}/fleet/actions/{action}", post(fleet_run))
        .add("/plugins/{id}/fleet/identify", post(fleet_identify))
        .add(
            "/devices/{id}/plugins/{plugin_id}/settings",
            put(device_settings),
        )
        .add("/plugin-batches/{id}", get(batch_show))
        .add("/plugin-batches/{id}/cancel", post(batch_cancel))
        .add("/plugin-batches/{id}/apply-patch", post(batch_apply_patch))
        .add("/plugins/import", post(import))
        .add("/plugins/{id}", get(show).put(update).delete(destroy))
        .add("/plugins/{id}/export", get(export))
        .add("/plugins/{id}/review", post(review))
        .add("/plugins/{id}/accept-review", post(accept_review))
        .add("/plugins/{id}/test", post(test))
        .add("/plugins/{id}/promote", post(promote))
        .add("/plugins/{id}/enable", post(enable))
        .add("/plugins/{id}/disable", post(disable))
        .add("/plugins/{id}/duplicate", post(duplicate))
        .add("/devices/{id}/plugins", get(device_plugins))
        .add(
            "/devices/{id}/plugins/{plugin_id}/actions/{action}",
            post(run_action),
        )
        .add("/devices/{id}/plugins/{plugin_id}/validate", post(validate))
        .add(
            "/devices/{id}/plugins/{plugin_id}/install",
            post(install).delete(uninstall),
        )
        .add(
            "/devices/{id}/credentials",
            get(credentials_index).put(credentials_upsert),
        )
        .add(
            "/devices/{id}/credentials/{kind}",
            delete(credentials_delete),
        )
        .add(
            "/devices/{id}/credentials/{kind}/session",
            post(credentials_session),
        )
        .add("/plugin-runs/{id}", get(run_show))
        .add("/plugin-runs/{id}/cancel", post(run_cancel))
        .add("/plugin-approvals/{key}", post(approval))
        .add(
            "/plugin-auto-accept",
            get(auto_accept_show)
                .post(auto_accept_store)
                .delete(auto_accept_destroy),
        )
}
