//! Autenticação, verificação de origem e rate limiting do endpoint `/mcp`.

use std::{sync::OnceLock, time::Duration};

use axum::http::{header, HeaderMap};
use loco_rs::app::AppContext;

use netmonitor_core::{
    models::{mcp_tokens, users},
    roles::AreaAccess,
    shared::{
        client_addr::{self, ClientOrigin},
        errors::{AppError, AppResult},
        personal_token::bearer_token,
        rate_limit::SlidingWindowRateLimiter,
    },
};

use crate::{
    scopes::McpScope,
    settings,
    tokens::{self, allowed_application_ids_of, scopes_of},
};

/// Falhas de autenticação permitidas por IP a cada 60 s em produção.
const AUTH_FAILURE_LIMIT: usize = 20;
/// Chamadas MCP permitidas por token a cada 60 s em produção.
const TOKEN_CALL_LIMIT: usize = 120;
const RATE_WINDOW: Duration = Duration::from_secs(60);

/// Rate limiter por IP para tentativas inválidas no `/mcp`.
#[must_use]
pub fn auth_failure_limiter() -> &'static SlidingWindowRateLimiter {
    static LIMITER: OnceLock<SlidingWindowRateLimiter> = OnceLock::new();
    LIMITER
        .get_or_init(|| SlidingWindowRateLimiter::for_environment(AUTH_FAILURE_LIMIT, RATE_WINDOW))
}

/// Rate limiter por token autenticado para chamadas `/mcp`.
#[must_use]
pub fn token_call_limiter() -> &'static SlidingWindowRateLimiter {
    static LIMITER: OnceLock<SlidingWindowRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(|| SlidingWindowRateLimiter::for_environment(TOKEN_CALL_LIMIT, RATE_WINDOW))
}

/// Identidade e contexto ao vivo de uma requisição autenticada no `/mcp`.
#[derive(Debug, Clone)]
pub struct AuthenticatedMcp {
    pub token: mcp_tokens::Model,
    pub user: users::Model,
    pub access: AreaAccess,
    pub scopes: Vec<McpScope>,
    pub allowed_application_ids: Option<Vec<i64>>,
    pub origin: ClientOrigin,
}

/// Recusa o `Origin` de outro site (DNS rebinding — exigência da especificação
/// MCP). Agentes de IDE e de terminal não enviam `Origin`; um navegador envia,
/// e só vale o da própria central (o mesmo `Host`).
///
/// # Errors
///
/// `403 Forbidden` com `Origin` de outro host ou ilegível.
pub fn check_browser_origin(headers: &HeaderMap) -> AppResult<()> {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let refused = || {
        AppError::forbidden(
            "Origem do navegador recusada pelo servidor MCP: só a própria central pode chamá-lo              de uma página.",
        )
    };
    let origin_host = origin
        .to_str()
        .ok()
        .and_then(|value| value.parse::<axum::http::Uri>().ok())
        .and_then(|uri| uri.authority().map(|a| a.as_str().to_ascii_lowercase()))
        .ok_or_else(refused)?;
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::to_ascii_lowercase)
        .ok_or_else(refused)?;
    if origin_host == host {
        Ok(())
    } else {
        Err(refused())
    }
}

/// Valida configurações globais, origem de rede, rate limit, token Bearer e
/// usuário ativo para o endpoint `/mcp`.
///
/// # Errors
///
/// * `503 ServiceUnavailable` quando `mcp.enabled == false`;
/// * `403 Forbidden` quando a origem não é interna e `allow_public_origin == false`;
/// * `429 RateLimited` quando o IP ou token excede a janela deslizante;
/// * `401 Unauthorized` quando o token falta, é inválido, revogado, vencido ou
///   o dono está desativado.
pub async fn authenticate_request(
    ctx: &AppContext,
    headers: &HeaderMap,
) -> AppResult<AuthenticatedMcp> {
    check_browser_origin(headers)?;
    let config = settings::load(&ctx.db).await?;
    if !config.enabled {
        return Err(AppError::service_unavailable(
            "Servidor MCP desativado nas configurações.",
        ));
    }

    let origin = client_addr::origin(headers);
    if !origin.is_internal() && !config.allow_public_origin {
        return Err(AppError::forbidden(
            "O servidor MCP aceita apenas conexões pela rede interna ou VPN. Habilite conexões externas nas configurações se necessário.",
        ));
    }

    let ip_str = origin.ip.map(|ip| ip.to_string());
    let ip_key = format!("mcp:auth:{}", ip_str.as_deref().unwrap_or("desconhecido"));
    // Barra antes de conferir o token: depois do limite, nem o palpite certo
    // entra.
    if let Some(retry_after) = auth_failure_limiter().blocked_for(&ip_key) {
        return Err(AppError::rate_limited(
            "Muitas tentativas de autenticação no servidor MCP. Aguarde antes de tentar novamente.",
            retry_after,
        ));
    }

    let Some(raw_token) = bearer_token(headers) else {
        let decision = auth_failure_limiter().consume(&ip_key);
        if !decision.allowed {
            return Err(AppError::rate_limited(
                "Muitas tentativas de autenticação no servidor MCP. Aguarde antes de tentar novamente.",
                decision.retry_after_seconds,
            ));
        }
        return Err(AppError::unauthorized(
            "Informe um token MCP válido no cabeçalho Authorization: Bearer nmm_...",
        ));
    };

    let Some((token, user)) = tokens::authenticate(&ctx.db, raw_token, ip_str).await? else {
        let decision = auth_failure_limiter().consume(&ip_key);
        if !decision.allowed {
            return Err(AppError::rate_limited(
                "Muitas tentativas de autenticação no servidor MCP. Aguarde antes de tentar novamente.",
                decision.retry_after_seconds,
            ));
        }
        return Err(AppError::unauthorized(
            "Token MCP inválido, revogado, vencido ou usuário desativado.",
        ));
    };

    let call_decision = token_call_limiter().consume(&format!("mcp:token:{}", token.id));
    if !call_decision.allowed {
        return Err(AppError::rate_limited(
            "Limite de chamadas MCP excedido para este token. Tente novamente em instantes.",
            call_decision.retry_after_seconds,
        ));
    }

    let access = AreaAccess::of_user(&user)?;
    let scopes = scopes_of(&token);
    let allowed_application_ids = allowed_application_ids_of(&token);

    Ok(AuthenticatedMcp {
        token,
        user,
        access,
        scopes,
        allowed_application_ids,
        origin,
    })
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(header::HeaderName, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(name.clone(), HeaderValue::from_str(value).expect("valor"));
        }
        map
    }

    #[test]
    fn sem_origin_passa() {
        assert!(check_browser_origin(&headers(&[(header::HOST, "10.0.0.5:3333")])).is_ok());
    }

    #[test]
    fn origin_da_propria_central_passa() {
        let map = headers(&[
            (header::HOST, "10.0.0.5:3333"),
            (header::ORIGIN, "http://10.0.0.5:3333"),
        ]);
        assert!(check_browser_origin(&map).is_ok());
    }

    #[test]
    fn origin_de_outro_site_e_recusado() {
        for origin in [
            "http://evil.example",
            "http://10.0.0.5:9999",
            "null",
            "lixo",
        ] {
            let map = headers(&[(header::HOST, "10.0.0.5:3333"), (header::ORIGIN, origin)]);
            assert!(check_browser_origin(&map).is_err(), "{origin}");
        }
    }
}
