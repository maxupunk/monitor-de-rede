//! O GitHub no deploy contínuo: conectar (o GitHub App pelo manifest), o
//! que o assistente descobre no repositório e o PR do workflow.
//!
//! Tudo aqui é do administrador da área Servidores — inclusive as leituras,
//! que usam a credencial da central (`users::request_is_allowed`, prefixo
//! `/github`).

use axum::{http::HeaderMap, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;

use netmonitor_core::{
    audit::AuditAction,
    shared::errors::{AppError, AppResult},
};

use super::deploys::{audit, central_url};
use crate::{
    deploys::{
        access::{self, Source},
        applications, connections, events,
        github::{
            app::{self, Scope},
            discovery, publish, split_repository, Client,
        },
    },
    dtos::deploys::{
        CentralUrlQuery, DeployGithubConnectInput, DeployGithubInstallationInput,
        DeployGithubManifestInput, DeployPullRequestInput, GithubSourceQuery,
    },
    views::deploys::{DeployGithubConnectionView, DeployGithubManifestView},
};

fn source(query: &GithubSourceQuery) -> Source {
    Source {
        connection_id: query.connection_id,
        credential_id: query.credential_id,
    }
}

fn repository(query: &GithubSourceQuery) -> AppResult<String> {
    let value = query
        .repository
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .trim_end_matches(".git")
        .to_string();
    split_repository(&value)
        .ok_or_else(|| AppError::validation("Informe o repositório como dono/repositorio"))?;
    Ok(value)
}

fn reference(query: &GithubSourceQuery) -> Option<&str> {
    query
        .reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn connection_label(row: &netmonitor_core::models::deploy_github_connections::Model) -> String {
    format!("GitHub {} ({})", row.account_login, row.app_slug)
}

// ── Conectar ─────────────────────────────────────────────────────────────

async fn manifest(Json(input): Json<DeployGithubManifestInput>) -> AppResult<Response> {
    let origin = app::normalize_origin(&input.origin)?;
    let organization = input
        .organization
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(org) = organization {
        app::validate_login(org)?;
    }
    let state = app::begin();
    let manifest = app::manifest(&origin, &app::suggested_name(), input.allow_write);
    Ok(format::json(DeployGithubManifestView {
        action_url: app::form_url(organization, &state),
        manifest: manifest.to_string(),
    })?)
}

async fn connect(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DeployGithubConnectInput>,
) -> AppResult<Response> {
    app::finish(input.state.trim())?;
    let conversion = app::convert(&Client::new(None)?, input.code.trim()).await?;
    let row = connections::save(&ctx.db, &conversion).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (row.id, &connection_label(&row)),
        format!(
            "GitHub App '{}' criado em {} para o deploy contínuo",
            row.app_slug, row.account_login
        ),
    )
    .await;
    events::publish_updated(&ctx);
    Ok((
        StatusCode::CREATED,
        Json(DeployGithubConnectionView::from(&row)),
    )
        .into_response())
}

async fn installation(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DeployGithubInstallationInput>,
) -> AppResult<Response> {
    let row = connections::claim(&ctx.db, input.installation_id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Update,
        (row.id, &connection_label(&row)),
        format!(
            "GitHub App '{}' instalado em {}",
            row.app_slug, row.account_login
        ),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(format::json(DeployGithubConnectionView::from(&row))?)
}

async fn refresh(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = connections::refresh(&ctx.db, id).await?;
    events::publish_updated(&ctx);
    Ok(format::json(DeployGithubConnectionView::from(&row))?)
}

async fn disconnect(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let row = connections::delete(&ctx.db, id).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Delete,
        (row.id, &connection_label(&row)),
        format!(
            "Conexão com o GitHub ({}) removida da central",
            row.account_login
        ),
    )
    .await;
    events::publish_updated(&ctx);
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ── Descoberta ───────────────────────────────────────────────────────────

async fn repositories(
    State(ctx): State<AppContext>,
    Query(query): Query<GithubSourceQuery>,
) -> AppResult<Response> {
    Ok(format::json(
        access::repositories(&ctx.db, source(&query)).await?,
    )?)
}

async fn references(
    State(ctx): State<AppContext>,
    Query(query): Query<GithubSourceQuery>,
) -> AppResult<Response> {
    let repository = repository(&query)?;
    let client =
        access::repository_client(&ctx.db, source(&query), &repository, Scope::Read).await?;
    Ok(format::json(
        discovery::references(&client, &repository).await?,
    )?)
}

async fn compose(
    State(ctx): State<AppContext>,
    Query(query): Query<GithubSourceQuery>,
) -> AppResult<Response> {
    let repository = repository(&query)?;
    let client =
        access::repository_client(&ctx.db, source(&query), &repository, Scope::Read).await?;
    Ok(format::json(
        discovery::compose(&client, &repository, reference(&query)).await?,
    )?)
}

// ── O workflow no repositório ────────────────────────────────────────────

async fn status(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<CentralUrlQuery>,
) -> AppResult<Response> {
    let application = applications::find(&ctx.db, id).await?;
    let central = central_url(&query)?;
    let expected = applications::workflow_yaml(&ctx.db, &application, central.as_deref()).await?;
    let can_write = match application.github_connection_id {
        Some(connection) => connections::find(&ctx.db, connection).await?.can_write,
        None => false,
    };
    let view = match access::repository_client(
        &ctx.db,
        Source::of(&application),
        &application.repository,
        Scope::Read,
    )
    .await
    {
        Ok(client) => publish::status(&client, &application.repository, &expected, can_write).await,
        Err(error) => publish::unavailable(error.to_string(), can_write),
    };
    Ok(format::json(view)?)
}

async fn pull_request(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<DeployPullRequestInput>,
) -> AppResult<Response> {
    let application = applications::find(&ctx.db, id).await?;
    let connection_id = application.github_connection_id.ok_or_else(|| {
        AppError::business_rule(
            "A aplicação não tem conexão com o GitHub: copie o workflow e crie o arquivo no \
             repositório",
        )
    })?;
    let connection = connections::find(&ctx.db, connection_id).await?;
    if !connection.can_write {
        return Err(AppError::business_rule(format!(
            "A conexão com {} é só de leitura: conecte o GitHub de novo marcando \"abrir o PR do \
             workflow\", ou copie o workflow",
            connection.account_login
        )));
    }
    let central = app::normalize_origin(&input.central_url)?;
    let yaml = applications::workflow_yaml(&ctx.db, &application, Some(&central)).await?;
    let client =
        connections::installation_client(&connection, Some(&application.repository), Scope::Full)
            .await?;
    let pull = publish::open_pull_request(&client, &application.repository, &yaml).await?;
    audit(
        &ctx,
        &headers,
        AuditAction::Create,
        (application.id, &application.name),
        format!(
            "PR #{} do workflow de deploy aberto em {}",
            pull.number, application.repository
        ),
    )
    .await;
    Ok(format::json(pull)?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/deploys")
        .add("/github/manifest", post(manifest))
        .add("/github/connections", post(connect))
        .add("/github/connections/{id}", delete(disconnect))
        .add("/github/connections/{id}/refresh", post(refresh))
        .add("/github/installations", post(installation))
        .add("/github/repositories", get(repositories))
        .add("/github/references", get(references))
        .add("/github/compose", get(compose))
        .add("/applications/{id}/github", get(status))
        .add("/applications/{id}/github/pull-request", post(pull_request))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repositorio_da_consulta() {
        let query = GithubSourceQuery {
            repository: Some(" maxupunk/SGCv4.git ".into()),
            ..Default::default()
        };
        assert_eq!(repository(&query).expect("ok"), "maxupunk/SGCv4");
        let invalid = GithubSourceQuery {
            repository: Some("https://github.com/x".into()),
            ..Default::default()
        };
        assert!(repository(&invalid).is_err());
    }
}
