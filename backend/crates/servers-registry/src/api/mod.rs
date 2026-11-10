//! A OCI Distribution Spec em `/v2` (RF-01..03).
//!
//! Fica na raiz do host (o Docker exige `/v2/`), fora do prefixo `/api` e
//! fora da sessão do painel: quem autentica é o token do registry
//! ([`crate::auth`]). O nome do repositório tem barras, então a rota é um
//! curinga e o caminho é lido aqui ([`Route`]).

mod blobs;
mod errors;
mod manifests;
mod uploads;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Json, Router,
};
use loco_rs::app::AppContext;
use serde_json::json;

use netmonitor_core::shared::client_addr;

use crate::{
    auth::{self, Access, Refusal},
    catalog,
    text::percent_decode,
    tokens::Action,
};
pub use errors::{Code, OciError};

pub type OciResult<T> = Result<T, OciError>;

/// O que o caminho depois de `/v2/` pede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    UploadStart { name: String },
    Upload { name: String, id: String },
    Blob { name: String, digest: String },
    Manifest { name: String, reference: String },
    Tags { name: String },
    Referrers { name: String, digest: String },
}

impl Route {
    /// Lê o caminho (`loja/api/blobs/sha256:…`).
    #[must_use]
    pub fn parse(path: &str) -> Option<Self> {
        let path = path.trim_start_matches('/');
        if let Some(name) = path
            .strip_suffix("/blobs/uploads/")
            .or_else(|| path.strip_suffix("/blobs/uploads"))
        {
            return Some(Self::UploadStart { name: name.into() });
        }
        if let Some(at) = path.rfind("/blobs/uploads/") {
            return Some(Self::Upload {
                name: path[..at].into(),
                id: path[at + "/blobs/uploads/".len()..].into(),
            });
        }
        if let Some(name) = path.strip_suffix("/tags/list") {
            return Some(Self::Tags { name: name.into() });
        }
        for (marker, build) in [("/manifests/", 0_u8), ("/blobs/", 1), ("/referrers/", 2)] {
            if let Some(at) = path.rfind(marker) {
                let (name, rest) = (
                    path[..at].to_string(),
                    path[at + marker.len()..].to_string(),
                );
                if rest.is_empty() || rest.contains('/') {
                    return None;
                }
                return Some(match build {
                    0 => Self::Manifest {
                        name,
                        reference: rest,
                    },
                    1 => Self::Blob { name, digest: rest },
                    _ => Self::Referrers { name, digest: rest },
                });
            }
        }
        None
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::UploadStart { name }
            | Self::Upload { name, .. }
            | Self::Blob { name, .. }
            | Self::Manifest { name, .. }
            | Self::Tags { name }
            | Self::Referrers { name, .. } => name,
        }
    }
}

/// A ação que o método pede.
fn action_of(method: &Method) -> Action {
    match *method {
        Method::GET | Method::HEAD => Action::Pull,
        Method::DELETE => Action::Delete,
        _ => Action::Push,
    }
}

/// `realm` do desafio: o endereço por onde o cliente chegou.
fn realm(headers: &HeaderMap) -> String {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost");
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("http");
    format!("{scheme}://{host}/v2/token")
}

/// `401` com o desafio do token, para o cliente pedir o escopo certo.
fn challenge(headers: &HeaderMap, scope: Option<String>) -> OciError {
    let mut value = format!(
        "Bearer realm=\"{}\",service=\"{}\"",
        realm(headers),
        auth::SERVICE
    );
    if let Some(scope) = scope {
        value.push_str(&format!(",scope=\"{scope}\""));
    }
    OciError::new(
        StatusCode::UNAUTHORIZED,
        Code::Unauthorized,
        "autenticação necessária",
    )
    .with_header(header::WWW_AUTHENTICATE, value)
}

/// Quem chama pode `action` em `name`? Pela borda pública, não (RS-04).
pub(crate) fn authorize(headers: &HeaderMap, name: &str, action: Action) -> OciResult<String> {
    if client_addr::origin(headers).public {
        return Err(OciError::new(
            StatusCode::FORBIDDEN,
            Code::Denied,
            Refusal::PublicOrigin.message(),
        ));
    }
    let scope = format!(
        "repository:{name}:{}",
        match action {
            Action::Pull => "pull",
            Action::Push => "pull,push",
            Action::Delete => "delete",
        }
    );
    let Some(claims) = auth::bearer(headers) else {
        return Err(challenge(headers, Some(scope)));
    };
    if claims
        .access
        .iter()
        .any(|access| access.allows(name, action))
    {
        return Ok(claims.sub);
    }
    if claims.access.iter().any(|access| access.name == name) {
        return Err(OciError::new(
            StatusCode::FORBIDDEN,
            Code::Denied,
            format!("Seu token não permite '{}' em {name}", action.as_str()),
        ));
    }
    Err(challenge(headers, Some(scope)))
}

async fn base(headers: HeaderMap) -> Response {
    if client_addr::origin(&headers).public {
        return OciError::new(
            StatusCode::FORBIDDEN,
            Code::Denied,
            Refusal::PublicOrigin.message(),
        )
        .into_response();
    }
    match auth::bearer(&headers) {
        Some(_) => (
            [(
                header::HeaderName::from_static("docker-distribution-api-version"),
                HeaderValue::from_static("registry/2.0"),
            )],
            Json(json!({})),
        )
            .into_response(),
        None => challenge(&headers, None).into_response(),
    }
}

/// `GET /v2/token` — o Docker pode mandar vários `scope`; lidos da query crua.
async fn token(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    let principal = match auth::login(&ctx.db, &headers).await {
        Ok(principal) => principal,
        Err(refusal) => {
            let status = if refusal == Refusal::PublicOrigin {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::UNAUTHORIZED
            };
            return OciError::new(status, Code::Unauthorized, refusal.message()).into_response();
        }
    };
    let requested: Vec<Access> = uri
        .query()
        .unwrap_or_default()
        .split('&')
        .filter_map(|pair| pair.strip_prefix("scope="))
        .flat_map(|raw| {
            let decoded = percent_decode(&raw.replace('+', " "));
            decoded
                .split(' ')
                .filter_map(Access::parse)
                .collect::<Vec<_>>()
        })
        .collect();
    let granted = auth::grant(
        &requested,
        &principal.token_scope,
        auth::servers_role(&principal.user),
    );
    match auth::issue(&principal.user.name, granted) {
        Ok(token) => Json(json!({
            "token": token,
            "access_token": token,
            "expires_in": 300,
            "issued_at": chrono::Utc::now().to_rfc3339(),
        }))
        .into_response(),
        Err(_) => OciError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Code::Unknown,
            "falha ao emitir",
        )
        .into_response(),
    }
}

/// Um parâmetro da query, já decodificado (`sha256%3A…` → `sha256:…`).
pub(crate) fn query_value(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
        .map(|raw| percent_decode(&raw.replace('+', " ")))
}

async fn dispatch(
    State(ctx): State<AppContext>,
    method: Method,
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<String>,
    uri: axum::http::Uri,
    body: Body,
) -> Response {
    let Some(route) = Route::parse(&path) else {
        return OciError::new(
            StatusCode::NOT_FOUND,
            Code::Unsupported,
            "rota desconhecida",
        )
        .into_response();
    };
    if !crate::names::valid_name(route.name()) {
        return OciError::new(
            StatusCode::BAD_REQUEST,
            Code::NameInvalid,
            "nome de repositório inválido",
        )
        .into_response();
    }
    let who = match authorize(&headers, route.name(), action_of(&method)) {
        Ok(who) => who,
        Err(error) => return error.into_response(),
    };
    let query = uri.query().unwrap_or_default().to_string();
    let result = match (&route, &method) {
        (Route::UploadStart { name }, &Method::POST) => {
            uploads::start(&ctx, &headers, name, &query, body).await
        }
        (Route::Upload { name, id }, &Method::PATCH) => {
            uploads::patch(&ctx, &headers, name, id, body).await
        }
        (Route::Upload { name, id }, &Method::PUT) => {
            uploads::finish(&ctx, name, id, &query, body).await
        }
        (Route::Upload { name, id }, &Method::GET) => uploads::status(&ctx, name, id).await,
        (Route::Upload { name, id }, &Method::DELETE) => uploads::cancel(&ctx, name, id).await,
        (Route::Blob { name, digest }, &Method::GET | &Method::HEAD) => {
            blobs::get(&ctx, &headers, name, digest, method == Method::HEAD).await
        }
        (Route::Blob { name, digest }, &Method::DELETE) => blobs::delete(&ctx, name, digest).await,
        (Route::Manifest { name, reference }, &Method::GET | &Method::HEAD) => {
            manifests::get(&ctx, name, reference, method == Method::HEAD).await
        }
        (Route::Manifest { name, reference }, &Method::PUT) => {
            manifests::put(&ctx, &headers, name, reference, body, &who).await
        }
        (Route::Manifest { name, reference }, &Method::DELETE) => {
            manifests::delete(&ctx, name, reference).await
        }
        (Route::Tags { name }, &Method::GET) => manifests::tags(&ctx, name, &query).await,
        (Route::Referrers { name, digest }, &Method::GET) => {
            manifests::referrers(&ctx, name, digest, &query).await
        }
        _ => Err(OciError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            Code::Unsupported,
            "método não suportado nesta rota",
        )),
    };
    result.unwrap_or_else(IntoResponse::into_response)
}

/// O repositório existente, ou `NAME_UNKNOWN`.
pub(crate) async fn existing(
    ctx: &AppContext,
    name: &str,
) -> OciResult<netmonitor_core::models::registry_repositories::Model> {
    catalog::repository(&ctx.db, name)
        .await
        .map_err(OciError::internal)?
        .ok_or_else(|| {
            OciError::new(
                StatusCode::NOT_FOUND,
                Code::NameUnknown,
                "repositório desconhecido",
            )
        })
}

/// As rotas da raiz (`/v2`), montadas antes do prefixo `/api`.
pub fn routes() -> Router<AppContext> {
    Router::new()
        .route("/v2", get(base))
        .route("/v2/", get(base))
        .route("/v2/token", get(token))
        .route("/v2/{*path}", any(dispatch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminhos_da_especificacao() {
        let digest = format!("sha256:{}", "a".repeat(64));
        assert_eq!(
            Route::parse("loja/api/blobs/uploads/"),
            Some(Route::UploadStart {
                name: "loja/api".into()
            })
        );
        assert_eq!(
            Route::parse("loja/api/blobs/uploads/abc"),
            Some(Route::Upload {
                name: "loja/api".into(),
                id: "abc".into()
            })
        );
        assert_eq!(
            Route::parse(&format!("loja/api/blobs/{digest}")),
            Some(Route::Blob {
                name: "loja/api".into(),
                digest: digest.clone()
            })
        );
        assert_eq!(
            Route::parse("a/b/c/manifests/latest"),
            Some(Route::Manifest {
                name: "a/b/c".into(),
                reference: "latest".into()
            })
        );
        assert_eq!(
            Route::parse("loja/api/tags/list"),
            Some(Route::Tags {
                name: "loja/api".into()
            })
        );
        assert_eq!(
            Route::parse(&format!("loja/api/referrers/{digest}")),
            Some(Route::Referrers {
                name: "loja/api".into(),
                digest
            })
        );
        assert_eq!(Route::parse("loja/api/coisa"), None);
    }

    #[test]
    fn escopo_decodificado() {
        assert_eq!(
            percent_decode("repository%3Aloja%2Fapi%3Apull%2Cpush"),
            "repository:loja/api:pull,push"
        );
    }
}
