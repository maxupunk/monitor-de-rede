//! Endpoints de monitoramento e administração da Docker Engine.

use async_compression::tokio::bufread::GzipEncoder;
use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;
use serde::Serialize;
use tokio::io::BufReader;

use crate::{
    dtos::docker::{
        DockerForceQuery, DockerLogsQuery, DockerNetworkConnectionInput, DockerNetworkCreateInput,
    },
    services::{
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        docker::{
            self,
            engine::{self, ContainerAction, LogFilters},
            log_clear, metrics, realtime, volume_export,
        },
        shared::errors::{AppError, AppResult},
    },
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DockerListing<T: Serialize> {
    available: bool,
    data: T,
}

async fn status() -> AppResult<Response> {
    Ok(format::json(engine::status().await)?)
}

async fn container_metrics(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(metrics::overview(&ctx).await)?)
}

async fn containers() -> AppResult<Response> {
    listing(engine::list_containers()).await
}

async fn container(Path(id): Path<String>) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Container")?;
    Ok(format::json(engine::inspect_container(&id).await?)?)
}

async fn container_logs(
    Path(id): Path<String>,
    Query(query): Query<DockerLogsQuery>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Container")?;
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
    let tail = query.tail.unwrap_or_else(|| "200".to_string());
    if tail != "all"
        && !tail
            .parse::<usize>()
            .is_ok_and(|value| (1..=10_000).contains(&value))
    {
        return Err(AppError::validation(
            "tail deve ser 'all' ou um número entre 1 e 10000",
        ));
    }
    Ok(format::json(
        engine::container_logs(
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

async fn clear_container_logs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Container")?;
    let response = log_clear::clear(&id).await?;
    emit_docker_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        ResourceType::DockerContainer,
        &id,
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn start_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    container_action(&ctx, &headers, id, ContainerAction::Start).await
}

async fn stop_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    container_action(&ctx, &headers, id, ContainerAction::Stop).await
}

async fn restart_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    container_action(&ctx, &headers, id, ContainerAction::Restart).await
}

async fn remove_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<DockerForceQuery>,
) -> AppResult<Response> {
    container_action(
        &ctx,
        &headers,
        id,
        ContainerAction::Remove {
            force: query.force.unwrap_or(false),
        },
    )
    .await
}

async fn container_action(
    ctx: &AppContext,
    headers: &HeaderMap,
    id: String,
    action: ContainerAction,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Container")?;
    let response = engine::container_action(&id, action).await?;
    emit_docker_updated(ctx).await;
    audit(
        ctx,
        headers,
        if matches!(action, ContainerAction::Remove { .. }) {
            AuditAction::Delete
        } else {
            AuditAction::Update
        },
        ResourceType::DockerContainer,
        &id,
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn volumes() -> AppResult<Response> {
    listing(engine::list_volumes()).await
}

async fn volume(Path(name): Path<String>) -> AppResult<Response> {
    let name = docker::validate_identifier(&name, "Volume")?;
    Ok(format::json(engine::inspect_volume(&name).await?)?)
}

async fn remove_volume(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Query(query): Query<DockerForceQuery>,
) -> AppResult<Response> {
    let name = docker::validate_identifier(&name, "Volume")?;
    let response = engine::remove_volume(&name, query.force.unwrap_or(false)).await?;
    emit_docker_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerVolume,
        &name,
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn export_volume(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> AppResult<Response> {
    let name = docker::validate_identifier(&name, "Volume")?;
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

async fn networks() -> AppResult<Response> {
    listing(engine::list_networks()).await
}

async fn network(Path(id): Path<String>) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Rede")?;
    Ok(format::json(engine::inspect_network(&id).await?)?)
}

async fn create_network(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DockerNetworkCreateInput>,
) -> AppResult<Response> {
    let name = docker::validate_identifier(&input.name, "Nome da rede")?;
    let driver = input.driver.unwrap_or_else(|| "bridge".to_string());
    if !matches!(driver.as_str(), "bridge" | "overlay" | "macvlan" | "ipvlan") {
        return Err(AppError::validation("Driver de rede não suportado"));
    }
    let response = engine::create_network(name.clone(), driver).await?;
    emit_docker_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        ResourceType::DockerNetwork,
        &name,
        &response.message,
    )
    .await;
    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn remove_network(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Rede")?;
    let response = engine::remove_network(&id).await?;
    emit_docker_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerNetwork,
        &id,
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn connect_network(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<DockerNetworkConnectionInput>,
) -> AppResult<Response> {
    network_connection(&ctx, &headers, id, input, false).await
}

async fn disconnect_network(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<DockerNetworkConnectionInput>,
) -> AppResult<Response> {
    network_connection(&ctx, &headers, id, input, true).await
}

async fn network_connection(
    ctx: &AppContext,
    headers: &HeaderMap,
    id: String,
    input: DockerNetworkConnectionInput,
    disconnect: bool,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Rede")?;
    let container = docker::validate_identifier(&input.container_id, "Container")?;
    let response = if disconnect {
        engine::disconnect_network(&id, container, input.force.unwrap_or(false)).await?
    } else {
        engine::connect_network(&id, container).await?
    };
    emit_docker_updated(ctx).await;
    audit(
        ctx,
        headers,
        AuditAction::Update,
        ResourceType::DockerNetwork,
        &id,
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn images() -> AppResult<Response> {
    listing(engine::list_images()).await
}

async fn image(Path(id): Path<String>) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Imagem")?;
    Ok(format::json(engine::inspect_image(&id).await?)?)
}

async fn remove_image(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<DockerForceQuery>,
) -> AppResult<Response> {
    let id = docker::validate_identifier(&id, "Imagem")?;
    let response = engine::remove_image(&id, query.force.unwrap_or(false)).await?;
    emit_docker_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerImage,
        &id,
        &response.message,
    )
    .await;
    Ok(format::json(response)?)
}

async fn prune_images(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    let response = engine::prune_images().await?;
    emit_docker_updated(&ctx).await;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        ResourceType::DockerImage,
        "dangling",
        "Imagens sem uso removidas",
    )
    .await;
    Ok(format::json(response)?)
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

async fn emit_docker_updated(ctx: &AppContext) {
    let ctx = ctx.clone();
    tokio::spawn(async move { realtime::publish_all(&ctx).await });
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/docker")
        .add("/status", get(status))
        .add("/metrics", get(container_metrics))
        .add("/containers", get(containers))
        .add("/containers/{id}/logs", get(container_logs))
        .add("/containers/{id}/logs", delete(clear_container_logs))
        .add("/containers/{id}/start", post(start_container))
        .add("/containers/{id}/stop", post(stop_container))
        .add("/containers/{id}/restart", post(restart_container))
        .add("/containers/{id}", get(container))
        .add("/containers/{id}", delete(remove_container))
        .add("/volumes", get(volumes))
        .add("/volumes/{name}/export", get(export_volume))
        .add("/volumes/{name}", get(volume))
        .add("/volumes/{name}", delete(remove_volume))
        .add("/networks", get(networks))
        .add("/networks", post(create_network))
        .add("/networks/{id}/connect", post(connect_network))
        .add("/networks/{id}/disconnect", post(disconnect_network))
        .add("/networks/{id}", get(network))
        .add("/networks/{id}", delete(remove_network))
        .add("/images/prune", post(prune_images))
        .add("/images", get(images))
        .add("/images/{id}", get(image))
        .add("/images/{id}", delete(remove_image))
}
