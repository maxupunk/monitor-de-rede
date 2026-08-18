//! Camada única de autenticação para as rotas de negócio.
//!
//! A validação usa o extractor JWT nativo do Loco, inclusive a cadeia de locais
//! configurada em `auth.jwt.location` (Bearer e `?token=` para SSE).

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use loco_rs::{app::AppContext, controller::extractor::auth};

/// Cabeçalho interno com a identidade de quem passou pelo guarda.
///
/// Existe porque a auditoria e o rate limit da VPN (§7.13) precisam saber
/// *quem* pediu, e o guarda é o único ponto que já validou o token. Escrever no
/// próprio `Request` (em vez de uma extensão tipada) mantém os handlers com
/// extratores infalíveis: um `HeaderMap` nunca falha, uma `Extension` ausente
/// vira 500.
///
/// É um cabeçalho de **requisição**, escrito por nós depois da validação —
/// nada que o cliente mande com esse nome sobrevive, porque a linha abaixo o
/// sobrescreve.
pub const AUTHENTICATED_USER_HEADER: &str = "x-authenticated-user";

pub async fn require_jwt(
    State(ctx): State<AppContext>,
    mut request: Request,
    next: Next,
) -> Response {
    let (mut parts, body) = request.into_parts();
    let claims = match auth::extract_jwt_from_request_parts(&parts, &ctx) {
        Ok(claims) => claims,
        Err(error) => {
            tracing::warn!(%error, path = %parts.uri.path(), "requisição de negócio sem JWT válido");
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "message": "Não autenticado" })),
            )
                .into_response();
        }
    };

    let Ok(user) = crate::models::users::Model::find_by_pid(&ctx.db, &claims.claims.pid).await
    else {
        tracing::warn!(pid = %claims.claims.pid, path = %parts.uri.path(), "usuário do JWT não existe no banco");
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "message": "Não autenticado" })),
        )
            .into_response();
    };

    if !user.active {
        tracing::info!(user_pid = %user.pid, path = %parts.uri.path(), "requisição de negócio recusada: usuário desativado");
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "message": "Este usuário está desativado. Procure um administrador." })),
        )
            .into_response();
    }

    parts.headers.remove(AUTHENTICATED_USER_HEADER);
    if let Ok(value) = HeaderValue::from_str(&claims.claims.pid) {
        parts.headers.insert(AUTHENTICATED_USER_HEADER, value);
    }

    request = Request::from_parts(parts, body);
    next.run(request).await
}
