//! Camada única de autenticação para as rotas de negócio.
//!
//! A validação usa o extractor JWT nativo do Loco, inclusive a cadeia de locais
//! configurada em `auth.jwt.location` (Bearer e `?token=` para SSE).

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use loco_rs::{app::AppContext, controller::extractor::auth};

pub async fn require_jwt(State(ctx): State<AppContext>, request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    if let Err(error) = auth::extract_jwt_from_request_parts(&parts, &ctx) {
        tracing::warn!(%error, path = %parts.uri.path(), "requisição de negócio sem JWT válido");
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "message": "Não autenticado" })),
        )
            .into_response();
    }
    next.run(Request::from_parts(parts, body)).await
}
