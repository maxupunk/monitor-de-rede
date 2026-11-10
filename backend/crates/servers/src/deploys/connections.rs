//! As conexões com o GitHub: cada uma é um GitHub App que esta central
//! criou numa conta ou organização ([`github::app`]).
//!
//! A chave privada do app é cifrada com a `ENCRYPTION_KEY` e nunca sai da
//! central — nem para a tela, nem para o agente. O que sai são tokens de
//! instalação de uma hora, pedidos na hora do uso.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use netmonitor_core::{
    models::{deploy_applications, deploy_github_connections},
    shared::{
        crypto,
        errors::{AppError, AppResult},
    },
};

use super::github::{
    app::{self, AppKey, Conversion, Installation, Scope},
    Client,
};

/// # Errors
///
/// Banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<deploy_github_connections::Model>> {
    Ok(deploy_github_connections::Entity::find()
        .order_by_asc(deploy_github_connections::Column::AccountLogin)
        .all(db)
        .await?)
}

/// # Errors
///
/// `404` quando não existe.
pub async fn find<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> AppResult<deploy_github_connections::Model> {
    deploy_github_connections::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Conexão com o GitHub não encontrada"))
}

/// Grava o app recém-criado (ou o mesmo app de novo, se a troca se repetir).
///
/// # Errors
///
/// Banco; cifra.
pub async fn save<C: ConnectionTrait>(
    db: &C,
    conversion: &Conversion,
) -> AppResult<deploy_github_connections::Model> {
    let existing = deploy_github_connections::Entity::find()
        .filter(deploy_github_connections::Column::AppId.eq(conversion.id))
        .one(db)
        .await?;
    let is_new = existing.is_none();
    let mut active: deploy_github_connections::ActiveModel = match existing {
        Some(row) => row.into(),
        None => deploy_github_connections::ActiveModel {
            app_id: Set(conversion.id),
            installation_id: Set(None),
            ..Default::default()
        },
    };
    active.account_login = Set(conversion.owner.login.clone());
    active.account_type = Set(conversion.owner.kind.clone());
    active.app_slug = Set(conversion.slug.clone());
    active.client_id = Set(conversion.client_id.clone());
    active.private_key_encrypted = Set(crypto::encrypt(&conversion.pem)?);
    active.can_write = Set(conversion.can_write());
    if let Some(secret) = conversion.webhook_secret.as_deref() {
        active.webhook_secret_encrypted = Set(Some(crypto::encrypt(secret)?));
    }
    Ok(if is_new {
        active.insert(db).await?
    } else {
        active.update(db).await?
    })
}

/// A chave do app, decifrada.
///
/// # Errors
///
/// `Validation` quando a `ENCRYPTION_KEY` mudou ou a chave é inválida.
pub fn key(row: &deploy_github_connections::Model) -> AppResult<AppKey> {
    let pem = crypto::decrypt(&row.private_key_encrypted).map_err(|_| {
        AppError::validation(format!(
            "A chave da conexão com {} não pode ser lida: a ENCRYPTION_KEY mudou. Remova a \
             conexão e conecte o GitHub de novo.",
            row.account_login
        ))
    })?;
    AppKey::from_pem(&row.client_id, &pem)
}

/// Um cliente que fala como o app (as rotas `/app/...`).
///
/// # Errors
///
/// Chave ilegível; cliente HTTP.
pub fn app_client(row: &deploy_github_connections::Model) -> AppResult<Client> {
    key(row)?.client(&Client::new(None)?)
}

/// Um cliente com token da instalação, limitado a `repository` (`dono/repo`)
/// quando vem.
///
/// # Errors
///
/// `BusinessRule` com o app ainda não instalado ou sem acesso ao
/// repositório; GitHub.
pub async fn installation_client(
    row: &deploy_github_connections::Model,
    repository: Option<&str>,
    scope: Scope,
) -> AppResult<Client> {
    let installation = row.installation_id.ok_or_else(|| {
        AppError::business_rule(format!(
            "O GitHub App de {} ainda não foi instalado: em Deploys → Conexões, clique em \
             \"Instalar\" e escolha os repositórios",
            row.account_login
        ))
    })?;
    let app = app_client(row)?;
    let name = repository.and_then(|full| full.split_once('/').map(|(_, name)| name));
    let token = app::installation_token(&app, installation, name, scope).await?;
    Ok(app.with_token(token))
}

async fn apply<C: ConnectionTrait>(
    db: &C,
    row: deploy_github_connections::Model,
    installation: Option<&Installation>,
) -> AppResult<deploy_github_connections::Model> {
    let mut active: deploy_github_connections::ActiveModel = row.into();
    active.installation_id = Set(installation.map(|item| item.id));
    if let Some(installation) = installation {
        active.can_write = Set(app::can_write(&installation.permissions));
    }
    Ok(active.update(db).await?)
}

/// O retorno da instalação: descobre de qual conexão ela é (a tela não sabe)
/// e a grava.
///
/// # Errors
///
/// `404` quando nenhum app desta central tem essa instalação; GitHub.
pub async fn claim<C: ConnectionTrait>(
    db: &C,
    installation_id: i64,
) -> AppResult<deploy_github_connections::Model> {
    for row in list(db).await? {
        let app = app_client(&row)?;
        if let Some(installation) = app::installation(&app, installation_id).await? {
            return apply(db, row, Some(&installation)).await;
        }
    }
    Err(AppError::not_found(
        "Esta instalação não é de um GitHub App desta central",
    ))
}

/// Relê do GitHub a instalação (instalado? com quais permissões?).
///
/// # Errors
///
/// `404`; GitHub.
pub async fn refresh<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> AppResult<deploy_github_connections::Model> {
    let row = find(db, id).await?;
    let installations = app::installations(&app_client(&row)?).await?;
    apply(db, row, installations.first()).await
}

/// # Errors
///
/// `404`; `409` enquanto alguma aplicação usa a conexão.
pub async fn delete<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> AppResult<deploy_github_connections::Model> {
    let row = find(db, id).await?;
    let used = deploy_applications::Entity::find()
        .filter(deploy_applications::Column::GithubConnectionId.eq(id))
        .count(db)
        .await?;
    if used > 0 {
        return Err(AppError::conflict(format!(
            "A conexão com {} está em uso por {used} aplicação(ões): troque a conexão delas \
             antes de remover",
            row.account_login
        )));
    }
    deploy_github_connections::Entity::delete_by_id(id)
        .exec(db)
        .await?;
    Ok(row)
}

/// As conexões com segredo de webhook, decifrado (para conferir a
/// assinatura de um aviso de push).
///
/// # Errors
///
/// Banco.
pub async fn webhook_secrets<C: ConnectionTrait>(
    db: &C,
) -> AppResult<Vec<(deploy_github_connections::Model, String)>> {
    Ok(deploy_github_connections::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .filter_map(|row| {
            let secret = crypto::decrypt(row.webhook_secret_encrypted.as_deref()?).ok()?;
            Some((row, secret))
        })
        .collect())
}
