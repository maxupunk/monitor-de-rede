//! Autenticação do registry: o fluxo de token do Docker (RS-03/04).
//!
//! 1. O cliente bate em `/v2/…` sem credencial e recebe `401` com
//!    `WWW-Authenticate: Bearer realm="…/v2/token",service=…,scope=…`;
//! 2. pede o token em `GET /v2/token` com Basic (usuário + token pessoal);
//! 3. usa o JWT de 5 minutos, com escopo mínimo, em cada chamada.
//!
//! O token pessoal só é aceito de origem interna (rede privada ou VPN): pela
//! borda pública ele é recusado com o motivo. A chave do JWT é derivada da
//! `ENCRYPTION_KEY` com um rótulo próprio — o token do registry não abre a
//! API do painel, nem o contrário (`aud` diferente, chave diferente).

use axum::http::HeaderMap;
use base64::Engine;
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use netmonitor_core::{
    areas::Area,
    models::users,
    roles::{AreaAccess, Role},
    shared::{client_addr, crypto::encryption_key, errors::AppResult},
};

use crate::tokens::{self, Action, Scope};

pub const SERVICE: &str = "netmonitor-registry";
const ISSUER: &str = "netmonitor";
const TOKEN_TTL_SECS: i64 = 300;

/// Uma permissão pedida ou concedida (`repository:loja/api:pull,push`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Access {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub actions: Vec<String>,
}

impl Access {
    /// Lê o `scope` da especificação.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let mut parts = raw.splitn(3, ':');
        let kind = parts.next()?;
        let name = parts.next()?;
        let actions = parts.next()?;
        (kind == "repository").then(|| Self {
            kind: kind.to_string(),
            name: name.to_string(),
            actions: actions
                .split(',')
                .filter_map(Action::parse)
                .map(|action| action.as_str().to_string())
                .collect(),
        })
    }

    #[must_use]
    pub fn allows(&self, repository: &str, action: Action) -> bool {
        self.kind == "repository"
            && self.name == repository
            && self
                .actions
                .iter()
                .any(|granted| granted == action.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub access: Vec<Access>,
}

fn signing_key() -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"netmonitor-registry-jwt\0");
    hasher.update(encryption_key());
    hasher.finalize().to_vec()
}

/// Quem pediu o token.
pub struct Principal {
    pub user: users::Model,
    pub token_scope: Scope,
}

/// As ações que o papel do usuário em Servidores permite (ADR 019).
#[must_use]
pub fn role_actions(role: Option<Role>) -> Vec<Action> {
    match role {
        Some(Role::Admin) => vec![Action::Pull, Action::Push, Action::Delete],
        Some(Role::Operator) => vec![Action::Pull, Action::Push],
        Some(Role::Viewer) => vec![Action::Pull],
        None => Vec::new(),
    }
}

/// O que conceder de cada pedido: o pedido ∩ o escopo do token ∩ o papel.
#[must_use]
pub fn grant(requested: &[Access], scope: &Scope, role: Option<Role>) -> Vec<Access> {
    let allowed = role_actions(role);
    requested
        .iter()
        .filter(|access| scope.covers(&access.name))
        .map(|access| Access {
            kind: access.kind.clone(),
            name: access.name.clone(),
            actions: access
                .actions
                .iter()
                .filter_map(|action| Action::parse(action))
                .filter(|action| scope.actions.contains(action) && allowed.contains(action))
                .map(|action| action.as_str().to_string())
                .collect(),
        })
        .filter(|access| !access.actions.is_empty())
        .collect()
}

/// Emite o JWT.
///
/// # Errors
///
/// Falha ao assinar.
pub fn issue(subject: &str, access: Vec<Access>) -> AppResult<String> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        iss: ISSUER.into(),
        sub: subject.into(),
        aud: SERVICE.into(),
        exp: now + TOKEN_TTL_SECS,
        iat: now,
        access,
    };
    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(&signing_key()),
    )
    .map_err(|error| netmonitor_core::shared::errors::AppError::Internal(error.into()))
}

/// Confere um JWT do registry.
#[must_use]
pub fn verify(token: &str) -> Option<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&[SERVICE]);
    validation.set_issuer(&[ISSUER]);
    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(&signing_key()),
        &validation,
    )
    .ok()
    .map(|data| data.claims)
}

/// `Basic usuário:segredo`.
#[must_use]
pub fn basic(headers: &HeaderMap) -> Option<(String, String)> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Basic ")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw.trim())
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let (user, secret) = text.split_once(':')?;
    Some((user.to_string(), secret.to_string()))
}

/// `Bearer …`.
#[must_use]
pub fn bearer(headers: &HeaderMap) -> Option<Claims> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    verify(raw.trim())
}

/// Por que o login foi recusado (a mensagem vai ao `docker login`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Missing,
    Invalid,
    PublicOrigin,
}

impl Refusal {
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::Missing => "Informe o usuário e um token de acesso do registry (Servidores › Registry › Tokens)",
            Self::Invalid => "Token do registry inválido, vencido ou revogado",
            Self::PublicOrigin => {
                "Token pessoal só é aceito pela rede interna ou pela VPN do NetMonitor"
            }
        }
    }
}

/// Confere o Basic de `/v2/token`: origem interna, token válido, dono ativo.
///
/// # Errors
///
/// [`Refusal`] com o motivo.
pub async fn login(db: &DatabaseConnection, headers: &HeaderMap) -> Result<Principal, Refusal> {
    let (_, secret) = basic(headers).ok_or(Refusal::Missing)?;
    let origin = client_addr::origin(headers);
    if !origin.is_internal() {
        return Err(Refusal::PublicOrigin);
    }
    let token = tokens::authenticate(db, &secret, origin.ip.map(|ip| ip.to_string()))
        .await
        .ok()
        .flatten()
        .ok_or(Refusal::Invalid)?;
    let user = users::Entity::find_by_id(token.user_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .ok_or(Refusal::Invalid)?;
    Ok(Principal {
        token_scope: tokens::scope_of(&token),
        user,
    })
}

/// O papel do usuário na área Servidores.
#[must_use]
pub fn servers_role(user: &users::Model) -> Option<Role> {
    AreaAccess::of_user(user)
        .ok()
        .and_then(|access| access.role_in(Area::Servers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concede_so_a_intersecao() {
        let requested = vec![
            Access::parse("repository:loja/api:pull,push,delete").unwrap(),
            Access::parse("repository:outra/api:pull").unwrap(),
        ];
        let scope = Scope {
            repositories: vec!["loja/*".into()],
            actions: vec![Action::Pull, Action::Push],
        };
        let operator = grant(&requested, &scope, Some(Role::Operator));
        assert_eq!(operator.len(), 1);
        assert_eq!(operator[0].actions, vec!["pull", "push"]);
        let viewer = grant(&requested, &scope, Some(Role::Viewer));
        assert_eq!(viewer[0].actions, vec!["pull"]);
        assert!(grant(&requested, &scope, None).is_empty());
    }

    #[test]
    fn jwt_ida_e_volta_e_escopo() {
        let access = vec![Access::parse("repository:loja/api:pull").unwrap()];
        let token = issue("ana", access).unwrap();
        let claims = verify(&token).unwrap();
        assert!(claims.access[0].allows("loja/api", Action::Pull));
        assert!(!claims.access[0].allows("loja/api", Action::Push));
        assert!(!claims.access[0].allows("loja/web", Action::Pull));
        assert!(verify("lixo").is_none());
    }

    #[test]
    fn basic_e_escopo_invalido() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Basic YW5hOm5tcl94".parse().unwrap(),
        );
        assert_eq!(basic(&headers), Some(("ana".into(), "nmr_x".into())));
        assert!(Access::parse("registry:catalog:*").is_none());
    }
}
