//! O que muda entre a versão no ar e a que vai entrar: os commits, pela API
//! do GitHub, para a tela de aprovação.
//!
//! É informação, não condição: sem credencial, com o GitHub fora ou com
//! repositório que o token não lê, a tela mostra o link do "Compare" e o
//! motivo — a aprovação continua possível.

use serde::Deserialize;

use super::{Client, GitHubError};
use crate::{
    deploys::short_sha,
    views::deploys::{DeployChangesView, DeployCommitView},
};

/// Commits mostrados; o resto vira "e mais N".
const MAX_COMMITS: usize = 50;

#[derive(Debug, Deserialize)]
struct Compare {
    total_commits: u32,
    commits: Vec<Commit>,
}

#[derive(Debug, Deserialize)]
struct Commit {
    sha: String,
    html_url: String,
    commit: CommitDetail,
}

#[derive(Debug, Deserialize)]
struct CommitDetail {
    message: String,
    author: Option<Author>,
}

#[derive(Debug, Deserialize)]
struct Author {
    name: Option<String>,
    date: Option<String>,
}

fn view(commit: Commit) -> DeployCommitView {
    DeployCommitView {
        short_sha: short_sha(&commit.sha).to_string(),
        message: commit
            .commit
            .message
            .lines()
            .next()
            .unwrap_or_default()
            .to_string(),
        author: commit
            .commit
            .author
            .as_ref()
            .and_then(|author| author.name.clone()),
        date: commit.commit.author.and_then(|author| author.date),
        url: commit.html_url,
        sha: commit.sha,
    }
}

/// O link do "Compare" (ou do commit, sem versão no ar).
#[must_use]
pub fn compare_url(repository: &str, base: Option<&str>, head: &str) -> String {
    match base {
        Some(base) => format!("https://github.com/{repository}/compare/{base}...{head}"),
        None => format!("https://github.com/{repository}/commit/{head}"),
    }
}

/// Os commits de `base` (a versão no ar) até `head`. Nunca falha: o que
/// não deu vira `unavailable_reason`.
pub async fn changes(
    repository: &str,
    base: Option<&str>,
    head: &str,
    token: Option<&str>,
) -> DeployChangesView {
    let mut output = DeployChangesView {
        base_sha: base.map(ToString::to_string),
        head_sha: head.to_string(),
        compare_url: Some(compare_url(repository, base, head)),
        commits: Vec::new(),
        total: 0,
        unavailable_reason: None,
    };
    if base == Some(head) {
        output.unavailable_reason = Some("Esta versão já é a que está no ar".into());
        return output;
    }
    let client = match Client::new(token.map(ToString::to_string)) {
        Ok(client) => client,
        Err(error) => {
            output.unavailable_reason = Some(error.to_string());
            return output;
        }
    };
    let parsed = match base {
        Some(base) => client
            .get::<Compare>(&format!("/repos/{repository}/compare/{base}...{head}"))
            .await
            .map(|compare| {
                (
                    compare.total_commits,
                    compare.commits.into_iter().rev().collect::<Vec<_>>(),
                )
            }),
        None => client
            .get::<Commit>(&format!("/repos/{repository}/commits/{head}"))
            .await
            .map(|commit| (1, vec![commit])),
    };
    let parsed = parsed.map_err(|error| match error {
        GitHubError::NotFound(_) if token.is_none() => {
            "Repositório privado: ligue a aplicação a uma conexão do GitHub para ver os commits"
                .to_string()
        }
        GitHubError::NotFound(_) => "Commit ou repositório não encontrado no GitHub".into(),
        other => other.to_string(),
    });
    match parsed {
        Ok((total, commits)) => {
            output.total = total;
            output.commits = commits.into_iter().take(MAX_COMMITS).map(view).collect();
        }
        Err(reason) => output.unavailable_reason = Some(reason),
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_do_compare_ou_do_commit() {
        assert_eq!(
            compare_url("a/b", Some("111"), "222"),
            "https://github.com/a/b/compare/111...222"
        );
        assert_eq!(
            compare_url("a/b", None, "222"),
            "https://github.com/a/b/commit/222"
        );
    }

    #[tokio::test]
    async fn mesma_versao_nem_consulta() {
        let changes = changes("a/b", Some("abc"), "abc", None).await;
        assert!(changes.unavailable_reason.is_some());
        assert!(changes.commits.is_empty());
    }

    #[test]
    fn commit_mostra_so_a_primeira_linha() {
        let commit: Commit = serde_json::from_value(serde_json::json!({
            "sha": "a1b2c3d4e5",
            "html_url": "https://github.com/a/b/commit/a1b2c3d4e5",
            "commit": {"message": "feat: x\n\ncorpo", "author": {"name": "Ana", "date": "2026-11-08T10:00:00Z"}}
        }))
        .expect("commit");
        let view = view(commit);
        assert_eq!(view.message, "feat: x");
        assert_eq!(view.short_sha, "a1b2c3d");
        assert_eq!(view.author.as_deref(), Some("Ana"));
    }
}
