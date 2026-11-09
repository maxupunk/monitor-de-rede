//! O cofre do deploy: tokens que leem o repositório e baixam as imagens.
//!
//! O segredo é cifrado com a `ENCRYPTION_KEY` (XChaCha20-Poly1305) e nunca
//! volta à tela — a listagem diz só se ele existe. Ele sai em claro em dois
//! lugares: no pedido ao agente (`Secret`, que não aparece em `Debug`) e na
//! consulta dos commits à API do GitHub.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use agent_protocol::docker::stacks::validate_registry;
use netmonitor_core::{
    models::{deploy_applications, deploy_credentials},
    shared::{
        crypto,
        errors::{AppError, AppResult},
    },
};

use crate::dtos::deploys::CredentialInput;

/// O registry padrão: o GitHub Container Registry.
pub const DEFAULT_REGISTRY: &str = "ghcr.io";

/// Uma credencial com o segredo em claro, só para uso imediato.
pub struct OpenCredential {
    pub row: deploy_credentials::Model,
    pub secret: String,
}

/// # Errors
///
/// Banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<deploy_credentials::Model>> {
    Ok(deploy_credentials::Entity::find()
        .order_by_asc(deploy_credentials::Column::Name)
        .all(db)
        .await?)
}

/// # Errors
///
/// `404` quando não existe.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_credentials::Model> {
    deploy_credentials::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Credencial não encontrada"))
}

/// A credencial com o segredo decifrado.
///
/// # Errors
///
/// `404`; segredo ilegível (a `ENCRYPTION_KEY` mudou).
pub async fn open<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<OpenCredential> {
    let row = find(db, id).await?;
    let secret = crypto::decrypt(&row.secret_encrypted).map_err(|_| {
        AppError::validation(format!(
            "O token da credencial \"{}\" não pode ser lido: a ENCRYPTION_KEY mudou. Edite a \
             credencial e informe o token de novo.",
            row.name
        ))
    })?;
    Ok(OpenCredential { row, secret })
}

fn normalized(input: &CredentialInput) -> AppResult<(String, String, String)> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.len() > 120 {
        return Err(AppError::validation(
            "Informe um nome de até 120 caracteres",
        ));
    }
    let registry = input
        .registry
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_REGISTRY)
        .to_lowercase();
    validate_registry(&registry).map_err(|error| AppError::validation(error.to_string()))?;
    let username = input.username.trim().to_string();
    if username.is_empty() || username.len() > 128 {
        return Err(AppError::validation(
            "Informe o usuário dono do token (no GitHub, o seu login)",
        ));
    }
    Ok((name, registry, username))
}

fn secret_of(input: &CredentialInput) -> Option<&str> {
    input
        .secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// # Errors
///
/// `Validation`; `409` com nome repetido.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    input: &CredentialInput,
) -> AppResult<deploy_credentials::Model> {
    let (name, registry, username) = normalized(input)?;
    let secret = secret_of(input).ok_or_else(|| AppError::validation("Informe o token"))?;
    ensure_unique_name(db, &name, None).await?;
    Ok(deploy_credentials::ActiveModel {
        name: Set(name),
        registry: Set(registry),
        username: Set(username),
        secret_encrypted: Set(crypto::encrypt(secret)?),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Sem `secret`, o token salvo continua.
///
/// # Errors
///
/// `404`, `Validation`, `409`.
pub async fn update<C: ConnectionTrait>(
    db: &C,
    id: i64,
    input: &CredentialInput,
) -> AppResult<deploy_credentials::Model> {
    let row = find(db, id).await?;
    let (name, registry, username) = normalized(input)?;
    ensure_unique_name(db, &name, Some(id)).await?;
    let mut active: deploy_credentials::ActiveModel = row.into();
    active.name = Set(name);
    active.registry = Set(registry);
    active.username = Set(username);
    if let Some(secret) = secret_of(input) {
        active.secret_encrypted = Set(crypto::encrypt(secret)?);
    }
    Ok(active.update(db).await?)
}

/// # Errors
///
/// `404`; `409` enquanto alguma aplicação usa a credencial.
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_credentials::Model> {
    let row = find(db, id).await?;
    let used = deploy_applications::Entity::find()
        .filter(deploy_applications::Column::CredentialId.eq(id))
        .count(db)
        .await?;
    if used > 0 {
        return Err(AppError::conflict(format!(
            "A credencial \"{}\" está em uso por {used} aplicação(ões): troque a credencial delas \
             antes de excluir",
            row.name
        )));
    }
    deploy_credentials::Entity::delete_by_id(id)
        .exec(db)
        .await?;
    Ok(row)
}

async fn ensure_unique_name<C: ConnectionTrait>(
    db: &C,
    name: &str,
    except: Option<i64>,
) -> AppResult<()> {
    let mut query =
        deploy_credentials::Entity::find().filter(deploy_credentials::Column::Name.eq(name));
    if let Some(id) = except {
        query = query.filter(deploy_credentials::Column::Id.ne(id));
    }
    if query.count(db).await? > 0 {
        return Err(AppError::conflict(format!(
            "Já existe uma credencial chamada \"{name}\""
        )));
    }
    Ok(())
}
