//! Endpoints de monitoramento e administração da Docker Engine.
//!
//! Cada rota existe duas vezes: `/api/docker/...` (a central, como sempre
//! foi) e `/api/docker/hosts/{host}/...` (a central com `host=local`, ou um
//! servidor remoto com `host=agent-<id>`, ADR 011). O handler não sabe qual:
//! recebe o [`DockerHost`] já resolvido e usa os mesmos serviços.

use async_compression::tokio::bufread::GzipEncoder;
use axum::{
    body::Body,
    extract::{FromRequestParts, RawPathParams},
    http::{header, request::Parts, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::MethodRouter,
};
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::io::BufReader;

use crate::{
    dtos::{
        agents::{DockerComposeInput, DockerFollowLogsInput, DockerHistoryQuery, DockerPullInput},
        docker::{
            DockerForceQuery, DockerLogsQuery, DockerNetworkConnectionInput,
            DockerNetworkCreateInput,
        },
    },
    services::{
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        docker::{
            self, compose,
            engine::{self, ContainerAction, LogFilters},
            hosts::{self, DockerHost, HostKey},
            log_stream::LogStreams,
            maintenance::ComposeRequest,
            operations::{self, Operation},
            realtime, volume_export,
        },
        shared::errors::{AppError, AppResult},
        telemetry::store,
    },
    views::{
        agents::DockerLogStreamStarted,
        telemetry::{ContainerHistoryResponse, HostHistoryResponse},
    },
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DockerListing<T: Serialize> {
    available: bool,
    data: T,
}

/// Parâmetros de caminho por nome: a mesma rota recebe ou não o `{host}`.
#[derive(Deserialize)]
struct IdPath {
    id: String,
}

#[derive(Deserialize)]
struct NamePath {
    name: String,
}

#[derive(Deserialize)]
struct ProjectPath {
    project: String,
}

#[derive(Deserialize)]
struct StreamPath {
    stream_id: String,
}

/// O host da rota (`local` quando não há `{host}`), já resolvido para as
/// fontes. Agente desconectado vira 503.
struct Target(DockerHost);

impl FromRequestParts<AppContext> for Target {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        ctx: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        let params = RawPathParams::from_request_parts(parts, ctx)
            .await
            .map_err(|_| AppError::validation("Host Docker inválido"))?;
        let key = params
            .iter()
            .find(|(name, _)| *name == "host")
            .map_or(Ok(HostKey::Local), |(_, value)| value.parse::<HostKey>())?;
        Ok(Self(hosts::resolve(ctx, key)?))
    }
}

/// Rótulo de auditoria: recursos remotos levam o host na frente.
fn label(host: &DockerHost, resource: &str) -> String {
    if host.key.is_local() {
        resource.to_string()
    } else {
        format!("{}/{resource}", host.key)
    }
}

async fn host_list(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(hosts::list(&ctx).await?)?)
}

async fn status(Target(host): Target) -> AppResult<Response> {
    Ok(format::json(engine::status(host.engine.as_ref()).await)?)
}

async fn container_metrics(
    State(ctx): State<AppContext>,
    Target(host): Target,
) -> AppResult<Response> {
    Ok(format::json(host.metrics(&ctx).await)?)
}

async fn containers(Target(host): Target) -> AppResult<Response> {
    listing(engine::list_containers(host.engine.as_ref())).await
}

async fn container(Target(host): Target, Path(path): Path<IdPath>) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Container")?;
    Ok(format::json(
        engine::inspect_container(host.engine.as_ref(), &id).await?,
    )?)
}

async fn container_logs(
    Target(host): Target,
    Path(path): Path<IdPath>,
    Query(query): Query<DockerLogsQuery>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Container")?;
    if query.since.is_some_and(|value| value < 0)
        || query.until.is_some_and(|value| value < 0)
        || query
            .since
            .zip(query.until)
            .is_some_and(|(since, until)| since > until)
    {
        return Err(AppError::validation(
            "O intervalo informado para os logs é inválido",
        ));
    }
    let tail = validate_tail(query.tail.unwrap_or_else(|| "200".to_string()))?;
    Ok(format::json(
        engine::container_logs(
            host.engine.as_ref(),
            &id,
            LogFilters {
                tail,
                since: query.since.unwrap_or_default(),
                until: query.until.unwrap_or_default(),
                timestamps: query.timestamps.unwrap_or(true),
            },
        )
        .await?,
    )?)
}

fn validate_tail(tail: String) -> AppResult<String> {
    if tail != "all"
        && !tail
            .parse::<usize>()
            .is_ok_and(|value| (1..=10_000).contains(&value))
    {
        return Err(AppError::validation(
            "tail deve ser 'all' ou um número entre 1 e 10000",
        ));
    }
    Ok(tail)
}

/// `POST .../containers/{id}/logs/follow` — começa a acompanhar o log. As
/// linhas chegam pelo SSE global como `docker:log`.
async fn follow_logs(
    State(ctx): State<AppContext>,
    Target(host): Target,
    Path(path): Path<IdPath>,
    body: String,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Container")?;
    let input: DockerFollowLogsInput = crate::dtos::optional_body(&body);
    let tail = validate_tail(input.tail.unwrap_or_else(|| "100".to_string()))?;
    let stream_id = LogStreams::from_context(&ctx)?.start(&ctx, host, id, tail)?;
    Ok(format::json(DockerLogStreamStarted { stream_id })?)
}

/// `DELETE /api/docker/log-streams/{stream_id}`.
async fn stop_log_stream(
    State(ctx): State<AppContext>,
    Path(path): Path<StreamPath>,
) -> AppResult<Response> {
    let stopped = LogStreams::from_context(&ctx)?.stop(&path.stream_id);
    Ok(format::json(serde_json::json!({ "stopped": stopped }))?)
}

async fn clear_container_logs(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Container")?;
    let response = host.maintenance.clear_logs(&id).await?;
    realtime::refresh(&ctx, host.key).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        ResourceType::DockerContainer,
        &label(&host, &id),
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn start_container(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> AppResult<Response> {
    container_action(&ctx, &host, &headers, &path.id, ContainerAction::Start).await
}

async fn stop_container(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> AppResult<Response> {
    container_action(&ctx, &host, &headers, &path.id, ContainerAction::Stop).await
}

async fn restart_container(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> AppResult<Response> {
    container_action(&ctx, &host, &headers, &path.id, ContainerAction::Restart).await
}

async fn remove_container(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Query(query): Query<DockerForceQuery>,
) -> AppResult<Response> {
    container_action(
        &ctx,
        &host,
        &headers,
        &path.id,
        ContainerAction::Remove {
            force: query.force.unwrap_or(false),
        },
    )
    .await
}

async fn container_action(
    ctx: &AppContext,
    host: &DockerHost,
    headers: &HeaderMap,
    id: &str,
    action: ContainerAction,
) -> AppResult<Response> {
    let id = docker::validate_identifier(id, "Container")?;
    let response = engine::container_action(host.engine.as_ref(), &id, action).await?;
    realtime::refresh(ctx, host.key).await;
    audit(
        ctx,
        headers,
        if matches!(action, ContainerAction::Remove { .. }) {
            AuditAction::Delete
        } else {
            AuditAction::Update
        },
        ResourceType::DockerContainer,
        &label(host, &id),
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

/// `POST .../containers/{id}/update` — pull da tag e recriação com rollback.
async fn update_container(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Container")?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        ResourceType::DockerContainer,
        &label(&host, &id),
        "Atualização de imagem e recriação solicitadas",
    )
    .await;
    let maintenance = host.maintenance.clone();
    let target = id.clone();
    let accepted = operations::start(
        &ctx,
        Operation {
            host: host.key,
            kind: "update",
            target: id,
        },
        move |progress| async move { maintenance.update_container(&target, progress).await },
    );
    Ok((StatusCode::ACCEPTED, Json(accepted)).into_response())
}

/// `POST .../images/pull`.
async fn pull_image(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Json(input): Json<DockerPullInput>,
) -> AppResult<Response> {
    let image = docker::validate_identifier(&input.image, "Imagem")?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        ResourceType::DockerImage,
        &label(&host, &image),
        "Pull de imagem solicitado",
    )
    .await;
    let maintenance = host.maintenance.clone();
    let target = image.clone();
    let accepted = operations::start(
        &ctx,
        Operation {
            host: host.key,
            kind: "pull",
            target: image,
        },
        move |progress| async move { maintenance.pull_image(&target, progress).await },
    );
    Ok((StatusCode::ACCEPTED, Json(accepted)).into_response())
}

/// `GET .../compose` — projetos compose do host.
async fn compose_projects(Target(host): Target) -> AppResult<Response> {
    listing(async {
        engine::list_containers(host.engine.as_ref())
            .await
            .map(|containers| compose::projects(&containers))
    })
    .await
}

/// `POST .../compose/{project}` — ação de projeto, executada pelo agente.
async fn compose_action(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<ProjectPath>,
    Json(input): Json<DockerComposeInput>,
) -> AppResult<Response> {
    let project = docker::validate_identifier(&path.project, "Projeto")?;
    let service = input
        .service
        .as_deref()
        .map(|service| docker::validate_identifier(service, "Serviço"))
        .transpose()?;
    let request = ComposeRequest {
        project: project.clone(),
        action: input.action,
        service: service.clone(),
    };
    let target = service.map_or_else(|| project.clone(), |service| format!("{project}/{service}"));
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        ResourceType::DockerContainer,
        &label(&host, &target),
        &format!("compose {} solicitado", request.action.label()),
    )
    .await;
    let maintenance = host.maintenance.clone();
    let accepted = operations::start(
        &ctx,
        Operation {
            host: host.key,
            kind: "compose",
            target,
        },
        move |progress| async move { maintenance.compose(&request, progress).await },
    );
    Ok((StatusCode::ACCEPTED, Json(accepted)).into_response())
}

async fn volumes(Target(host): Target) -> AppResult<Response> {
    listing(engine::list_volumes(host.engine.as_ref())).await
}

async fn volume(Target(host): Target, Path(path): Path<NamePath>) -> AppResult<Response> {
    let name = docker::validate_identifier(&path.name, "Volume")?;
    Ok(format::json(
        engine::inspect_volume(host.engine.as_ref(), &name).await?,
    )?)
}

async fn remove_volume(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<NamePath>,
    Query(query): Query<DockerForceQuery>,
) -> AppResult<Response> {
    let name = docker::validate_identifier(&path.name, "Volume")?;
    let response =
        engine::remove_volume(host.engine.as_ref(), &name, query.force.unwrap_or(false)).await?;
    realtime::refresh(&ctx, host.key).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerVolume,
        &label(&host, &name),
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

/// Exportação transmite o tar pelo socket local; não atravessa o canal do
/// agente.
async fn export_volume(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<NamePath>,
) -> AppResult<Response> {
    if !host.key.is_local() {
        return Err(AppError::business_rule(
            "A exportação de volume só está disponível para o Docker desta central",
        ));
    }
    let name = docker::validate_identifier(&path.name, "Volume")?;
    let export = volume_export::export(&name).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        ResourceType::DockerVolume,
        &name,
        "Exportação de volume iniciada",
    )
    .await;
    let file_name = export.file_name.clone();
    let gzip = GzipEncoder::new(BufReader::new(export));
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(gzip));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/gzip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(r#"attachment; filename="{file_name}""#),
            ),
        ],
        body,
    )
        .into_response())
}

async fn networks(Target(host): Target) -> AppResult<Response> {
    listing(engine::list_networks(host.engine.as_ref())).await
}

async fn network(Target(host): Target, Path(path): Path<IdPath>) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Rede")?;
    Ok(format::json(
        engine::inspect_network(host.engine.as_ref(), &id).await?,
    )?)
}

async fn create_network(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Json(input): Json<DockerNetworkCreateInput>,
) -> AppResult<Response> {
    let name = docker::validate_identifier(&input.name, "Nome da rede")?;
    let driver = input.driver.unwrap_or_else(|| "bridge".to_string());
    if !matches!(driver.as_str(), "bridge" | "overlay" | "macvlan" | "ipvlan") {
        return Err(AppError::validation("Driver de rede não suportado"));
    }
    let response = engine::create_network(host.engine.as_ref(), name.clone(), driver).await?;
    realtime::refresh(&ctx, host.key).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        ResourceType::DockerNetwork,
        &label(&host, &name),
        &response.message,
    )
    .await;
    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn remove_network(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Rede")?;
    let response = engine::remove_network(host.engine.as_ref(), &id).await?;
    realtime::refresh(&ctx, host.key).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerNetwork,
        &label(&host, &id),
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn connect_network(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Json(input): Json<DockerNetworkConnectionInput>,
) -> AppResult<Response> {
    network_connection(&ctx, &host, &headers, &path.id, input, false).await
}

async fn disconnect_network(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Json(input): Json<DockerNetworkConnectionInput>,
) -> AppResult<Response> {
    network_connection(&ctx, &host, &headers, &path.id, input, true).await
}

async fn network_connection(
    ctx: &AppContext,
    host: &DockerHost,
    headers: &HeaderMap,
    id: &str,
    input: DockerNetworkConnectionInput,
    disconnect: bool,
) -> AppResult<Response> {
    let id = docker::validate_identifier(id, "Rede")?;
    let container = docker::validate_identifier(&input.container_id, "Container")?;
    let response = if disconnect {
        engine::disconnect_network(
            host.engine.as_ref(),
            &id,
            container,
            input.force.unwrap_or(false),
        )
        .await?
    } else {
        engine::connect_network(host.engine.as_ref(), &id, container).await?
    };
    realtime::refresh(ctx, host.key).await;
    audit(
        ctx,
        headers,
        AuditAction::Update,
        ResourceType::DockerNetwork,
        &label(host, &id),
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn images(Target(host): Target) -> AppResult<Response> {
    listing(engine::list_images(host.engine.as_ref())).await
}

async fn image(Target(host): Target, Path(path): Path<IdPath>) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Imagem")?;
    Ok(format::json(
        engine::inspect_image(host.engine.as_ref(), &id).await?,
    )?)
}

async fn remove_image(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
    Path(path): Path<IdPath>,
    Query(query): Query<DockerForceQuery>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&path.id, "Imagem")?;
    let response =
        engine::remove_image(host.engine.as_ref(), &id, query.force.unwrap_or(false)).await?;
    realtime::refresh(&ctx, host.key).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerImage,
        &label(&host, &id),
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn prune_images(
    State(ctx): State<AppContext>,
    Target(host): Target,
    headers: HeaderMap,
) -> AppResult<Response> {
    let response = engine::prune_images(host.engine.as_ref()).await?;
    realtime::refresh(&ctx, host.key).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerImage,
        &label(&host, "dangling"),
        "Imagens sem uso removidas",
    )
    .await;
    Ok(format::json(response)?)
}

/// `GET .../history` — série do host e uso por container na janela. Leitura
/// de histórico sob demanda, não um ciclo de atualização.
async fn history(
    State(ctx): State<AppContext>,
    Path(host): Path<HostPath>,
    Query(query): Query<DockerHistoryQuery>,
) -> AppResult<Response> {
    let key = host.key()?;
    let range = query.range.unwrap_or_default();
    let now = chrono::Utc::now();
    let host_key = key.to_string();
    let (host_points, containers) = tokio::try_join!(
        store::host_history(&ctx.db, &host_key, range, now),
        store::container_usage(&ctx.db, &host_key, range, now)
    )?;
    Ok(format::json(HostHistoryResponse {
        host_key,
        range,
        step_minutes: range.step_minutes(),
        host: host_points,
        containers,
    })?)
}

/// `GET .../history/containers/{name}`.
async fn container_history(
    State(ctx): State<AppContext>,
    Path(path): Path<HostNamePath>,
    Query(query): Query<DockerHistoryQuery>,
) -> AppResult<Response> {
    let key = path.key()?;
    let name = docker::validate_identifier(&path.name, "Container")?;
    let range = query.range.unwrap_or_default();
    let host_key = key.to_string();
    let points =
        store::container_history(&ctx.db, &host_key, &name, range, chrono::Utc::now()).await?;
    Ok(format::json(ContainerHistoryResponse {
        host_key,
        container_name: name,
        range,
        step_minutes: range.step_minutes(),
        points,
    })?)
}

/// O histórico é do banco: não exige o agente conectado.
#[derive(Deserialize)]
struct HostPath {
    host: Option<String>,
}

impl HostPath {
    fn key(&self) -> AppResult<HostKey> {
        self.host
            .as_deref()
            .map_or(Ok(HostKey::Local), str::parse::<HostKey>)
    }
}

#[derive(Deserialize)]
struct HostNamePath {
    host: Option<String>,
    name: String,
}

impl HostNamePath {
    fn key(&self) -> AppResult<HostKey> {
        HostPath {
            host: self.host.clone(),
        }
        .key()
    }
}

async fn listing<T: Serialize>(
    operation: impl std::future::Future<Output = Result<Vec<T>, docker::DockerError>>,
) -> AppResult<Response> {
    match operation.await {
        Ok(data) => Ok(format::json(DockerListing {
            available: true,
            data,
        })?),
        Err(docker::DockerError::Disabled | docker::DockerError::Unavailable) => {
            Ok(format::json(DockerListing::<Vec<T>> {
                available: false,
                data: Vec::new(),
            })?)
        }
        Err(error) => Err(error.into()),
    }
}

async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    resource_type: ResourceType,
    label: &str,
    description: &str,
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
                resource_id: None,
                resource_label: Some(label.chars().take(255).collect()),
                description: Some(description.to_string()),
                changes: None,
            },
        )
        .await;
}

/// Rotas que existem por host. Registradas com e sem o `{host}`.
fn per_host() -> Vec<(&'static str, MethodRouter<AppContext>)> {
    vec![
        ("/status", get(status)),
        ("/metrics", get(container_metrics)),
        ("/history", get(history)),
        ("/history/containers/{name}", get(container_history)),
        ("/compose", get(compose_projects)),
        ("/compose/{project}", post(compose_action)),
        ("/containers", get(containers)),
        ("/containers/{id}/logs/follow", post(follow_logs)),
        (
            "/containers/{id}/logs",
            get(container_logs).delete(clear_container_logs),
        ),
        ("/containers/{id}/start", post(start_container)),
        ("/containers/{id}/stop", post(stop_container)),
        ("/containers/{id}/restart", post(restart_container)),
        ("/containers/{id}/update", post(update_container)),
        ("/containers/{id}", get(container).delete(remove_container)),
        ("/volumes", get(volumes)),
        ("/volumes/{name}/export", get(export_volume)),
        ("/volumes/{name}", get(volume).delete(remove_volume)),
        ("/networks", get(networks).post(create_network)),
        ("/networks/{id}/connect", post(connect_network)),
        ("/networks/{id}/disconnect", post(disconnect_network)),
        ("/networks/{id}", get(network).delete(remove_network)),
        ("/images/prune", post(prune_images)),
        ("/images/pull", post(pull_image)),
        ("/images", get(images)),
        ("/images/{id}", get(image).delete(remove_image)),
    ]
}

pub fn routes() -> Routes {
    let mut routes = Routes::new()
        .prefix("/docker")
        .add("/hosts", get(host_list))
        .add("/log-streams/{stream_id}", delete(stop_log_stream));
    for (path, handler) in per_host() {
        routes = routes
            .add(path, handler.clone())
            .add(&format!("/hosts/{{host}}{path}"), handler);
    }
    routes
}
