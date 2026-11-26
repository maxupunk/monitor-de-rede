//! Agentes remotos (ADR 011): administração e o canal do próprio agente.
//!
//! Dois públicos, como em `probes`: o operador, por JWT ([`routes`]), e o
//! agente, pelo código de enrollment ou pelo token ([`agent_routes`]). O
//! controller extrai, valida, delega a `services::agents` e serializa; a
//! ponte WebSocket ↔ canais é a única coisa de transporte que mora aqui.

use std::{
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

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
use netmonitor_core::{
    agents::install_policy::InstallPolicy,
    shared::{body::AppJson, client_addr::TrustedProxies},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
            protocol::{WireFrame, MAX_FRAME_BYTES},
            send_bounded,
            service::{
                disconnect_session, publish_status, to_view, AgentService, ENROLLMENT_TTL_SECONDS,
            },
        },
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        shared::errors::{AppError, AppResult},
    },
    views::agents::{AgentEnrollResponse, AgentEnrollmentView, AgentView, AgentWithEnrollment},
};

const TOKEN_HEADER: &str = "x-probe-token";
const WS_PING_INTERVAL: Duration = Duration::from_secs(25);
const WS_IDLE_TIMEOUT_SECS: u64 = 60;
const WS_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
/// Escrita no socket presa além disto (buffer cheio, agente sumido) é half-open.
const WS_SEND_TIMEOUT: Duration = Duration::from_secs(WS_IDLE_TIMEOUT_SECS);

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
/// Cabeçalhos `X-Forwarded-*` só valem quando o peer é um proxy confiável
/// (`TRUSTED_PROXIES`), e ambos passam por validação estrita.
fn request_origin(headers: &HeaderMap, peer: Option<IpAddr>) -> String {
    let trusted = peer.is_none_or(|ip| TrustedProxies::from_env().contains(ip));
    let forwarded_host = trusted
        .then(|| headers.get("x-forwarded-host"))
        .flatten()
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| install_address::is_safe(value));
    let host = forwarded_host
        .or_else(|| {
            headers
                .get(header::HOST)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| install_address::is_safe(value))
        })
        .unwrap_or("localhost");
    let scheme = trusted
        .then(|| headers.get("x-forwarded-proto"))
        .flatten()
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| matches!(*value, "http" | "https"))
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
    if let Err(error) = AuditService::new(&ctx.db)
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
        .await
    {
        tracing::warn!(%error, probe_id = probe.id, "falha ao registrar auditoria do agente");
    }
}

// --- Administração (JWT) ----------------------------------------------------

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let hub = AgentHub::from_context(&ctx).ok();
    let views = AgentService::new(&ctx.db).list_views(hub.as_ref()).await?;
    Ok(format::json(views)?)
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let probe = AgentService::new(&ctx.db).find(id).await?;
    Ok(format::json(view_of(&ctx, &probe).await?)?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    Json(input): Json<AgentCreateInput>,
) -> AppResult<Response> {
    let service = AgentService::new(&ctx.db);
    let (probe, code) = service.create(&input).await?;
    let devices = service.devices_of(std::slice::from_ref(&probe)).await?;
    let policy = InstallPolicy::of(&probe);
    let enrollment = enrollment_of(
        &ctx,
        code,
        devices.first(),
        &request_origin(&headers, peer),
        &policy,
    )
    .await?;
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
    publish_status(&ctx, &probe).await;
    Ok(format::json(view_of(&ctx, &probe).await?)?)
}

/// Novo código de enrollment (reinstalação do agente).
async fn enrollment(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let service = AgentService::new(&ctx.db);
    let code = service.reissue_code(id).await?;
    let probe = service.find(id).await?;
    let devices = service.devices_of(std::slice::from_ref(&probe)).await?;
    let policy = InstallPolicy::of(&probe);
    let enrollment = enrollment_of(
        &ctx,
        code,
        devices.first(),
        &request_origin(&headers, peer),
        &policy,
    )
    .await?;
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
/// endereço desta central ou outra política. O código não é reemitido nem
/// consumido: só volta estampado no texto (e a política escolhida fica salva no
/// cadastro quando o código está ativo).
async fn install_commands(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    PeerAddr(peer): PeerAddr,
    AppJson(input): AppJson<AgentInstallCommandsInput>,
) -> AppResult<Response> {
    let policy = InstallPolicy::chosen_or_suggested(input.policy.as_ref())?;
    if input.policy.is_some() {
        if let Err(error) = AgentService::new(&ctx.db)
            .save_policy_for_code(&input.code, &policy)
            .await
        {
            tracing::warn!(%error, "falha ao salvar política de instalação do agente");
        }
    }
    Ok(format::json(
        install_address::commands(
            &ctx,
            &input.code,
            input.address_id.as_deref(),
            None,
            &request_origin(&headers, peer),
            &policy,
        )
        .await?,
    )?)
}

/// `GET /api/agents/install-policy` — o que a tela marca por padrão ao
/// instalar um agente (também no script da VPN).
async fn install_policy() -> AppResult<Response> {
    Ok(format::json(InstallPolicy::suggested().view())?)
}

/// Código recém-emitido com os comandos do endereço padrão e da política salva.
async fn enrollment_of(
    ctx: &AppContext,
    code: String,
    device: Option<&crate::models::devices::Model>,
    origin: &str,
    policy: &InstallPolicy,
) -> AppResult<AgentEnrollmentView> {
    let commands = install_address::commands(ctx, &code, None, device, origin, policy).await?;
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
    disconnect_session(&ctx, probe.id);
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
    disconnect_session(&ctx, probe.id);
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
    // Reinstalação: encerra eventual conexão anterior que usava o token antigo.
    disconnect_session(&ctx, probe.id);
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

/// Liga o socket aos canais de quadros do serviço, com detecção ativa de
/// conexões half-open (Ping periódico + timeout de inatividade).
async fn bridge(ctx: AppContext, probe: probes::Model, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (incoming_tx, incoming_rx) = mpsc::channel::<WireFrame>(64);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<WireFrame>(64);
    let stop = CancellationToken::new();
    let started_at = tokio::time::Instant::now();
    let last_rx = Arc::new(AtomicU64::new(0));
    let probe_id = probe.id;

    let reader = {
        let stop = stop.clone();
        let last_rx = last_rx.clone();
        tokio::spawn(async move {
            loop {
                let next = tokio::select! {
                    biased;
                    () = stop.cancelled() => break,
                    next = stream.next() => next,
                };
                let Some(Ok(message)) = next else {
                    break;
                };
                last_rx.store(started_at.elapsed().as_secs(), Ordering::Relaxed);
                let frame = match message {
                    Message::Text(text) => WireFrame::Text(text.to_string()),
                    Message::Binary(bytes) => WireFrame::Binary(Vec::from(bytes)),
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) => continue,
                };
                if incoming_tx.send(frame).await.is_err() {
                    break;
                }
            }
            stop.cancel();
        })
    };
    let writer = {
        let stop = stop.clone();
        let last_rx = last_rx.clone();
        tokio::spawn(async move {
            let mut ping = tokio::time::interval(WS_PING_INTERVAL);
            ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            ping.tick().await;
            loop {
                tokio::select! {
                    biased;
                    () = stop.cancelled() => break,
                    frame = outgoing_rx.recv() => {
                        let message = match frame {
                            Some(WireFrame::Text(text)) => Message::Text(text.into()),
                            Some(WireFrame::Binary(bytes)) => Message::Binary(bytes.into()),
                            None => break,
                        };
                        if !send_bounded(&mut sink, message, &stop, WS_SEND_TIMEOUT).await {
                            break;
                        }
                    }
                    _ = ping.tick() => {
                        let idle = started_at
                            .elapsed()
                            .as_secs()
                            .saturating_sub(last_rx.load(Ordering::Relaxed));
                        if idle >= WS_IDLE_TIMEOUT_SECS {
                            tracing::warn!(
                                probe_id,
                                segundos = idle,
                                "conexão de agente sem atividade (half-open); encerrando"
                            );
                            break;
                        }
                        if !send_bounded(&mut sink, Message::Ping(Vec::new().into()), &stop, WS_SEND_TIMEOUT).await {
                            break;
                        }
                    }
                }
            }
            stop.cancel();
            let _ = tokio::time::timeout(WS_CLOSE_TIMEOUT, sink.close()).await;
        })
    };
    if let Err(error) = connection::serve(ctx, probe, incoming_rx, outgoing_tx).await {
        tracing::warn!(%error, probe_id, "conexão de agente recusada no handshake");
    }
    stop.cancel();
    reader.abort();
    if tokio::time::timeout(WS_CLOSE_TIMEOUT + Duration::from_secs(1), writer)
        .await
        .is_err()
    {
        tracing::debug!(
            probe_id,
            "encerramento do WebSocket do agente excedeu o prazo"
        );
    }
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
    let content_length = file.metadata().await.ok().map(|meta| meta.len());
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(disposition) = header::HeaderValue::from_str(&format!(
        r#"attachment; filename="netmonitor-agent-{arch}""#
    )) {
        headers.insert(header::CONTENT_DISPOSITION, disposition);
    }
    if let Some(len) = content_length {
        if let Ok(value) = header::HeaderValue::from_str(&len.to_string()) {
            headers.insert(header::CONTENT_LENGTH, value);
        }
    }
    Ok((
        StatusCode::OK,
        headers,
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
        .add("/install-policy", get(install_policy))
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
