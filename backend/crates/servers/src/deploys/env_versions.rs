//! As versões do `.env` de cada alvo.
//!
//! Salvar nunca sobrescreve: cria uma versão nova, cifrada. O deploy guarda
//! qual versão usou — é isso que deixa o rollback voltar a imagem **e** o
//! `.env` que funcionava com ela.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::json;

use netmonitor_core::{
    models::deploy_env_versions,
    shared::{
        crypto,
        errors::{AppError, AppResult},
    },
};

use super::env::{self, EnvDiff};

/// # Errors
///
/// `Validation` do conteúdo; banco.
pub async fn save<C: ConnectionTrait>(
    db: &C,
    target_id: i64,
    content: &str,
    note: Option<String>,
    author: Option<String>,
) -> AppResult<deploy_env_versions::Model> {
    env::validate(content)?;
    let note = note
        .map(|note| note.trim().chars().take(200).collect::<String>())
        .filter(|note| !note.is_empty());
    Ok(deploy_env_versions::ActiveModel {
        target_id: Set(target_id),
        content_encrypted: Set(crypto::encrypt(content)?),
        keys: Set(json!(env::keys(content))),
        created_by: Set(author.map(|author| author.chars().take(120).collect())),
        note: Set(note),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Mais recentes primeiro.
///
/// # Errors
///
/// Banco.
pub async fn list<C: ConnectionTrait>(
    db: &C,
    target_id: i64,
) -> AppResult<Vec<deploy_env_versions::Model>> {
    Ok(deploy_env_versions::Entity::find()
        .filter(deploy_env_versions::Column::TargetId.eq(target_id))
        .order_by_desc(deploy_env_versions::Column::Id)
        .all(db)
        .await?)
}

/// A versão em vigor: a mais nova.
///
/// # Errors
///
/// Banco.
pub async fn current<C: ConnectionTrait>(
    db: &C,
    target_id: i64,
) -> AppResult<Option<deploy_env_versions::Model>> {
    Ok(deploy_env_versions::Entity::find()
        .filter(deploy_env_versions::Column::TargetId.eq(target_id))
        .order_by_desc(deploy_env_versions::Column::Id)
        .one(db)
        .await?)
}

/// # Errors
///
/// `404`.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<deploy_env_versions::Model> {
    deploy_env_versions::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Versão do .env não encontrada"))
}

/// O conteúdo em claro.
///
/// # Errors
///
/// `Validation` quando a `ENCRYPTION_KEY` mudou.
pub fn reveal(version: &deploy_env_versions::Model) -> AppResult<String> {
    crypto::decrypt(&version.content_encrypted).map_err(|_| {
        AppError::validation(
            "Esta versão do .env não pode ser lida: a ENCRYPTION_KEY mudou. Salve o .env de novo.",
        )
    })
}

/// O que mudaria salvando `content` no lugar da versão atual.
///
/// # Errors
///
/// Banco; versão atual ilegível.
pub async fn preview<C: ConnectionTrait>(
    db: &C,
    target_id: i64,
    content: &str,
) -> AppResult<EnvDiff> {
    env::validate(content)?;
    let before = match current(db, target_id).await? {
        Some(version) => reveal(&version)?,
        None => String::new(),
    };
    Ok(env::diff(&before, content))
}

/// As chaves gravadas na versão, sem decifrar.
#[must_use]
pub fn keys_of(version: &deploy_env_versions::Model) -> Vec<String> {
    serde_json::from_value(version.keys.clone()).unwrap_or_default()
}
