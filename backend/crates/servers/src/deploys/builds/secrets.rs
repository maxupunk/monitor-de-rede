//! Segredos de build de uma aplicação (ADR 023): cifrados com a
//! `ENCRYPTION_KEY`, nunca devolvidos à tela (só os nomes), entregues ao
//! agente que compila e usados só como `--secret` do BuildKit.

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Serialize;
use ts_rs::TS;

use agent_protocol::docker::{
    images::BuildSecret,
    stacks::{validate_variable, Secret},
};
use netmonitor_core::{
    models::deploy_build_secrets,
    shared::{
        crypto,
        errors::{AppError, AppResult},
    },
};

/// O que a tela vê de um segredo: o nome e quando mudou.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuildSecretView"
)]
pub struct SecretView {
    pub name: String,
    pub updated_at: String,
}

/// # Errors
///
/// Banco.
pub async fn list<C: ConnectionTrait>(db: &C, application_id: i64) -> AppResult<Vec<SecretView>> {
    Ok(deploy_build_secrets::Entity::find()
        .filter(deploy_build_secrets::Column::ApplicationId.eq(application_id))
        .order_by_asc(deploy_build_secrets::Column::Name)
        .all(db)
        .await?
        .into_iter()
        .map(|row| SecretView {
            name: row.name,
            updated_at: row.updated_at.to_rfc3339(),
        })
        .collect())
}

/// Cria ou troca o valor.
///
/// # Errors
///
/// `Validation` com nome fora da regra ou valor vazio.
pub async fn save<C: ConnectionTrait>(
    db: &C,
    application_id: i64,
    name: &str,
    value: &str,
) -> AppResult<()> {
    let name = name.trim();
    validate_variable(name).map_err(|error| AppError::validation(error.to_string()))?;
    if value.is_empty() || value.len() > 64 * 1024 {
        return Err(AppError::validation(
            "Informe o valor do segredo (até 64 KB)",
        ));
    }
    let encrypted = crypto::encrypt(value)?;
    let existing = deploy_build_secrets::Entity::find()
        .filter(deploy_build_secrets::Column::ApplicationId.eq(application_id))
        .filter(deploy_build_secrets::Column::Name.eq(name))
        .one(db)
        .await?;
    let now = Utc::now().into();
    match existing {
        Some(row) => {
            let mut active: deploy_build_secrets::ActiveModel = row.into();
            active.value_encrypted = Set(encrypted);
            active.updated_at = Set(now);
            active.update(db).await?;
        }
        None => {
            deploy_build_secrets::ActiveModel {
                application_id: Set(application_id),
                name: Set(name.to_string()),
                value_encrypted: Set(encrypted),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// # Errors
///
/// `404`.
pub async fn remove<C: ConnectionTrait>(db: &C, application_id: i64, name: &str) -> AppResult<()> {
    let result = deploy_build_secrets::Entity::delete_many()
        .filter(deploy_build_secrets::Column::ApplicationId.eq(application_id))
        .filter(deploy_build_secrets::Column::Name.eq(name))
        .exec(db)
        .await?;
    if result.rows_affected == 0 {
        return Err(AppError::not_found("Segredo não encontrado"));
    }
    Ok(())
}

/// Os segredos decifrados, para o pedido ao agente.
///
/// # Errors
///
/// Banco; chave de cifra trocada.
pub async fn reveal_all<C: ConnectionTrait>(
    db: &C,
    application_id: i64,
) -> AppResult<Vec<BuildSecret>> {
    deploy_build_secrets::Entity::find()
        .filter(deploy_build_secrets::Column::ApplicationId.eq(application_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| {
            Ok(BuildSecret {
                id: row.name,
                value: Secret::new(crypto::decrypt(&row.value_encrypted)?),
            })
        })
        .collect()
}
