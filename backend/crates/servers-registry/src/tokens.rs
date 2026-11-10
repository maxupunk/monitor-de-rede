//! Tokens de acesso pessoal do registry (RF-12, RS-02).
//!
//! `nmr_` + 32 bytes aleatórios: aparece uma vez, na criação. O banco guarda
//! só o SHA-256 (o token tem entropia de sobra; Argon2 não acrescenta nada e
//! custaria a cada login). Validade é obrigatória, o escopo é o mínimo que o
//! usuário marcou, e o que vale na hora é a interseção com o papel dele em
//! Servidores (ADR 019) — rebaixado o papel, o token encolhe junto.

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use netmonitor_core::{
    models::registry_tokens,
    shared::{
        crypto::sha256_hex,
        errors::{AppError, AppResult},
    },
};

pub const PREFIX: &str = "nmr_";
/// Validade máxima (dias).
pub const MAX_DAYS: i64 = 366;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "RegistryAction"
)]
pub enum Action {
    Pull,
    Push,
    Delete,
}

impl Action {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pull => "pull",
            Self::Push => "push",
            Self::Delete => "delete",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "pull" => Some(Self::Pull),
            "push" => Some(Self::Push),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }
}

/// O que o token alcança.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "RegistryTokenScope"
)]
pub struct Scope {
    /// Repositórios: o nome exato, `namespace/*` ou `*` (todos).
    pub repositories: Vec<String>,
    pub actions: Vec<Action>,
}

impl Scope {
    /// O padrão casa com o repositório?
    #[must_use]
    pub fn covers(&self, repository: &str) -> bool {
        self.repositories.iter().any(|pattern| {
            pattern == "*"
                || pattern == repository
                || pattern
                    .strip_suffix("/*")
                    .is_some_and(|prefix| repository.starts_with(&format!("{prefix}/")))
        })
    }

    /// # Errors
    ///
    /// `Validation` com escopo vazio ou padrão inválido.
    pub fn validate(&self) -> AppResult<()> {
        if self.actions.is_empty() {
            return Err(AppError::validation(
                "Marque ao menos uma ação (pull ou push)",
            ));
        }
        if self.repositories.is_empty() {
            return Err(AppError::validation("Escolha ao menos um repositório"));
        }
        for pattern in &self.repositories {
            let base = pattern.strip_suffix("/*").unwrap_or(pattern);
            if pattern != "*" && !crate::names::valid_name(base) {
                return Err(AppError::validation(format!(
                    "Repositório inválido no escopo: {pattern}"
                )));
            }
        }
        Ok(())
    }
}

/// Lê o escopo gravado (o JSON da coluna).
#[must_use]
pub fn scope_of(token: &registry_tokens::Model) -> Scope {
    serde_json::from_value(token.scopes.clone()).unwrap_or(Scope {
        repositories: Vec::new(),
        actions: Vec::new(),
    })
}

/// Cria o token; devolve a linha e o texto (a única vez que ele aparece).
///
/// # Errors
///
/// `Validation` com nome, validade ou escopo fora da regra.
pub async fn create(
    db: &DatabaseConnection,
    user_id: i64,
    name: &str,
    scope: &Scope,
    days: i64,
) -> AppResult<(registry_tokens::Model, String)> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::validation(
            "Dê um nome ao token (até 120 caracteres)",
        ));
    }
    if !(1..=MAX_DAYS).contains(&days) {
        return Err(AppError::validation(format!(
            "A validade vai de 1 a {MAX_DAYS} dias"
        )));
    }
    scope.validate()?;
    let secret = format!(
        "{PREFIX}{}",
        netmonitor_core::shared::crypto::random_token()
    );
    let now = Utc::now();
    let row = registry_tokens::ActiveModel {
        user_id: Set(user_id),
        name: Set(name.to_string()),
        token_hash: Set(sha256_hex(&secret)),
        hint: Set(secret
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect()),
        scopes: Set(serde_json::to_value(scope).map_err(|error| AppError::Internal(error.into()))?),
        expires_at: Set((now + Duration::days(days)).into()),
        last_used_at: Set(None),
        last_used_ip: Set(None),
        revoked_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok((row, secret))
}

/// Os tokens (de um usuário, ou de todos para o administrador).
///
/// # Errors
///
/// Erro do banco.
pub async fn list(
    db: &DatabaseConnection,
    user_id: Option<i64>,
) -> AppResult<Vec<registry_tokens::Model>> {
    let mut query = registry_tokens::Entity::find().order_by_desc(registry_tokens::Column::Id);
    if let Some(user_id) = user_id {
        query = query.filter(registry_tokens::Column::UserId.eq(user_id));
    }
    Ok(query.all(db).await?)
}

/// Revoga. Só o dono ou o administrador (`owner = None`).
///
/// # Errors
///
/// `NotFound` quando o token não é de quem pediu.
pub async fn revoke(
    db: &DatabaseConnection,
    id: i64,
    owner: Option<i64>,
) -> AppResult<registry_tokens::Model> {
    let token = registry_tokens::Entity::find_by_id(id)
        .one(db)
        .await?
        .filter(|token| owner.is_none_or(|owner| owner == token.user_id))
        .ok_or_else(|| AppError::not_found("Token não encontrado"))?;
    let mut active: registry_tokens::ActiveModel = token.into();
    active.revoked_at = Set(Some(Utc::now().into()));
    Ok(active.update(db).await?)
}

/// O token válido com este texto (e marca o uso).
///
/// # Errors
///
/// Erro do banco.
pub async fn authenticate(
    db: &DatabaseConnection,
    secret: &str,
    ip: Option<String>,
) -> AppResult<Option<registry_tokens::Model>> {
    if !secret.starts_with(PREFIX) {
        return Ok(None);
    }
    let Some(token) = registry_tokens::Entity::find()
        .filter(registry_tokens::Column::TokenHash.eq(sha256_hex(secret)))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    if token.revoked_at.is_some() || token.expires_at.with_timezone(&Utc) <= Utc::now() {
        return Ok(None);
    }
    let mut active: registry_tokens::ActiveModel = token.clone().into();
    active.last_used_at = Set(Some(Utc::now().into()));
    active.last_used_ip = Set(ip);
    Ok(Some(active.update(db).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padroes_de_repositorio() {
        let scope = Scope {
            repositories: vec!["loja/*".into(), "tools/cli".into()],
            actions: vec![Action::Pull],
        };
        assert!(scope.covers("loja/api"));
        assert!(scope.covers("tools/cli"));
        assert!(!scope.covers("lojas/api"));
        assert!(!scope.covers("tools/cli2"));
        assert!(Scope {
            repositories: vec!["*".into()],
            actions: vec![Action::Push]
        }
        .covers("x/y"));
        assert!(Scope {
            repositories: vec!["Ruim/*".into()],
            actions: vec![Action::Pull]
        }
        .validate()
        .is_err());
    }
}
