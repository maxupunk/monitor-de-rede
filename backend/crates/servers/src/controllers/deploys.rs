//! Deploy contínuo (ADR 021): `/api/deploys` e o webhook público.
//!
//! Leitura é de quem tem acesso à área Servidores; mudança, aprovação e o
//! conteúdo do `.env` são do administrador da área
//! (`users::request_is_allowed`). O andamento chega pelo SSE (`deploys:*`).
//!
//! O webhook (`/api/deploy-hooks/github`) fica **fora** do guarda JWT: quem
//! chama é o GitHub Actions, e a prova é o token OIDC no `Authorization`
//! ([`crate::deploys::oidc`]).

use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;
use serde::Deserialize;

use netmonitor_core::{
    audit::{authenticated_user, AuditAction, ResourceType},
    shared::errors::{AppError, AppResult},
};

use crate::{
    databases::backups::{self, Trigger},
    deploys::{
        access, applications, catalog, credentials, env_versions, events,
        github::{self, app::normalize_origin, app::Scope},
        oidc, orchestrator, readiness, runs, short_sha, webhook, workflow,
    },
    dtos::deploys::{
        ApplicationInput, ApproveInput, CentralUrlQuery, CredentialInput, DeployInput, DeployWhen,
        EnvPreviewInput, EnvSaveInput, HookInput, RejectInput, SetupInput, TargetInput,
    },
    views::deploys::{
        DeployBackupView, DeployCredentialView, DeployEnvContentView, DeployEnvSaved,
        DeployEnvVersionView, DeployRunLogView, DeployWorkflowView,
    },
};

/// Quantos deploys o histórico de um alvo mostra.
const HISTORY_LIMIT: u64 = 50;

pub(crate) async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    resource: (i64, &str),
    description: String,
) {
    netmonitor_core::audit::record(
        &ctx.db,
        headers,
        action,
        ResourceType::Deploy,
        resource,
        description,
    )
    .await;
}

/// O nome de quem pediu, para o histórico do deploy.
async fn author(ctx: &AppContext, headers: &HeaderMap) -> AppResult<String> {
    Ok(authenticated_user(ctx, headers).await?.name)
}

async fn overview(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(catalog::overview(&ctx).await?)?)
}

async fn agents(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(catalog::agents(&ctx).await?)?)
}

// ── Credenciais ──────────────────────────────────────────────────────────

async fn store_credential(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<CredentialInput>,
) -> AppResult<Response> {
    let row = credentials::create(&ctx.db, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (row.id, &row.name),
        format!("Credencial de deploy '{}' cadastrada", row.name),
    )
    .await;
    events::publish_updated(&ctx);
    Ok((StatusCode::CREATED, Json(DeployCredentialView::from(&row))).into_response())
}

async fn update_credential(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<CredentialInput>,
) -> AppResult<Response> {
    let row = credentials::update(&ctx.db, id, &input).await?;
    let token = if input
        .secret
        .as_deref()
        .is_some_and(|secret| !secret.trim().is_empty())
    {
        " (token trocado)"
    } else {
        ""
    };
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (row.id, &row.name),
        format!("Credencial de deploy '{}' atualizada{token}", row.name),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(format::json(DeployCredentialView::from(&row))?)
}

async fn destroy_credential(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let row = credentials::delete(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (row.id, &row.name),
        format!("Credencial de deploy '{}' removida", row.name),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ── Aplicações e alvos ───────────────────────────────────────────────────

async fn setup(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<SetupInput>,
) -> AppResult<Response> {
    let user = author(&ctx, &headers).await?;
    let (application, target) = applications::setup(&ctx.db, &input, Some(user)).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (
            target.id,
            &format!("{} / {}", application.name, target.name),
        ),
        format!(
            "Deploy automático configurado: {} ({}) no projeto '{}'",
            application.name, application.repository, target.project
        ),
    )
    .await;
    events::publish_updated(&ctx);
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "applicationId": application.id,
            "targetId": target.id,
        })),
    )
        .into_response())
}

async fn store_application(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ApplicationInput>,
) -> AppResult<Response> {
    let row = applications::create(&ctx.db, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (row.id, &row.name),
        format!("Aplicação '{}' ({}) cadastrada", row.name, row.repository),
    )
    .await;
    events::publish_updated(&ctx);
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": row.id })),
    )
        .into_response())
}

async fn update_application(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ApplicationInput>,
) -> AppResult<Response> {
    let row = applications::update(&ctx.db, id, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (row.id, &row.name),
        format!("Aplicação '{}' atualizada", row.name),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(format::json(serde_json::json!({ "id": row.id }))?)
}

async fn destroy_application(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let row = applications::delete(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (row.id, &row.name),
        format!(
            "Aplicação '{}' removida (os containers continuam no ar)",
            row.name
        ),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn workflow(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<CentralUrlQuery>,
) -> AppResult<Response> {
    let application = applications::find(&ctx.db, id).await?;
    let central = central_url(&query)?;
    Ok(format::json(DeployWorkflowView {
        path: workflow::WORKFLOW_PATH.to_string(),
        yaml: applications::workflow_yaml(&ctx.db, &application, central.as_deref()).await?,
        url_variable: workflow::URL_VARIABLE.to_string(),
        hook_path: workflow::HOOK_PATH.to_string(),
        audience: oidc::audience(),
    })?)
}

/// O endereço público da central, normalizado (vazio = sem endereço).
pub(crate) fn central_url(query: &CentralUrlQuery) -> AppResult<Option<String>> {
    query
        .central_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_origin)
        .transpose()
}

async fn store_target(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<TargetInput>,
) -> AppResult<Response> {
    let row = applications::create_target(&ctx.db, id, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (row.id, &row.name),
        format!(
            "Alvo de deploy '{}' (projeto '{}') criado",
            row.name, row.project
        ),
    )
    .await;
    events::publish_updated(&ctx);
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": row.id })),
    )
        .into_response())
}

async fn update_target(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<TargetInput>,
) -> AppResult<Response> {
    let row = applications::update_target(&ctx.db, id, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (row.id, &row.name),
        format!("Alvo de deploy '{}' atualizado", row.name),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(format::json(serde_json::json!({ "id": row.id }))?)
}

async fn destroy_target(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let row = applications::delete_target(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (row.id, &row.name),
        format!(
            "Alvo de deploy '{}' removido (os containers continuam no ar)",
            row.name
        ),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadinessQuery {
    probe_id: i64,
    project: String,
}

async fn readiness_for(
    State(ctx): State<AppContext>,
    Query(query): Query<ReadinessQuery>,
) -> AppResult<Response> {
    Ok(format::json(
        readiness::inspect(&ctx, query.probe_id, query.project.trim(), None, None).await?,
    )?)
}

async fn target_readiness(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let target = applications::find_target(&ctx.db, id).await?;
    let application = applications::find(&ctx.db, target.application_id).await?;
    let probe_id = target
        .probe_id
        .ok_or_else(|| AppError::validation("O agente deste alvo foi removido: escolha outro"))?;
    Ok(format::json(
        readiness::inspect(
            &ctx,
            probe_id,
            &target.project,
            Some(&application),
            Some(id),
        )
        .await?,
    )?)
}

async fn target_history(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(
        catalog::history(&ctx, id, HISTORY_LIMIT).await?,
    )?)
}

async fn deploy_target(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<DeployInput>,
) -> AppResult<Response> {
    let user = author(&ctx, &headers).await?;
    let run = orchestrator::request_manual(&ctx, id, &input, &user).await?;
    let target = applications::find_target(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (target.id, &target.name),
        format!(
            "Deploy #{} da versão {} pedido em '{}'{}",
            run.id,
            short_sha(&run.commit_sha),
            target.name,
            when_note(input.when)
        ),
    )
    .await;
    Ok((
        StatusCode::ACCEPTED,
        Json(events::view(&ctx.db, &run).await?),
    )
        .into_response())
}

fn when_note(when: DeployWhen) -> &'static str {
    match when {
        DeployWhen::Now => " (agora)",
        DeployWhen::Window => " (na janela de manutenção)",
    }
}

// ── .env ─────────────────────────────────────────────────────────────────

async fn env_versions_list(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    applications::find_target(&ctx.db, id).await?;
    let versions = env_versions::list(&ctx.db, id).await?;
    Ok(format::json(
        versions
            .iter()
            .enumerate()
            .map(|(index, version)| DeployEnvVersionView::new(version, index == 0))
            .collect::<Vec<_>>(),
    )?)
}

async fn env_current(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let target = applications::find_target(&ctx.db, id).await?;
    let current = env_versions::current(&ctx.db, id).await?;
    let content = match &current {
        Some(version) => env_versions::reveal(version)?,
        None => String::new(),
    };
    if current.is_some() {
        audit(
            &ctx,
            &headers,
            AuditAction::Execute,
            (target.id, &target.name),
            format!("Variáveis (.env) do alvo '{}' abertas", target.name),
        )
        .await;
    }
    Ok(format::json(DeployEnvContentView {
        id: current.map(|version| version.id),
        content,
    })?)
}

async fn env_version_content(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let version = env_versions::find(&ctx.db, id).await?;
    let content = env_versions::reveal(&version)?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (version.target_id, "env"),
        format!("Versão #{id} do .env aberta"),
    )
    .await;
    Ok(format::json(DeployEnvContentView {
        id: Some(version.id),
        content,
    })?)
}

async fn env_preview(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<EnvPreviewInput>,
) -> AppResult<Response> {
    applications::find_target(&ctx.db, id).await?;
    Ok(format::json(
        env_versions::preview(&ctx.db, id, &input.content).await?,
    )?)
}

async fn env_save(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<EnvSaveInput>,
) -> AppResult<Response> {
    let target = applications::find_target(&ctx.db, id).await?;
    let user = author(&ctx, &headers).await?;
    let diff = env_versions::preview(&ctx.db, id, &input.content).await?;
    let note = input.note.clone().or_else(|| Some(diff.summary()));
    let version = env_versions::save(&ctx.db, id, &input.content, note, Some(user.clone())).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (target.id, &target.name),
        format!(
            "Variáveis (.env) do alvo '{}' salvas: {} — {}",
            target.name,
            diff.summary(),
            [
                diff.added.join(", "),
                diff.changed.join(", "),
                diff.removed.join(", ")
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("; ")
        ),
    )
    .await;
    let run = if input.redeploy {
        let run = orchestrator::request_manual(
            &ctx,
            id,
            &DeployInput {
                commit_sha: None,
                image_tag: None,
                when: DeployWhen::Now,
            },
            &user,
        )
        .await?;
        Some(events::view(&ctx.db, &run).await?)
    } else {
        None
    };
    events::publish_updated(&ctx);
    Ok(format::json(DeployEnvSaved {
        version: DeployEnvVersionView::new(&version, true),
        diff,
        run,
    })?)
}

// ── Deploys ──────────────────────────────────────────────────────────────

async fn run_show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let run = runs::find(&ctx.db, id).await?;
    Ok(format::json(events::view(&ctx.db, &run).await?)?)
}

async fn run_log(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let run = runs::find(&ctx.db, id).await?;
    Ok(format::json(DeployRunLogView {
        id: run.id,
        log: run.log.unwrap_or_default(),
    })?)
}

async fn run_changes(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let run = runs::find(&ctx.db, id).await?;
    let application = applications::find(&ctx.db, run.application_id).await?;
    let base = runs::live(&ctx.db, run.target_id)
        .await?
        .filter(|live| live.id != run.id)
        .map(|live| live.commit_sha);
    // Sem acesso (app desinstalado, token revogado) a tela ainda mostra o
    // link do "Compare": os commits são informação, não condição.
    let client = access::repository_client(
        &ctx.db,
        access::Source::of(&application),
        &application.repository,
        Scope::Read,
    )
    .await
    .ok();
    let token = client
        .as_ref()
        .and_then(|client| client.token().map(ToString::to_string));
    Ok(format::json(
        github::changes(
            &application.repository,
            base.as_deref(),
            &run.commit_sha,
            token.as_deref(),
        )
        .await,
    )?)
}

async fn run_backups(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let run = runs::find(&ctx.db, id).await?;
    let target = applications::find_target(&ctx.db, run.target_id).await?;
    let (Some(connection), Some(started)) = (target.backup_connection_id, run.started_at) else {
        return Ok(format::json(Vec::<DeployBackupView>::new())?);
    };
    let copies = backups::copies_since(
        &ctx.db,
        connection,
        Trigger::PreDeploy,
        started.with_timezone(&chrono::Utc),
        run.finished_at.map(|at| at.with_timezone(&chrono::Utc)),
    )
    .await?;
    Ok(format::json(
        copies
            .into_iter()
            .map(|row| DeployBackupView {
                id: row.id,
                connection_id: row.connection_id,
                database_name: row.database_name,
                status: row.status,
                started_at: row.started_at.to_rfc3339(),
            })
            .collect::<Vec<_>>(),
    )?)
}

async fn run_label(ctx: &AppContext, run_id: i64) -> String {
    match runs::find(&ctx.db, run_id).await {
        Ok(run) => match events::view(&ctx.db, &run).await {
            Ok(view) => format!(
                "#{} {} / {} ({})",
                run.id,
                view.application_name,
                view.target_name,
                short_sha(&run.commit_sha)
            ),
            Err(_) => format!("#{run_id}"),
        },
        Err(_) => format!("#{run_id}"),
    }
}

async fn approve(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ApproveInput>,
) -> AppResult<Response> {
    let user = author(&ctx, &headers).await?;
    let run = orchestrator::approve(&ctx, id, input.when, &user).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (run.id, &run_label(&ctx, id).await),
        format!("Deploy aprovado{}", when_note(input.when)),
    )
    .await;
    Ok(format::json(events::view(&ctx.db, &run).await?)?)
}

async fn reject(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<RejectInput>,
) -> AppResult<Response> {
    let user = author(&ctx, &headers).await?;
    let run = orchestrator::reject(&ctx, id, &user, input.reason.as_deref()).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (run.id, &run_label(&ctx, id).await),
        "Deploy recusado".into(),
    )
    .await;
    Ok(format::json(events::view(&ctx.db, &run).await?)?)
}

async fn cancel(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let user = author(&ctx, &headers).await?;
    let run = orchestrator::cancel(&ctx, id, &user).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (run.id, &run_label(&ctx, id).await),
        "Deploy cancelado".into(),
    )
    .await;
    Ok(format::json(events::view(&ctx.db, &run).await?)?)
}

async fn restore(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ApproveInput>,
) -> AppResult<Response> {
    let user = author(&ctx, &headers).await?;
    let run = orchestrator::request_restore(&ctx, id, input.when, &user).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Execute,
        (run.id, &run_label(&ctx, run.id).await),
        format!(
            "Restauração da versão do deploy #{id} pedida{}",
            when_note(input.when)
        ),
    )
    .await;
    Ok((
        StatusCode::ACCEPTED,
        Json(events::view(&ctx.db, &run).await?),
    )
        .into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/deploys")
        .add("/", get(overview))
        .add("/agents", get(agents))
        .add("/setup", post(setup))
        .add("/readiness", get(readiness_for))
        .add("/credentials", post(store_credential))
        .add(
            "/credentials/{id}",
            put(update_credential).delete(destroy_credential),
        )
        .add("/applications", post(store_application))
        .add(
            "/applications/{id}",
            put(update_application).delete(destroy_application),
        )
        .add("/applications/{id}/workflow", get(workflow))
        .add("/applications/{id}/targets", post(store_target))
        .add("/targets/{id}", put(update_target).delete(destroy_target))
        .add("/targets/{id}/readiness", get(target_readiness))
        .add("/targets/{id}/runs", get(target_history))
        .add("/targets/{id}/deploy", post(deploy_target))
        .add("/targets/{id}/env", get(env_current).put(env_save))
        .add("/targets/{id}/env/versions", get(env_versions_list))
        .add("/targets/{id}/env/preview", post(env_preview))
        .add("/env-versions/{id}", get(env_version_content))
        .add("/runs/{id}", get(run_show))
        .add("/runs/{id}/log", get(run_log))
        .add("/runs/{id}/changes", get(run_changes))
        .add("/runs/{id}/backups", get(run_backups))
        .add("/runs/{id}/approve", post(approve))
        .add("/runs/{id}/reject", post(reject))
        .add("/runs/{id}/cancel", post(cancel))
        .add("/runs/{id}/restore", post(restore))
}

// ── Webhook ──────────────────────────────────────────────────────────────

/// O token do `Authorization: Bearer …`.
fn bearer(headers: &HeaderMap) -> AppResult<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            AppError::unauthorized(
                "Envie o token OIDC do GitHub Actions em Authorization: Bearer (veja o workflow \
                 gerado em Deploys)",
            )
        })
}

async fn github_hook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Response> {
    let token = bearer(&headers)?;
    let claims = oidc::verify(token, oidc::github_keys(), &oidc::audience()).await?;
    let input: HookInput = if body.is_empty() {
        HookInput::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|_| AppError::bad_request("Corpo inválido: envie JSON"))?
    };
    let response = webhook::handle(&ctx, &claims, &input).await?;
    tracing::info!(
        repository = %claims.repository,
        git_ref = %claims.git_ref,
        sha = %claims.sha,
        runs = response.runs.len(),
        "webhook de deploy do GitHub recebido"
    );
    Ok((StatusCode::ACCEPTED, Json(response)).into_response())
}

/// A rota pública do webhook: o ponto de montagem a registra fora do guarda
/// JWT.
pub fn hook_routes() -> Routes {
    Routes::new()
        .prefix("/deploy-hooks")
        .add("/github", post(github_hook))
}
