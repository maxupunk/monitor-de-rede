//! CRUD administrativo de probes e o protocolo HTTP dos agentes (§7.10).
//!
//! São dois públicos no mesmo recurso: o operador, autenticado por JWT, e o
//! agente, autenticado pelo token do probe. Por isso as rotas saem em dois
//! grupos — ver [`routes`] e [`agent_routes`].

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    dtos::{optional_body, resources::ProbeInput},
    models::probes,
    services::{
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        events::EventBus,
        maintenance::resource_cleanup::ResourceCleanupService,
        probes::{
            dispatcher,
            liveness::{status_payload, STATUS_ONLINE},
            receiver::{self, ProbeDiscoveryResultPayload, ProbeResultPayload},
        },
        shared::{
            crypto::sha256_hex,
            errors::{AppError, AppResult},
        },
    },
};

/// Cabeçalho que o agente usa para se identificar.
const PROBE_TOKEN_HEADER: &str = "x-probe-token";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeResponse {
    id: i64,
    site_id: Option<i64>,
    name: String,
    status: String,
    version: Option<String>,
    last_seen_at: Option<String>,
    registered_at: Option<String>,
    revoked_at: Option<String>,
    configuration: Option<serde_json::Value>,
    created_at: String,
    updated_at: String,
    /// Token cru, só preenchido na criação. Em listagens e consultas vem `null`.
    token: Option<String>,
}
impl ProbeResponse {
    fn from_model(row: probes::Model, token: Option<String>) -> Self {
        Self {
            id: row.id,
            site_id: row.site_id,
            name: row.name,
            status: row.status,
            version: row.version,
            last_seen_at: row.last_seen_at.map(|v| v.to_rfc3339()),
            registered_at: row.registered_at.map(|v| v.to_rfc3339()),
            revoked_at: row.revoked_at.map(|v| v.to_rfc3339()),
            configuration: row.configuration,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
            token,
        }
    }
}
impl From<probes::Model> for ProbeResponse {
    fn from(row: probes::Model) -> Self {
        Self::from_model(row, None)
    }
}

/// Corpo do heartbeat. `token` no corpo é aceito porque agentes antigos o
/// mandavam assim, antes do cabeçalho existir.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeartbeatInput {
    token: Option<String>,
    version: Option<String>,
    configuration: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeResultsInput {
    token: Option<String>,
    #[serde(default)]
    results: Vec<ProbeResultPayload>,
    #[serde(default)]
    discovery_results: Vec<ProbeDiscoveryResultPayload>,
}

/// Publica no SSE apenas quando o estado do probe realmente muda.
///
/// Sem esta guarda, o heartbeat de 5 em 5 segundos inundaria o stream com um
/// evento por probe por ciclo, sem nenhuma informação nova.
async fn emit_status_if_changed(ctx: &AppContext, probe: &probes::Model, previous: &str) {
    if previous == probe.status {
        return;
    }
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(&ctx.db, "probe:status", status_payload(probe))
            .await
        {
            tracing::warn!(%error, probe_id = probe.id, "falha ao publicar probe:status");
        }
    }
}

/// Autentica o agente por `sha256(token)`.
///
/// O hash **não** é único (o `DEFAULT_VPN_PROBE_TOKEN` é compartilhado), então
/// a consulta pode devolver mais de uma linha e fica com a primeira. Probes
/// revogados já são excluídos em `Probe::find_by_token`.
async fn authenticate(
    ctx: &AppContext,
    headers: &HeaderMap,
    body_token: Option<&str>,
) -> AppResult<probes::Model> {
    let raw_token = headers
        .get(PROBE_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            body_token
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
        .ok_or_else(|| AppError::unauthorized("Probe não encontrado ou token inválido"))?;

    probes::Entity::find_by_token(&raw_token)
        .order_by_asc(probes::Column::Id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::unauthorized("Probe não encontrado ou token inválido"))
}

// --- Administração (JWT) ----------------------------------------------------

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = probes::Entity::find()
        .order_by_asc(probes::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        rows.into_iter()
            .map(ProbeResponse::from)
            .collect::<Vec<_>>(),
    )?)
}
async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ProbeInput>,
) -> AppResult<Response> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::validation("Nome do probe é obrigatório"))?;
    let token = input
        .token
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let token_hash = sha256_hex(&token);
    let row = probes::ActiveModel {
        site_id: Set(input.site_id),
        name: Set(name.into()),
        token_hash: Set(token_hash),
        status: Set(input.status.unwrap_or_else(|| "pending".into())),
        version: Set(input.version),
        configuration: Set(input.configuration),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::Probe,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!("Probe '{}' criado", row.name)),
                changes: None,
            },
        )
        .await;

    Ok((
        StatusCode::CREATED,
        Json(ProbeResponse::from_model(row, Some(token))),
    )
        .into_response())
}
async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    Ok(format::json(ProbeResponse::from(row))?)
}
async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ProbeInput>,
) -> AppResult<Response> {
    let old = probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    let old_for_audit = old.clone();
    let previous_status = old.status.clone();
    let row = probes::ActiveModel {
        id: Set(id),
        site_id: Set(input.site_id.or(old.site_id)),
        name: Set(input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(&old.name)
            .into()),
        status: Set(input.status.unwrap_or(old.status)),
        version: Set(input.version.or(old.version)),
        configuration: Set(input.configuration.or(old.configuration)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    emit_status_if_changed(&ctx, &row, &previous_status).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::Probe,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!("Probe '{}' atualizado", row.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(ProbeResponse::from(old_for_audit)).ok(),
                    new: serde_json::to_value(ProbeResponse::from(row.clone())).ok(),
                }),
            },
        )
        .await;

    Ok(format::json(ProbeResponse::from(row))?)
}
async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let old = probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    ResourceCleanupService::delete_probe(&ctx.db, id).await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::Probe,
                resource_id: Some(id),
                resource_label: Some(old.name.clone()),
                description: Some(format!("Probe '{}' excluído", old.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(ProbeResponse::from(old)).ok(),
                    new: None,
                }),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}
async fn revoke(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let old = probes::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Probe não encontrado"))?;
    let previous_status = old.status.clone();
    let mut active: probes::ActiveModel = old.clone().into();
    active.status = Set(probes::STATUS_REVOKED.into());
    active.revoked_at = Set(Some(Utc::now().into()));
    let saved = active.update(&ctx.db).await?;
    // A fila do agente revogado morre junto: entregar tarefa a quem perdeu o
    // acesso só produziria resultado que nunca chega de volta.
    dispatcher::clear_tasks_for_probe(&ctx.db, saved.id).await?;
    emit_status_if_changed(&ctx, &saved, &previous_status).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::Probe,
                resource_id: Some(saved.id),
                resource_label: Some(saved.name.clone()),
                description: Some(format!("Probe '{}' revogado", saved.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(ProbeResponse::from(old)).ok(),
                    new: serde_json::to_value(ProbeResponse::from(saved.clone())).ok(),
                }),
            },
        )
        .await;

    Ok(format::json(ProbeResponse::from(saved))?)
}
async fn test(Path(id): Path<i64>) -> AppResult<Response> {
    Ok(format::json(
        serde_json::json!({"message":format!("Teste de conectividade enviado para o probe ID {id}")}),
    )?)
}

// --- Protocolo do agente (token de probe) -----------------------------------

/// `POST /api/probes/heartbeat` — o agente diz que está vivo.
async fn heartbeat(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: String,
) -> AppResult<Response> {
    let input: HeartbeatInput = optional_body(&body);
    let probe = authenticate(&ctx, &headers, input.token.as_deref()).await?;
    let previous_status = probe.status.clone();
    let mut active: probes::ActiveModel = probe.into();
    active.status = Set(STATUS_ONLINE.into());
    active.last_seen_at = Set(Some(Utc::now().into()));
    if let Some(version) = input.version {
        active.version = Set(Some(version));
    }
    if let Some(configuration) = input.configuration {
        active.configuration = Set(Some(configuration));
    }
    let saved = active.update(&ctx.db).await?;
    emit_status_if_changed(&ctx, &saved, &previous_status).await;
    Ok(format::json(serde_json::json!({
        "status": "ok",
        "probeId": saved.id,
    }))?)
}

/// `GET /api/probes/tasks` — entrega e remove as tarefas pendentes.
async fn tasks(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let probe = authenticate(&ctx, &headers, None).await?;
    let tasks = dispatcher::get_pending_tasks(&ctx.db, probe.id).await?;
    let discovery_tasks = dispatcher::get_pending_discovery_tasks(&ctx.db, probe.id).await?;
    if let Some(task) = discovery_tasks.first() {
        if let Ok(session) =
            crate::services::discovery::service::ScanSessionService::from_context(&ctx)
        {
            session.remote_started(task.run_id).await;
        }
    }
    Ok(format::json(serde_json::json!({
        "tasks": tasks,
        "discoveryTasks": discovery_tasks,
    }))?)
}

/// `POST /api/probes/results` — o agente devolve o que mediu.
async fn results(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: String,
) -> AppResult<Response> {
    let input: ProbeResultsInput = optional_body(&body);
    let probe = authenticate(&ctx, &headers, input.token.as_deref()).await?;
    if !input.results.is_empty() {
        receiver::receive_batch_results(&ctx, probe.id, &input.results).await?;
    }
    if !input.discovery_results.is_empty() {
        receiver::receive_discovery_results(&ctx, probe.id, &input.discovery_results).await?;
    }
    // `count` é o que o agente mandou, não o que foi aceito; o agente usa o
    // campo somente para diagnóstico.
    Ok(format::json(serde_json::json!({
        "status": "processed",
        "count": input.results.len() + input.discovery_results.len(),
    }))?)
}

/// Rotas administrativas — vão atrás do guarda JWT.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/probes")
        .add("/", get(index).post(store))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/revoke", post(revoke))
        .add("/{id}/test", post(test))
}

/// Rotas do agente — **sem** JWT.
///
/// O probe não tem sessão de usuário: ele se autentica pelo `X-Probe-Token` em
/// cada requisição, dentro do próprio handler. Deixá-las no grupo protegido
/// devolveria 401 a todo agente e mataria o monitoramento remoto inteiro.
///
/// Os caminhos são estáticos (`/heartbeat`, `/tasks`, `/results`) e por isso
/// não conflitam com o `/probes/{id}` do grupo administrativo — o roteador
/// casa segmento literal antes de parâmetro.
pub fn agent_routes() -> Routes {
    Routes::new()
        .prefix("/probes")
        .add("/heartbeat", post(heartbeat))
        .add("/tasks", get(tasks))
        .add("/results", post(results))
}
