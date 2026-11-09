//! Qual credencial cada uso do GitHub leva — a regra num lugar só.
//!
//! * **Repositório** (ler o compose, os commits, clonar no agente): a
//!   conexão do GitHub App quando a aplicação tem uma; senão o token
//!   cadastrado; senão nenhum (repositório público).
//! * **Imagens** (`docker pull` do ghcr.io): só o token cadastrado. O ghcr.io
//!   não documenta o token de GitHub App como credencial de registry; imagem
//!   pública dispensa as duas coisas.

use sea_orm::ConnectionTrait;

use agent_protocol::docker::stacks::{RegistryAuth, Secret};
use netmonitor_core::{
    models::deploy_applications,
    shared::errors::{AppError, AppResult},
};

use super::{
    connections, credentials,
    github::{
        app::Scope,
        discovery::{self, Lister},
        Client,
    },
};
use crate::views::deploys::DeployRepositoryView;

/// De onde vem o acesso ao GitHub: a conexão (app) e/ou o token.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Source {
    pub connection_id: Option<i64>,
    pub credential_id: Option<i64>,
}

impl Source {
    #[must_use]
    pub fn of(application: &deploy_applications::Model) -> Self {
        Self {
            connection_id: application.github_connection_id,
            credential_id: application.credential_id,
        }
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.connection_id.is_none() && self.credential_id.is_none()
    }
}

/// Um cliente que lê `repository` (`dono/repo`).
///
/// # Errors
///
/// `404` da conexão ou do token; app não instalado ou sem o repositório;
/// token ilegível.
pub async fn repository_client<C: ConnectionTrait>(
    db: &C,
    source: Source,
    repository: &str,
    scope: Scope,
) -> AppResult<Client> {
    if let Some(id) = source.connection_id {
        let row = connections::find(db, id).await?;
        return connections::installation_client(&row, Some(repository), scope).await;
    }
    if let Some(id) = source.credential_id {
        let credential = credentials::open(db, id).await?;
        return Ok(Client::new(Some(credential.secret))?);
    }
    Ok(Client::new(None)?)
}

/// O token que o agente usa no `git fetch` — de uma hora e só de leitura
/// quando vem do app.
///
/// # Errors
///
/// Como [`repository_client`].
pub async fn git_token<C: ConnectionTrait>(
    db: &C,
    application: &deploy_applications::Model,
) -> AppResult<Option<String>> {
    let source = Source::of(application);
    if let Some(id) = source.connection_id {
        let row = connections::find(db, id).await?;
        let client =
            connections::installation_client(&row, Some(&application.repository), Scope::Clone)
                .await?;
        return Ok(client.token().map(ToString::to_string));
    }
    match source.credential_id {
        Some(id) => Ok(Some(credentials::open(db, id).await?.secret)),
        None => Ok(None),
    }
}

/// O login do `docker pull`, quando a aplicação tem token.
///
/// # Errors
///
/// `404`; token ilegível.
pub async fn registry_auth<C: ConnectionTrait>(
    db: &C,
    application: &deploy_applications::Model,
) -> AppResult<Option<RegistryAuth>> {
    let Some(id) = application.credential_id else {
        return Ok(None);
    };
    let credential = credentials::open(db, id).await?;
    Ok(Some(RegistryAuth {
        registry: credential.row.registry,
        username: credential.row.username,
        secret: Secret::new(credential.secret),
    }))
}

/// Os repositórios que a conexão (ou o dono do token) enxerga.
///
/// # Errors
///
/// `Validation` sem conexão nem token; os de [`repository_client`].
pub async fn repositories<C: ConnectionTrait>(
    db: &C,
    source: Source,
) -> AppResult<Vec<DeployRepositoryView>> {
    if let Some(id) = source.connection_id {
        let row = connections::find(db, id).await?;
        let client = connections::installation_client(&row, None, Scope::Read).await?;
        return Ok(discovery::repositories(&client, Lister::Installation).await?);
    }
    if let Some(id) = source.credential_id {
        let credential = credentials::open(db, id).await?;
        let client = Client::new(Some(credential.secret))?;
        return Ok(discovery::repositories(&client, Lister::User).await?);
    }
    Err(AppError::validation(
        "Escolha uma conexão com o GitHub (ou um token) para listar os repositórios",
    ))
}
