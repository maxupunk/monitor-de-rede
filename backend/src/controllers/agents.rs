//! Agentes remotos (ADR 011): administração e o canal do próprio agente.
//!
//! Dois públicos, como em `probes`: o operador, por JWT ([`routes`]), e o
//! agente, pelo código de enrollment ou pelo token ([`agent_routes`]). O
//! controller extrai, valida, delega a `services::agents` e serializa; a
//! ponte WebSocket ↔ canais é a única coisa de transporte que mora aqui.

use std::net::{IpAddr, SocketAddr};

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, FromRequestParts,
    },
    http::{header, request::Parts, HeaderMap, StatusCode},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use loco_rs::prelude::*;
use tokio::sync::mpsc;

use crate::{
    dtos::agents::{
        AgentCreateInput, AgentEnrollInput, AgentInstallCommandsInput, AgentUpdateInput,
    },
    models::probes,
    services::{
        agents::{
            connection,
            hub::AgentHub,
            install, install_address,
            protocol::MAX_FRAME_BYTES,
            service::{publish_status, to_view, AgentService, ENROLLMENT_TTL_SECONDS},
        },
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        shared::errors::{AppError, AppResult},
    },
    views::agents::{AgentEnrollResponse, AgentEnrollmentView, AgentView, AgentWithEnrollment},
};

const TOKEN_HEADER: &str = "x-probe-token";

/// IP de origem da conexão, quando o servidor o conhece.
struct PeerAddr(Option<IpAddr>);

impl<S: Send + Sync> FromRequestParts<S> for PeerAddr {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(addr)| addr.ip()),
        ))
    }
}

/// `scheme://host` de quem chamou — último recurso para a URL da central.
fn request_origin(headers: &HeaderMap) -> String {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost");
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("http");
    format!("{scheme}://{host}")
}

async fn view_of(ctx: &AppContext, probe: &probes::Model) -> AppResult<AgentView> {
    let service = AgentService::new(&ctx.db);
    let devices = service.devices_of(std::slice::from_ref(probe)).await?;
    let connected = AgentHub::from_context(ctx).is_ok_and(|hub| hub.get(probe.id).is_some());
    Ok(to_view(probe, devices.first(), connected))
}

async fn audit(
    ctx: &AppContext,
    headers: &HeaderMap,
    action: AuditAction,
    probe: &probes::Model,
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
                resource_type: ResourceType::Probe,
                resource_id: Some(probe.id),
                resource_label: Some(probe.name.clone()),
                description: Some(description.to_string()),
                changes: None,
            },
        )
        .await;
}

// --- Administração (JWT) ----------------------------------------------------

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let service = AgentService::new(&ctx.db);
    let agents = service.list().await?;
    let devices = service.devices_of(&agents).await?;
    let hub = AgentHub::from_context(&ctx).ok();
    let views: Vec<AgentView> = agents
        .iter()
        .map(|agent| {
            let device = devices
                .iter()
                .find(|device| Some(device.id) == agent.device_id);
            let connected = hub.as_ref().is_some_and(|hub| hub.get(agent.id).is_some());
            to_view(agent, device, connected)
        })
        .collect();
    Ok(format::json(views)?)
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let probe = AgentService::new(&ctx.db).find(id).await?;
    Ok(format::json(view_of(&ctx, &probe).await?)?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<AgentCreateInput>,
) -> AppResult<Response> {
    let service = AgentService::new(&ctx.db);
    let (probe, code) = service.create(&input).await?;
    let devices = service.devices_of(std::slice::from_ref(&probe)).await?;
    let enrollment = enrollment_of(&ctx, code, devices.first(), &request_origin(&headers)).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        &probe,
        "Agente remoto cadastrado",
    )
    .await;
    publish_status(&ctx, &probe).await;
    Ok((
        StatusCode::CREATED,
        Json(AgentWithEnrollment {
            agent: to_view(&probe, devices.first(), false),
            enrollment,
        }),
    )
        .into_response())
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<AgentUpdateInput>,
) -> AppResult<Response> {
    let probe = AgentService::new(&ctx.db).update(id, &input).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        &probe,
        "Agente remoto alterado",
    )
    .await;
    Ok(format::json(view_of(&ctx, &probe).await?)?)
}

/// Novo código de enrollment (reinstalação do agente).
async fn enrollment(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let service = AgentService::new(&ctx.db);
    let code = service.reissue_code(id).await?;
    let probe = service.find(id).await?;
    let devices = service.devices_of(std::slice::from_ref(&probe)).await?;
    let enrollment = enrollment_of(&ctx, code, devices.first(), &request_origin(&headers)).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        &probe,
        "Novo código de instalação emitido",
    )
    .await;
    Ok(format::json(enrollment)?)
}

/// `POST /api/agents/install-commands` — os mesmos comandos, para outro
/// endereço desta central. O código não é reemitido nem guardado: só volta
/// estampado no texto.
async fn install_commands(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<AgentInstallCommandsInput>,
) -> AppResult<Response> {
    Ok(format::json(
        install_address::commands(
            &ctx,
            &input.code,
            Some(input.address_id.as_str()),
            None,
            &request_origin(&headers),
        )
        .await?,
    )?)
}

/// Código recém-emitido com os comandos do endereço padrão.
async fn enrollment_of(
    ctx: &AppContext,
    code: String,
    device: Option<&crate::models::devices::Model>,
    origin: &str,
) -> AppResult<AgentEnrollmentView> {
    let commands = install_address::commands(ctx, &code, None, device, origin).await?;
    Ok(AgentEnrollmentView {
        code,
        expires_in_seconds: ENROLLMENT_TTL_SECONDS,
        commands,
    })
}

async fn revoke(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let probe = AgentService::new(&ctx.db).revoke(id).await?;
    if let Ok(hub) = AgentHub::from_context(&ctx) {
        if let Some(session) = hub.get(probe.id) {
            hub.unregister(&session);
        }
    }
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        &probe,
        "Agente remoto revogado",
    )
    .await;
    publish_status(&ctx, &probe).await;
    Ok(format::json(view_of(&ctx, &probe).await?)?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let probe = AgentService::new(&ctx.db).delete(id).await?;
    if let Ok(hub) = AgentHub::from_context(&ctx) {
        if let Some(session) = hub.get(probe.id) {
            hub.unregister(&session);
        }
    }
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        &probe,
        "Agente remoto removido",
    )
    .await;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// --- Protocolo do agente (código / token) -----------------------------------

/// `POST /api/agents/enroll` — troca o código de uso único pelo token.
async fn enroll(
    State(ctx): State<AppContext>,
    Json(input): Json<AgentEnrollInput>,
) -> AppResult<Response> {
    let (probe, token) = AgentService::new(&ctx.db).enroll(&input).await?;
    Ok(format::json(AgentEnrollResponse {
        probe_id: probe.id,
        name: probe.name,
        token,
    })?)
}

/// `GET /api/agents/connect` — abre o canal (WebSocket).
async fn connect(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    upgrade: WebSocketUpgrade,
) -> AppResult<Response> {
    let token = headers
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let probe = AgentService::new(&ctx.db).authenticate(token, peer).await?;
    Ok(upgrade
        .max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| bridge(ctx, probe, socket)))
}

/// Liga o socket aos canais de texto do serviço.
async fn bridge(ctx: AppContext, probe: probes::Model, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (incoming_tx, incoming_rx) = mpsc::channel::<String>(64);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<String>(64);
    let reader = tokio::spawn(async move {
        while let Some(Ok(message)) = stream.next().await {
            match message {
                Message::Text(text) => {
                    if incoming_tx.send(text.to_string()).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    let writer = tokio::spawn(async move {
        while let Some(text) = outgoing_rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    let probe_id = probe.id;
    if let Err(error) = connection::serve(ctx, probe, incoming_rx, outgoing_tx).await {
        tracing::warn!(%error, probe_id, "conexão de agente recusada no handshake");
    }
    reader.abort();
    let _ = writer.await;
}

/// `GET /api/agents/install.sh` — script de instalação systemd (sem segredo).
async fn install_script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/x-shellscript; charset=utf-8")],
        install::script(),
    )
}

/// `GET /api/agents/download/{arch}` — binário do agente.
async fn download(Path(arch): Path<String>) -> AppResult<Response> {
    let path = install::binary_path(&arch)
        .ok_or_else(|| AppError::not_found("Arquitetura não suportada"))?;
    let file = tokio::fs::File::open(&path).await.map_err(|_| {
        AppError::not_found("Binário do agente não publicado nesta central (AGENT_DIST_DIR)")
    })?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(r#"attachment; filename="netmonitor-agent-{arch}""#),
            ),
        ],
        Body::from_stream(tokio_util::io::ReaderStream::new(file)),
    )
        .into_response())
}

/// Rotas administrativas — atrás do guarda JWT.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/agents")
        .add("/", get(index).post(store))
        .add("/install-commands", post(install_commands))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/enrollment", post(enrollment))
        .add("/{id}/revoke", post(revoke))
}

/// Rotas do agente — **sem** JWT: autenticadas no handler pelo código de
/// enrollment ou pelo token. Os caminhos são literais e não conflitam com o
/// `/agents/{id}` administrativo.
pub fn agent_routes() -> Routes {
    Routes::new()
        .prefix("/agents")
        .add("/enroll", post(enroll))
        .add("/connect", get(connect))
        .add("/install.sh", get(install_script))
        .add("/download/{arch}", get(download))
}
