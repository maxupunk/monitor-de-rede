//! Painel do servidor WireGuard (§7.13).
//!
//! Configuração, estado dos túneis, auto-detecção de endpoint e teste de
//! pré-voo (CGNAT).

use loco_rs::prelude::*;
use serde::Deserialize;
use serde_json::json;

use crate::{
    dtos::optional_body,
    services::{
        events::EventBus,
        shared::errors::AppResult,
        vpn::{
            preflight,
            profiles::{registry, PERSISTENT_KEEPALIVE_SECONDS},
            server_service::{self, VpnServerPayload, DEFAULT_LISTEN_PORT},
        },
    },
    views::vpn::{VpnServerResponse, VpnServerStateResponse},
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VpnServerInput {
    cidr: Option<String>,
    site_id: Option<i64>,
    network_id: Option<i64>,
    listen_port: Option<i32>,
    public_endpoint: Option<String>,
    mtu: Option<i32>,
    dns_servers: Option<String>,
    allow_peer_to_peer: Option<bool>,
    active: Option<bool>,
}

impl From<VpnServerInput> for VpnServerPayload {
    fn from(input: VpnServerInput) -> Self {
        Self {
            cidr: input.cidr,
            site_id: input.site_id,
            network_id: input.network_id,
            listen_port: input.listen_port,
            public_endpoint: input.public_endpoint,
            mtu: input.mtu,
            dns_servers: input.dns_servers,
            allow_peer_to_peer: input.allow_peer_to_peer,
            active: input.active,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreflightInput {
    public_endpoint: Option<String>,
    listen_port: Option<i32>,
}

/// `GET /api/vpn/server` — sincroniza a telemetria antes de responder.
async fn show(State(ctx): State<AppContext>) -> AppResult<Response> {
    let state = server_service::get_state(&ctx.db).await?;
    Ok(format::json(VpnServerStateResponse {
        configured: state.server.is_some(),
        server: state.server.map(VpnServerResponse::from),
        cidr: state.cidr,
        server_address: state.server_address,
        peers_total: state.peers_total,
        peers_connected: state.peers_connected,
        bytes_rx: state.bytes_rx,
        bytes_tx: state.bytes_tx,
        persistent_keepalive: PERSISTENT_KEEPALIVE_SECONDS,
        profiles: registry::list(),
    })?)
}

/// `PUT /api/vpn/server` — salva e aplica via arquivo compartilhado + `syncconf`.
async fn update(
    State(ctx): State<AppContext>,
    Json(input): Json<VpnServerInput>,
) -> AppResult<Response> {
    let (server, network) = server_service::create_or_update(&ctx.db, &input.into()).await?;
    emit_vpn_peers_updated(&ctx).await;

    // O `public_endpoint` é a origem do endereço "Internet" da lista de
    // endereços deste servidor. Se havia ali uma correção manual e ela acabou
    // de virar cópia do que foi gravado aqui, some com ela — senão a próxima
    // mudança deste campo valeria para os peers e não para o syslog, e as duas
    // telas passariam a apontar endereços diferentes sem dizer nada.
    if let Some(endpoint) = server.public_endpoint.as_deref() {
        crate::services::server_addresses::forget_override_if(&ctx.db, "public", endpoint).await?;
    }

    Ok(format::json(json!({
        "message": "Configuração aplicada sem derrubar os túneis ativos",
        "server": VpnServerResponse::from(server),
        "cidr": network.cidr,
        "serverAddress": server_service::server_address(&network)?.to_string(),
    }))?)
}

/// `POST /api/vpn/server/preflight`.
///
/// O corpo é opcional: sem ele, o diagnóstico usa o endpoint e a porta já
/// gravados no servidor.
async fn run_preflight(State(ctx): State<AppContext>, body: String) -> AppResult<Response> {
    let input: PreflightInput = optional_body(&body);
    let server = server_service::find(&ctx.db).await?;
    let endpoint = input
        .public_endpoint
        .or_else(|| server.as_ref().and_then(|s| s.public_endpoint.clone()));
    let port = input
        .listen_port
        .or_else(|| server.as_ref().map(|s| s.listen_port))
        .unwrap_or(DEFAULT_LISTEN_PORT);

    Ok(format::json(
        preflight::run(endpoint.as_deref(), port).await,
    )?)
}

/// `POST /api/vpn/server/detect-endpoint`.
async fn detect_endpoint() -> AppResult<Response> {
    let Some(public_ip) = preflight::detect_public_ip().await else {
        return Ok(format::json(json!({
            "detected": false,
            "publicEndpoint": null,
            "message": "Não foi possível detectar o IP público a partir deste servidor.",
        }))?);
    };
    Ok(format::json(json!({
        "detected": true,
        "publicEndpoint": public_ip.to_string(),
        "message": format!("Endereço público detectado: {public_ip}"),
    }))?)
}

async fn emit_vpn_peers_updated(ctx: &AppContext) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(&ctx.db, "vpn:peers_updated", serde_json::json!({}))
            .await
        {
            tracing::warn!(%error, "falha ao publicar vpn:peers_updated");
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/vpn/server")
        .add("/", get(show).put(update))
        .add("/preflight", post(run_preflight))
        .add("/detect-endpoint", post(detect_endpoint))
}
