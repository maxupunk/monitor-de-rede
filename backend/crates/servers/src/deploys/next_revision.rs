//! A versão que o "Publicar" leva — a regra num lugar só.
//!
//! * **Commit informado**: ele (a tag da imagem e a mensagem vêm de um deploy
//!   anterior do mesmo commit, quando houver).
//! * **Vazio, com versão no ar**: a do ar de novo, com o `.env` atual.
//! * **Vazio, sem versão no ar**: o commit mais novo da branch que o alvo
//!   acompanha, perguntado ao GitHub pelo acesso da aplicação
//!   ([`super::access`]). Alvo de tag (`v*`) não tem "mais novo" seguro: pede
//!   o commit.
//!
//! Quem descobre o commit mais novo é injetado ([`resolve`] recebe a
//! consulta): em produção é [`super::builds::head_of`], nos testes um dublê.
//! A aprovação, a janela e a fila continuam as do [`super::orchestrator`].

use std::future::Future;

use sea_orm::ConnectionTrait;

use agent_protocol::docker::stacks::validate_commit;
use netmonitor_core::{
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::errors::{AppError, AppResult},
};

use super::{
    access::{self, Source},
    github::{self, app::Scope},
    orchestrator::Revision,
    runs,
};
use crate::{dtos::deploys::DeployInput, views::deploys::DeployBranchCommitsView};

/// Commits recentes oferecidos no diálogo "Publicar".
pub const RECENT_COMMITS: usize = 10;

/// O texto do campo, sem espaços; vazio vira `None`.
fn filled(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// A branch cujo commit mais novo um alvo sem versão no ar publica — puro.
///
/// # Errors
///
/// `Validation` para alvo que acompanha tags (`v*`).
pub fn latest_branch(reference: &str) -> AppResult<&str> {
    let reference = reference.trim();
    if reference.contains('*') {
        return Err(AppError::validation(format!(
            "Este alvo acompanha tags ({reference}) e ainda não tem versão publicada: informe o \
             commit a publicar"
        )));
    }
    Ok(reference)
}

/// Sem conexão do GitHub App nem token não há a quem perguntar.
#[must_use]
pub fn no_access(branch: &str) -> String {
    format!(
        "A aplicação não tem acesso ao GitHub (conexão do GitHub App ou token) para descobrir o \
         último commit de {branch}: ligue-a a uma conexão ao editá-la, ou informe o commit a \
         publicar"
    )
}

/// O GitHub não respondeu o que se pediu (app desinstalado, repositório ou
/// branch que o acesso não enxerga, GitHub fora).
#[must_use]
pub fn unresolved(branch: &str, reason: &str) -> String {
    format!(
        "Não deu para descobrir o último commit de {branch} no GitHub ({reason}): confira o \
         acesso da aplicação ao repositório em Deploys → Conexões, ou informe o commit a publicar"
    )
}

/// A versão de um commit — o que se sabe dele por um deploy anterior do
/// mesmo alvo completa o que veio do pedido.
fn of_commit(
    commit: String,
    image_tag: Option<&str>,
    known: Option<deploy_runs::Model>,
    message: Option<String>,
    reference: Option<String>,
) -> Revision {
    Revision {
        image_tag: image_tag
            .map(ToString::to_string)
            .or_else(|| known.as_ref().map(|run| run.image_tag.clone()))
            .unwrap_or_else(|| commit.clone()),
        commit_message: known
            .as_ref()
            .and_then(|run| run.commit_message.clone())
            .or(message),
        build_id: known.as_ref().and_then(|run| run.build_id),
        reference: known.and_then(|run| run.reference).or(reference),
        commit_sha: commit,
    }
}

/// O deploy anterior deste commit no alvo, se houve.
async fn known<C: ConnectionTrait>(
    db: &C,
    target_id: i64,
    commit: &str,
) -> AppResult<Option<deploy_runs::Model>> {
    Ok(runs::recent(db, Some(target_id), 200)
        .await?
        .into_iter()
        .find(|run| run.commit_sha == commit))
}

/// A versão que o "Publicar" leva.
///
/// `head` responde o commit mais novo (e a mensagem) de uma branch; só é
/// chamado quando o commit vem vazio, o alvo nunca foi publicado e a
/// aplicação tem acesso ao GitHub.
///
/// # Errors
///
/// `Validation` com commit inválido, alvo de tag sem commit, aplicação sem
/// acesso ao GitHub ou GitHub que não respondeu; banco.
pub async fn resolve<C, H, Fut>(
    db: &C,
    application: &deploy_applications::Model,
    target: &deploy_targets::Model,
    input: &DeployInput,
    head: H,
) -> AppResult<Revision>
where
    C: ConnectionTrait,
    H: FnOnce(String) -> Fut,
    Fut: Future<Output = AppResult<(String, Option<String>)>>,
{
    let image_tag = filled(input.image_tag.as_deref());
    if let Some(commit) = filled(input.commit_sha.as_deref()) {
        let commit = commit.to_lowercase();
        if commit.len() != 40 {
            return Err(AppError::validation(
                "Informe o SHA completo do commit (40 caracteres)",
            ));
        }
        validate_commit(&commit).map_err(|error| AppError::validation(error.to_string()))?;
        let known = known(db, target.id, &commit).await?;
        return Ok(of_commit(commit, image_tag, known, None, None));
    }
    if let Some(live) = runs::live(db, target.id).await? {
        return Ok(Revision {
            commit_sha: live.commit_sha,
            commit_message: live.commit_message,
            reference: live.reference,
            image_tag: live.image_tag,
            build_id: live.build_id,
        });
    }
    let branch = latest_branch(&target.reference)?;
    if Source::of(application).is_empty() {
        return Err(AppError::validation(no_access(branch)));
    }
    let (commit, message) = head(branch.to_string())
        .await
        .map_err(|error| AppError::validation(unresolved(branch, &error.to_string())))?;
    let commit = commit.to_lowercase();
    validate_commit(&commit).map_err(|error| AppError::validation(error.to_string()))?;
    let known = known(db, target.id, &commit).await?;
    Ok(of_commit(
        commit,
        image_tag,
        known,
        message,
        Some(branch.to_string()),
    ))
}

/// Os commits recentes da branch do alvo, para o diálogo "Publicar"
/// escolher. É informação, não condição: o que não deu vira
/// `unavailable_reason` com o mesmo texto que o "Publicar" daria.
///
/// # Errors
///
/// Banco.
pub async fn branch_commits<C: ConnectionTrait>(
    db: &C,
    application: &deploy_applications::Model,
    target: &deploy_targets::Model,
) -> AppResult<DeployBranchCommitsView> {
    let mut view = DeployBranchCommitsView {
        branch: None,
        commits: Vec::new(),
        unavailable_reason: None,
    };
    let branch = match latest_branch(&target.reference) {
        Ok(branch) => branch,
        Err(error) => {
            view.unavailable_reason = Some(error.to_string());
            return Ok(view);
        }
    };
    view.branch = Some(branch.to_string());
    let source = Source::of(application);
    if source.is_empty() {
        view.unavailable_reason = Some(no_access(branch));
        return Ok(view);
    }
    let listed =
        match access::repository_client(db, source, &application.repository, Scope::Read).await {
            Ok(client) => {
                github::changes::recent(&client, &application.repository, branch, RECENT_COMMITS)
                    .await
                    .map_err(|error| error.to_string())
            }
            Err(error) => Err(error.to_string()),
        };
    match listed {
        Ok(commits) => view.commits = commits,
        Err(reason) => view.unavailable_reason = Some(unresolved(branch, &reason)),
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(commit: &str) -> deploy_runs::Model {
        let now = chrono::Utc::now().into();
        deploy_runs::Model {
            id: 7,
            application_id: 1,
            target_id: 1,
            trigger: "manual".into(),
            triggered_by: None,
            reference: Some("main".into()),
            commit_sha: commit.into(),
            commit_message: Some("feat: NF-e".into()),
            image_tag: "v4.2.0".into(),
            build_id: Some(3),
            env_version_id: None,
            status: "succeeded".into(),
            stage: None,
            error: None,
            log: None,
            rollback_of_run_id: None,
            after_run_id: None,
            backup_id: None,
            approved_by: None,
            approved_at: None,
            scheduled_for: None,
            started_at: None,
            finished_at: None,
            duration_ms: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn branch_exata_publica_o_mais_novo_e_tag_pede_o_commit() {
        assert_eq!(latest_branch(" main ").expect("branch"), "main");
        let error = latest_branch("v*").expect_err("tag");
        assert!(error.to_string().contains("acompanha tags (v*)"), "{error}");
    }

    #[test]
    fn commit_novo_usa_o_que_o_github_disse() {
        let sha = "a".repeat(40);
        let revision = of_commit(
            sha.clone(),
            None,
            None,
            Some("fix: login".into()),
            Some("main".into()),
        );
        assert_eq!(revision.commit_sha, sha);
        assert_eq!(revision.image_tag, sha);
        assert_eq!(revision.commit_message.as_deref(), Some("fix: login"));
        assert_eq!(revision.reference.as_deref(), Some("main"));
        assert_eq!(revision.build_id, None);
    }

    #[test]
    fn commit_ja_publicado_reaproveita_a_tag_e_a_compilacao() {
        let sha = "b".repeat(40);
        let revision = of_commit(sha.clone(), None, Some(run(&sha)), None, None);
        assert_eq!(revision.image_tag, "v4.2.0");
        assert_eq!(revision.build_id, Some(3));
        assert_eq!(revision.commit_message.as_deref(), Some("feat: NF-e"));
        // A tag pedida vence a do deploy anterior.
        let revision = of_commit(sha.clone(), Some("v5"), Some(run(&sha)), None, None);
        assert_eq!(revision.image_tag, "v5");
    }

    #[test]
    fn mensagens_dizem_o_que_fazer() {
        assert!(no_access("main").contains("informe o commit a publicar"));
        let text = unresolved("main", "O GitHub negou o acesso");
        assert!(text.contains("último commit de main"), "{text}");
        assert!(text.contains("O GitHub negou o acesso"), "{text}");
    }
}
