//! O workflow no repositório: conferir se ele está lá (e igual ao gerado),
//! como foi o último build, e — com a conexão que pode escrever — abrir o PR
//! que o coloca lá.
//!
//! O PR sai de uma branch só do NetMonitor ([`BRANCH`]), refeita a partir da
//! branch padrão a cada pedido: nunca se escreve direto na `main`, e quem
//! revisa e faz o merge é uma pessoa.

use base64::Engine as _;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{encode, Client, GitHubError};
use crate::{
    deploys::{short_sha, workflow::WORKFLOW_PATH},
    views::deploys::{
        DeployBuildView, DeployGithubStatusView, DeployPullRequestView, DeployWorkflowState,
    },
};

/// A branch do PR do NetMonitor.
pub const BRANCH: &str = "netmonitor/deploy-workflow";

#[derive(Debug, Deserialize)]
struct Repo {
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct GitRef {
    object: GitObject,
}

#[derive(Debug, Deserialize)]
struct GitObject {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct FileSha {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct Pull {
    number: u64,
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct Runs {
    workflow_runs: Vec<Run>,
}

#[derive(Debug, Deserialize)]
struct Run {
    status: Option<String>,
    conclusion: Option<String>,
    html_url: String,
    head_branch: Option<String>,
    head_sha: String,
    created_at: String,
}

/// Fim de linha e espaço sobrando não contam.
fn same_text(a: &str, b: &str) -> bool {
    let clean = |text: &str| text.replace("\r\n", "\n").trim().to_string();
    clean(a) == clean(b)
}

/// O arquivo do repositório comparado ao gerado.
#[must_use]
pub fn workflow_state(current: Option<&str>, expected: &str) -> DeployWorkflowState {
    match current {
        None => DeployWorkflowState::Missing,
        Some(text) if same_text(text, expected) => DeployWorkflowState::Current,
        Some(_) => DeployWorkflowState::Different,
    }
}

async fn open_pull(client: &Client, repository: &str) -> Result<Option<Pull>, GitHubError> {
    let owner = repository.split('/').next().unwrap_or_default();
    let pulls: Vec<Pull> = client
        .get(&format!(
            "/repos/{repository}/pulls?state=open&head={}",
            encode(&format!("{owner}:{BRANCH}"))
        ))
        .await?;
    Ok(pulls.into_iter().next())
}

fn pull_view(pull: Pull) -> DeployPullRequestView {
    DeployPullRequestView {
        number: pull.number,
        url: pull.html_url,
    }
}

async fn last_build(
    client: &Client,
    repository: &str,
) -> Result<Option<DeployBuildView>, GitHubError> {
    let file = WORKFLOW_PATH.rsplit('/').next().unwrap_or(WORKFLOW_PATH);
    let runs: Option<Runs> = client
        .find(&format!(
            "/repos/{repository}/actions/workflows/{file}/runs?per_page=1"
        ))
        .await?;
    Ok(runs
        .and_then(|runs| runs.workflow_runs.into_iter().next())
        .map(|run| DeployBuildView {
            status: run.status.unwrap_or_else(|| "unknown".into()),
            conclusion: run.conclusion,
            url: run.html_url,
            branch: run.head_branch,
            short_sha: short_sha(&run.head_sha).to_string(),
            created_at: run.created_at,
        }))
}

/// Sem como conferir (app não instalado, token revogado).
#[must_use]
pub fn unavailable(reason: String, can_write: bool) -> DeployGithubStatusView {
    DeployGithubStatusView {
        workflow: DeployWorkflowState::Unknown,
        workflow_url: None,
        pull_request: None,
        last_build: None,
        can_write,
        unavailable_reason: Some(reason),
    }
}

/// Como está o repositório. Nunca falha: o que não deu vira
/// `unavailable_reason`, e a tela mostra os passos manuais.
pub async fn status(
    client: &Client,
    repository: &str,
    expected: &str,
    can_write: bool,
) -> DeployGithubStatusView {
    let mut out = unavailable(String::new(), can_write);
    out.unavailable_reason = None;
    let result: Result<(), GitHubError> = async {
        let repo: Repo = client.get(&format!("/repos/{repository}")).await?;
        let current = client
            .raw(&format!(
                "/repos/{repository}/contents/{WORKFLOW_PATH}?ref={}",
                encode(&repo.default_branch)
            ))
            .await?;
        out.workflow = workflow_state(current.as_deref(), expected);
        if current.is_some() {
            out.workflow_url = Some(format!(
                "https://github.com/{repository}/blob/{}/{WORKFLOW_PATH}",
                repo.default_branch
            ));
        }
        out.pull_request = open_pull(client, repository).await?.map(pull_view);
        out.last_build = last_build(client, repository).await?;
        Ok(())
    }
    .await;
    if let Err(error) = result {
        out.unavailable_reason = Some(error.to_string());
    }
    out
}

/// Grava o workflow na branch do NetMonitor e abre (ou reaproveita) o PR.
///
/// # Errors
///
/// [`GitHubError`] — sem permissão de escrita, o GitHub responde 403/404.
pub async fn open_pull_request(
    client: &Client,
    repository: &str,
    yaml: &str,
) -> Result<DeployPullRequestView, GitHubError> {
    let repo: Repo = client.get(&format!("/repos/{repository}")).await?;
    let base: GitRef = client
        .get(&format!(
            "/repos/{repository}/git/ref/heads/{}",
            encode(&repo.default_branch)
        ))
        .await?;

    // A branch do NetMonitor sempre parte da padrão de agora: o PR mostra
    // só o workflow.
    let branch_path = format!("/repos/{repository}/git/refs/heads/{BRANCH}");
    let exists: Option<GitRef> = client
        .find(&format!("/repos/{repository}/git/ref/heads/{BRANCH}"))
        .await?;
    if exists.is_some() {
        client
            .write::<_, Value>(
                Method::PATCH,
                &branch_path,
                &json!({ "sha": base.object.sha, "force": true }),
            )
            .await?;
    } else {
        client
            .write::<_, Value>(
                Method::POST,
                &format!("/repos/{repository}/git/refs"),
                &json!({ "ref": format!("refs/heads/{BRANCH}"), "sha": base.object.sha }),
            )
            .await?;
    }

    let file_path = format!("/repos/{repository}/contents/{WORKFLOW_PATH}");
    let existing: Option<FileSha> = client
        .find(&format!("{file_path}?ref={}", encode(BRANCH)))
        .await?;
    let mut body = json!({
        "message": "ci: deploy contínuo pelo NetMonitor",
        "content": base64::engine::general_purpose::STANDARD.encode(yaml),
        "branch": BRANCH,
    });
    if let Some(file) = existing {
        body["sha"] = json!(file.sha);
    }
    client
        .write::<_, Value>(Method::PUT, &file_path, &body)
        .await?;

    if let Some(pull) = open_pull(client, repository).await? {
        return Ok(pull_view(pull));
    }
    let pull: Pull = client
        .write(
            Method::POST,
            &format!("/repos/{repository}/pulls"),
            &json!({
                "title": "Deploy contínuo pelo NetMonitor",
                "head": BRANCH,
                "base": repo.default_branch,
                "body": "Este workflow compila as imagens, publica no ghcr.io com a tag do commit \
                         e avisa o NetMonitor, que decide quando e onde aplicar (aprovação, \
                         janela de manutenção, backup e rollback).\n\n\
                         Nenhum segredo é necessário: o NetMonitor confere o token OIDC do \
                         próprio GitHub Actions.\n\n\
                         Aberto pelo NetMonitor — revise e faça o merge quando quiser ligar.",
            }),
        )
        .await?;
    Ok(pull_view(pull))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compara_o_workflow_sem_ligar_para_fim_de_linha() {
        assert_eq!(workflow_state(None, "a"), DeployWorkflowState::Missing);
        assert_eq!(
            workflow_state(Some("a: 1\r\nb: 2\r\n"), "a: 1\nb: 2"),
            DeployWorkflowState::Current
        );
        assert_eq!(
            workflow_state(Some("a: 1\nb: 3\n"), "a: 1\nb: 2\n"),
            DeployWorkflowState::Different
        );
    }
}
