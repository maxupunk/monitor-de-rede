//! Peers da VPN (§7.13): listagem com telemetria, wizard de criação, artefatos
//! de configuração, rotação de chaves e revogação.

use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;
use qrcode::{render::svg, QrCode};
use serde::Deserialize;
use serde_json::json;

use crate::{
    controllers::{
        auth_guard::AUTHENTICATED_USER_HEADER, devices::present_for_vpn as present_device,
    },
    services::{
        shared::errors::{AppError, AppResult},
        vpn::{
            access_control::{audit, sensitive_endpoint_limiter, VpnAuditAction, VpnAuditEntry},
            ip_allocator,
            peer_service::{self, CreatePeerPayload},
            server_service, GeneratedArtifact, PRIVATE_KEY_UNAVAILABLE,
        },
    },
    views::vpn::{SerializedVpnArtifact, VpnPeerListItem, VpnPeerResponse, VpnPeerWithDevice},
};

/// Lado **mínimo** do QR Code em pixels — o mesmo alvo do backend anterior,
/// para o código continuar legível na mesma distância de câmera. A margem
/// (quiet zone) é a padrão do formato: sem ela, leitores recusam o código.
const QR_SIZE: u32 = 320;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePeerInput {
    name: Option<String>,
    profile: Option<String>,
    ip_address: Option<String>,
    site_id: Option<i64>,
    snmp_enabled: Option<bool>,
    snmp_community: Option<String>,
    snmp_version: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenamePeerInput {
    name: Option<String>,
}

/// Identidade usada em rate limit e auditoria dos endpoints sensíveis.
///
/// Prefere o usuário autenticado (o guarda JWT grava o `pid` no cabeçalho
/// interno); cai no IP declarado pelo proxy. O prefixo evita que um IP e um id
/// colidam na mesma chave de janela.
fn requester_id(headers: &HeaderMap) -> String {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };

    if let Some(user) = header(AUTHENTICATED_USER_HEADER) {
        let safe_user: String = user
            .chars()
            .filter(|c| !c.is_control() && !c.is_whitespace())
            .collect();
        if !safe_user.is_empty() {
            return format!("user:{safe_user}");
        }
    }

    let ip = crate::services::vpn::access_control::extract_client_ip(headers);
    format!("ip:{ip}")
}

/// Aplica a janela deslizante de 10 req/60 s, devolvendo 429 + `Retry-After`.
fn enforce_rate_limit(scope: &str, requester: &str) -> AppResult<()> {
    let decision = sensitive_endpoint_limiter().consume(&format!("{scope}:{requester}"));
    if decision.allowed {
        return Ok(());
    }
    Err(AppError::rate_limited(
        format!(
            "Muitas solicitações de configuração. Tente novamente em {}s.",
            decision.retry_after_seconds
        ),
        decision.retry_after_seconds,
    ))
}

/// Renderiza o QR Code **junto** do artefato.
///
/// Precisa acontecer na mesma resposta: a chave privada só existe até a
/// primeira leitura, então buscar o QR Code numa segunda requisição devolveria
/// um código com o placeholder no lugar da chave — e um celular que lesse esse
/// código montaria um túnel que nunca conecta.
fn serialize_artifact(artifact: GeneratedArtifact) -> SerializedVpnArtifact {
    let usable = artifact.supports_qr_code && !artifact.content.contains(PRIVATE_KEY_UNAVAILABLE);
    let qr_svg = usable.then(|| render_qr(&artifact.content)).flatten();
    SerializedVpnArtifact { artifact, qr_svg }
}

fn render_qr(content: &str) -> Option<String> {
    let code = QrCode::new(content.as_bytes()).ok()?;
    Some(
        code.render::<svg::Color<'_>>()
            .quiet_zone(true)
            // `min_dimensions` é piso, não medida exata: o renderizador ajusta
            // para que todo módulo tenha o mesmo tamanho inteiro em pixels.
            // Forçar 320 exatos distorceria o código e o leitor recusaria.
            .min_dimensions(QR_SIZE, QR_SIZE)
            .build(),
    )
}

fn audit_entry(
    action: VpnAuditAction,
    peer_id: Option<i64>,
    requester: &str,
    details: serde_json::Value,
) {
    let (user_id, ip_address) = requester
        .split_once(':')
        .map_or((None, None), |(kind, value)| {
            if kind == "user" {
                (Some(value.to_string()), None)
            } else {
                (None, Some(value.to_string()))
            }
        });
    audit(&VpnAuditEntry {
        action,
        peer_id,
        user_id,
        ip_address,
        details,
    });
}

/// `GET /api/vpn/peers`.
async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let items = peer_service::list(&ctx.db).await?;
    let mut payload = Vec::with_capacity(items.len());
    for item in items {
        let device = match item.device {
            Some(device) => Some(present_device(&ctx.db, device).await?),
            None => None,
        };
        payload.push(VpnPeerListItem {
            peer: VpnPeerResponse::from(&item.peer),
            hints: item.hints,
            device,
        });
    }
    Ok(format::json(payload)?)
}

/// `GET /api/vpn/peers/next-ip` — sugestão de IP livre para o wizard.
async fn next_ip(State(ctx): State<AppContext>) -> AppResult<Response> {
    let server = server_service::find(&ctx.db)
        .await?
        .ok_or_else(|| AppError::business_rule("Servidor VPN ainda não foi configurado"))?;
    let network = server_service::network_of(&ctx.db, &server).await?;
    let ip_address =
        ip_allocator::find_next_free(&ctx.db, server.network_id, &network.cidr, &[]).await?;
    Ok(format::json(json!({
        "ipAddress": ip_address.to_string(),
        "cidr": network.cidr,
    }))?)
}

/// `POST /api/vpn/peers` — cria peer, aloca IP e provisiona device + monitores.
async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<CreatePeerInput>,
) -> AppResult<Response> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let profile = input
        .profile
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let (Some(name), Some(profile)) = (name, profile) else {
        return Err(AppError::business_rule(
            "Informe o nome do dispositivo e o perfil do equipamento",
        ));
    };

    let (peer, artifact) = peer_service::create(
        &ctx.db,
        &CreatePeerPayload {
            name: name.to_string(),
            profile: profile.to_string(),
            ip_address: input.ip_address,
            site_id: input.site_id,
            snmp_enabled: input.snmp_enabled.unwrap_or(false),
            snmp_community: input.snmp_community,
            snmp_version: input.snmp_version,
            description: input.description,
        },
    )
    .await?;

    let requester = requester_id(&headers);
    audit_entry(
        VpnAuditAction::PeerCreated,
        Some(peer.id),
        &requester,
        json!({ "profile": peer.device_profile }),
    );

    let device = match crate::models::devices::Entity::find_by_id(peer.device_id)
        .one(&ctx.db)
        .await?
    {
        Some(device) => present_device(&ctx.db, device).await?,
        None => serde_json::Value::Null,
    };
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "peer": VpnPeerResponse::from(&peer),
            "device": device,
            "artifact": serialize_artifact(artifact),
        })),
    )
        .into_response())
}

/// `PATCH /api/vpn/peers/:id` — renomeia o dispositivo do peer.
async fn update(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(input): Json<RenamePeerInput>,
) -> AppResult<Response> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::business_rule("Informe o nome do dispositivo"))?;

    let (peer, device) = peer_service::rename(&ctx.db, id, name).await?;
    let device = match device {
        Some(device) => Some(present_device(&ctx.db, device).await?),
        None => None,
    };
    Ok(format::json(VpnPeerWithDevice {
        peer: VpnPeerResponse::from(&peer),
        device,
    })?)
}

/// `GET /api/vpn/peers/:id/config` — 🔒 credencial de acesso à rede.
async fn config(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let requester = requester_id(&headers);
    enforce_rate_limit("config", &requester)?;

    let artifact = peer_service::build_artifact(&ctx.db, id).await?;
    audit_entry(
        VpnAuditAction::ConfigDownload,
        Some(id),
        &requester,
        json!({ "profile": artifact.profile }),
    );
    Ok(format::json(serialize_artifact(artifact))?)
}

/// `GET /api/vpn/peers/:id/qrcode` — 🔒 apenas perfis móveis.
async fn qrcode(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let requester = requester_id(&headers);
    enforce_rate_limit("qrcode", &requester)?;

    let artifact = peer_service::build_artifact(&ctx.db, id).await?;
    if !artifact.supports_qr_code {
        return Err(AppError::business_rule(format!(
            "O perfil {} não utiliza QR Code — copie o script gerado.",
            artifact.label
        )));
    }

    let profile = artifact.profile.clone();
    let file_name = artifact.file_name.clone();
    let serialized = serialize_artifact(artifact);
    let Some(svg) = serialized.qr_svg else {
        // Matriz de paridade #34: sem a chave, o QR Code levaria o celular a um
        // túnel que nunca conecta — 409 e a instrução de rotacionar.
        return Err(AppError::conflict(
            "A chave privada deste dispositivo já foi entregue. Rotacione as chaves para gerar um novo QR Code.",
        ));
    };

    audit_entry(
        VpnAuditAction::QrcodeDownload,
        Some(id),
        &requester,
        json!({ "profile": profile }),
    );
    Ok(format::json(json!({
        "profile": profile,
        "fileName": file_name,
        "svg": svg,
    }))?)
}

/// `POST /api/vpn/peers/:id/rotate` — 🔒.
async fn rotate(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let requester = requester_id(&headers);
    enforce_rate_limit("rotate", &requester)?;

    let (peer, artifact) = peer_service::rotate_keys(&ctx.db, id).await?;
    audit_entry(
        VpnAuditAction::KeyRotation,
        Some(peer.id),
        &requester,
        json!({ "profile": peer.device_profile }),
    );
    Ok(format::json(json!({
        "message": "Novo par de chaves gerado. A configuração anterior foi invalidada.",
        "peer": VpnPeerResponse::from(&peer),
        "artifact": serialize_artifact(artifact),
    }))?)
}

/// `POST /api/vpn/peers/:id/firewall-hints` — diagnóstico do erro nº 1.
async fn firewall_hints(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let (profile, label, content) = peer_service::firewall_hints(&ctx.db, id).await?;
    Ok(format::json(json!({
        "profile": profile,
        "label": label,
        "content": content,
        "message": "Túnel conectado, mas o dispositivo não responde a ping? Copie as regras abaixo e aplique no equipamento.",
    }))?)
}

/// `DELETE /api/vpn/peers/:id` — revoga e libera o IP.
async fn destroy(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> AppResult<Response> {
    peer_service::revoke(&ctx.db, id).await?;
    audit_entry(
        VpnAuditAction::PeerRevoked,
        Some(id),
        &requester_id(&headers),
        json!({}),
    );
    Ok(format::json(json!({
        "message": "Peer revogado. O acesso foi cortado imediatamente.",
    }))?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/vpn/peers")
        // `/next-ip` é estático e casa antes de `/{id}`; declarado primeiro
        // para que uma reordenação futura não o transforme num id inválido.
        .add("/next-ip", get(next_ip))
        .add("/", get(index).post(store))
        .add("/{id}", patch(update).delete(destroy))
        .add("/{id}/config", get(config))
        .add("/{id}/qrcode", get(qrcode))
        .add("/{id}/rotate", post(rotate))
        .add("/{id}/firewall-hints", post(firewall_hints))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_qr_code_sai_como_svg_utilizavel() {
        let svg = render_qr("[Interface]\nPrivateKey = ABC\n").expect("QR gerado");
        assert!(svg.starts_with("<?xml"));
        assert!(svg.contains("<svg"));
        // O lado é o menor múltiplo do módulo que alcança `QR_SIZE`.
        let width = svg
            .split("width=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse::<u32>().ok())
            .expect("largura no SVG");
        assert!(width >= QR_SIZE, "QR de {width} px é pequeno demais");
    }

    #[test]
    fn conteudo_grande_demais_nao_derruba_a_resposta() {
        // Um QR Code tem teto de capacidade; acima dele a resposta precisa vir
        // sem `qrSvg`, e não com um 500.
        assert!(render_qr(&"A".repeat(10_000)).is_none());
    }

    #[test]
    fn o_identificador_prefere_o_usuario_e_cai_no_ip() {
        let mut headers = HeaderMap::new();
        assert_eq!(requester_id(&headers), "ip:desconhecido");

        headers.insert("x-real-ip", "203.0.113.9".parse().unwrap());
        assert_eq!(requester_id(&headers), "ip:203.0.113.9");

        // `X-Forwarded-For` com cadeia: o cliente é o primeiro da lista.
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        assert_eq!(requester_id(&headers), "ip:203.0.113.7");

        headers.insert(AUTHENTICATED_USER_HEADER, "pid-7".parse().unwrap());
        assert_eq!(requester_id(&headers), "user:pid-7");
    }

    #[test]
    #[serial_test::serial(vpn_rate_limiter)]
    fn o_rate_limit_devolve_429_com_retry_after() {
        sensitive_endpoint_limiter().reset();
        for _ in 0..10 {
            assert!(enforce_rate_limit("teste", "user:99").is_ok());
        }
        let error = enforce_rate_limit("teste", "user:99").expect_err("11ª chamada é barrada");
        assert_eq!(error.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(error.to_string().contains("Tente novamente em"));
        sensitive_endpoint_limiter().reset();
    }

    #[test]
    #[serial_test::serial(vpn_rate_limiter)]
    fn o_escopo_do_rate_limit_e_por_endpoint() {
        sensitive_endpoint_limiter().reset();
        for _ in 0..10 {
            assert!(enforce_rate_limit("config", "user:1").is_ok());
        }
        assert!(enforce_rate_limit("config", "user:1").is_err());
        // Baixar a config não pode barrar a rotação de chaves.
        assert!(enforce_rate_limit("rotate", "user:1").is_ok());
        sensitive_endpoint_limiter().reset();
    }
}
