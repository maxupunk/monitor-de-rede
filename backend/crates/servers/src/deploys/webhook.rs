//! Do evento verificado do GitHub aos deploys pedidos.
//!
//! O repositório do token escolhe as aplicações; o ref escolhe os alvos. Um
//! alvo que já está nessa versão (no ar, na fila ou rodando) fica de fora:
//! reexecutar o workflow não vira um segundo deploy igual.

use loco_rs::app::AppContext;
use serde::Serialize;
use ts_rs::TS;

use agent_protocol::docker::stacks::{validate_commit, validate_image_tag};
use netmonitor_core::{
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::errors::{AppError, AppResult},
};

use super::{
    applications,
    oidc::GithubClaims,
    orchestrator::{self, Revision},
    refs::{self, GitRef},
    runs::{self, status_of},
    RunStatus,
};
use crate::dtos::deploys::HookInput;

/// O que a chamada do workflow recebe de volta (aparece no log do Actions).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployHookResponse"
)]
pub struct HookResponse {
    pub message: String,
    pub runs: Vec<HookRun>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployHookRun"
)]
pub struct HookRun {
    #[ts(type = "number")]
    pub id: i64,
    pub target: String,
    pub status: RunStatus,
}

/// O alvo já está nesta versão, ou a caminho dela?
fn already_there(history: &[deploy_runs::Model], commit: &str, tag: &str) -> bool {
    let latest_relevant = history.iter().find(|run| {
        let status = status_of(run);
        status == RunStatus::Succeeded || status == RunStatus::Running || status.is_pending()
    });
    latest_relevant.is_some_and(|run| run.commit_sha == commit && run.image_tag == tag)
}

fn wanted(target: &deploy_targets::Model, reference: &GitRef) -> bool {
    target.enabled && target.probe_id.is_some() && refs::matches(&target.reference, reference)
}

/// # Errors
///
/// `Validation` para ref que não é branch nem tag, commit ou tag inválidos;
/// `404` quando nenhuma aplicação usa o repositório.
pub async fn handle(
    ctx: &AppContext,
    claims: &GithubClaims,
    input: &HookInput,
) -> AppResult<HookResponse> {
    let reference = GitRef::parse(&claims.git_ref)?;
    let commit = claims.sha.to_lowercase();
    validate_commit(&commit).map_err(|error| AppError::validation(error.to_string()))?;
    let image_tag = input
        .image_tag
        .as_deref()
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map_or_else(|| commit.clone(), ToString::to_string);
    validate_image_tag(&image_tag).map_err(|error| AppError::validation(error.to_string()))?;

    let applications = applications::by_repository(&ctx.db, &claims.repository).await?;
    if applications.is_empty() {
        return Err(AppError::not_found(format!(
            "Nenhuma aplicação do NetMonitor usa o repositório {}: cadastre-a em Deploys",
            claims.repository
        )));
    }
    let revision = Revision {
        commit_sha: commit,
        commit_message: input.commit_message.clone(),
        reference: Some(reference.name().to_string()),
        image_tag,
        build_id: None,
    };
    let mut created = Vec::new();
    for application in &applications {
        created.extend(
            request_revision(
                ctx,
                application,
                &reference,
                &revision,
                claims.actor.clone(),
            )
            .await?,
        );
    }
    let message = if created.is_empty() {
        format!(
            "Nenhum alvo ativo acompanha {} (ou todos já estão nesta versão)",
            reference.name()
        )
    } else {
        format!("{} deploy(s) pedido(s)", created.len())
    };
    Ok(HookResponse {
        message,
        runs: created,
    })
}

/// Os deploys de uma revisão nova de uma aplicação: um por alvo ativo que
/// acompanha o ref e ainda não está nela. Vale para o aviso do GitHub Actions
/// (ADR 021) e para o fim de uma compilação no servidor (ADR 023).
///
/// # Errors
///
/// Banco.
pub async fn request_revision(
    ctx: &AppContext,
    application: &deploy_applications::Model,
    reference: &GitRef,
    revision: &Revision,
    actor: Option<String>,
) -> AppResult<Vec<HookRun>> {
    let mut targets = Vec::new();
    for target in applications::targets_of(&ctx.db, application.id).await? {
        if !wanted(&target, reference) {
            continue;
        }
        let history = runs::recent(&ctx.db, Some(target.id), 20).await?;
        if !already_there(&history, &revision.commit_sha, &revision.image_tag) {
            targets.push(target);
        }
    }
    let mut created = Vec::new();
    for run in orchestrator::request_from_hook(ctx, application, &targets, revision, actor).await? {
        let target = targets
            .iter()
            .find(|target| target.id == run.target_id)
            .map_or_else(String::new, |target| target.name.clone());
        created.push(HookRun {
            id: run.id,
            target: format!("{} / {target}", application.name),
            status: status_of(&run),
        });
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn run(id: i64, status: RunStatus, commit: &str) -> deploy_runs::Model {
        let now = Utc::now().into();
        deploy_runs::Model {
            id,
            application_id: 1,
            target_id: 1,
            trigger: "webhook".into(),
            triggered_by: None,
            reference: Some("main".into()),
            commit_sha: commit.into(),
            commit_message: None,
            image_tag: commit.into(),
            env_version_id: None,
            status: status.as_str().into(),
            stage: None,
            rollback_of_run_id: None,
            after_run_id: None,
            backup_id: None,
            approved_by: None,
            approved_at: None,
            scheduled_for: None,
            error: None,
            log: None,
            started_at: None,
            finished_at: None,
            duration_ms: None,
            build_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn mesma_versao_no_ar_ou_na_fila_nao_repete() {
        let history = vec![
            run(3, RunStatus::Failed, "b"),
            run(2, RunStatus::Succeeded, "a"),
        ];
        assert!(already_there(&history, "a", "a"));
        assert!(
            !already_there(&history, "b", "b"),
            "falha não conta como versão no ar"
        );
        let queued = vec![
            run(4, RunStatus::Queued, "c"),
            run(2, RunStatus::Succeeded, "a"),
        ];
        assert!(already_there(&queued, "c", "c"));
        assert!(
            !already_there(&queued, "a", "a"),
            "a fila já leva outra versão"
        );
    }
}
