//! O que o assistente descobre no GitHub, para ninguém digitar o que o
//! repositório já diz: a lista de repositórios, as branches e tags, e o
//! compose (arquivos, imagens, variável da tag).

use serde::Deserialize;

use super::{compose, encode, Client, GitHubError, PER_PAGE};
use crate::views::deploys::{
    DeployComposeDiscovery, DeployEnvTemplateView, DeployReferencesView, DeployRepositoryView,
};

/// Até 500 repositórios: mais que isso, a busca da tela resolve.
const MAX_PAGES: usize = 5;
/// Tags recentes oferecidas como exemplo de padrão.
const MAX_TAGS: usize = 30;
/// Limite de arquivos `.env*` carregados da raiz do repositório.
const MAX_ENV_TEMPLATE_FILES: usize = 8;
/// Limite de tamanho (64 KiB) por arquivo `.env*` carregado no assistente.
const MAX_ENV_TEMPLATE_BYTES: u64 = 64 * 1024;

/// De onde vem a lista: o que a instalação do app enxerga ou o que o dono do
/// token enxerga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lister {
    Installation,
    User,
}

#[derive(Debug, Deserialize)]
struct Repo {
    full_name: String,
    private: bool,
    default_branch: String,
    description: Option<String>,
    pushed_at: Option<String>,
}

impl From<Repo> for DeployRepositoryView {
    fn from(repo: Repo) -> Self {
        Self {
            full_name: repo.full_name,
            private: repo.private,
            default_branch: repo.default_branch,
            description: repo.description.filter(|text| !text.is_empty()),
            pushed_at: repo.pushed_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct InstallationRepos {
    repositories: Vec<Repo>,
}

/// Os repositórios, mais recentes primeiro.
///
/// # Errors
///
/// [`GitHubError`].
pub async fn repositories(
    client: &Client,
    lister: Lister,
) -> Result<Vec<DeployRepositoryView>, GitHubError> {
    let mut out: Vec<Repo> = Vec::new();
    for page in 1..=MAX_PAGES {
        let batch = match lister {
            Lister::Installation => {
                client
                    .get::<InstallationRepos>(&format!(
                        "/installation/repositories?per_page={PER_PAGE}&page={page}"
                    ))
                    .await?
                    .repositories
            }
            Lister::User => {
                client
                    .get::<Vec<Repo>>(&format!(
                        "/user/repos?per_page={PER_PAGE}&page={page}&sort=pushed\
                         &affiliation=owner,collaborator,organization_member"
                    ))
                    .await?
            }
        };
        let last = batch.len() < PER_PAGE;
        out.extend(batch);
        if last {
            break;
        }
    }
    // A data ISO-8601 ordena como texto.
    out.sort_by(|a, b| b.pushed_at.cmp(&a.pushed_at));
    Ok(out.into_iter().map(Into::into).collect())
}

#[derive(Debug, Deserialize)]
struct Named {
    name: String,
}

/// Branches (a padrão primeiro) e as tags mais recentes.
///
/// # Errors
///
/// [`GitHubError`].
pub async fn references(
    client: &Client,
    repository: &str,
) -> Result<DeployReferencesView, GitHubError> {
    let repo_path = format!("/repos/{repository}");
    let branches_path = format!("/repos/{repository}/branches?per_page={PER_PAGE}");
    let tags_path = format!("/repos/{repository}/tags?per_page={MAX_TAGS}");
    let (repo, raw_branches, raw_tags) = tokio::try_join!(
        client.get::<Repo>(&repo_path),
        client.get::<Vec<Named>>(&branches_path),
        client.get::<Vec<Named>>(&tags_path),
    )?;
    let mut branches: Vec<String> = raw_branches.into_iter().map(|branch| branch.name).collect();
    branches.retain(|name| name != &repo.default_branch);
    branches.insert(0, repo.default_branch.clone());
    let tags = raw_tags.into_iter().map(|tag| tag.name).collect();
    Ok(DeployReferencesView {
        default_branch: repo.default_branch,
        branches,
        tags,
    })
}

#[derive(Debug, Deserialize)]
struct Entry {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    size: u64,
}

/// Reconhece arquivos `.env` e `.env.*` (`.env.example`, `.env.sample`,
/// `.env.production.example`, `.env.docker`, etc.), ignorando scripts como `.envrc`.
fn is_env_template_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == ".env" || (lower.starts_with(".env.") && lower.len() > ".env.".len())
}

/// Prioridade de exibição dos modelos `.env`: `.env.example` primeiro, depois
/// exemplos de produção/docker, outros exemplos/templates, ambientes e `.env`.
fn env_template_priority(name: &str) -> (u8, String) {
    let lower = name.to_ascii_lowercase();
    let rank = match lower.as_str() {
        ".env.example" => 0,
        ".env.production.example" | ".env.prod.example" | ".env.docker.example" => 1,
        _ if lower.ends_with(".example")
            || lower.ends_with(".sample")
            || lower.ends_with(".template")
            || lower.ends_with(".dist") =>
        {
            2
        }
        ".env.production" | ".env.prod" | ".env.docker" => 3,
        ".env" => 4,
        _ => 5,
    };
    (rank, lower)
}

/// Os arquivos compose da raiz e o que o principal diz, além de modelos `.env*`.
///
/// Compose inválido não é erro: vira nota, e a tela deixa preencher à mão.
///
/// # Errors
///
/// [`GitHubError`] ao listar a raiz (repositório inacessível).
pub async fn compose(
    client: &Client,
    repository: &str,
    reference: Option<&str>,
) -> Result<DeployComposeDiscovery, GitHubError> {
    let query = reference
        .map(|reference| format!("?ref={}", encode(reference)))
        .unwrap_or_default();
    let entries: Vec<Entry> = client
        .get(&format!("/repos/{repository}/contents{query}"))
        .await?;
    let mut files: Vec<String> = entries
        .iter()
        .filter(|entry| entry.kind == "file" && compose::is_compose_file(&entry.name))
        .map(|entry| entry.name.clone())
        .collect();
    files.sort();

    let mut env_names: Vec<String> = entries
        .iter()
        .filter(|entry| {
            entry.kind == "file"
                && is_env_template_file(&entry.name)
                && entry.size <= MAX_ENV_TEMPLATE_BYTES
        })
        .map(|entry| entry.name.clone())
        .collect();
    env_names.sort_by_key(|name| env_template_priority(name));
    env_names.truncate(MAX_ENV_TEMPLATE_FILES);

    let mut env_templates = Vec::with_capacity(env_names.len());
    for name in env_names {
        let path = format!("/repos/{repository}/contents/{}{query}", encode(&name));
        if let Ok(Some(content)) = client.raw(&path).await {
            if (content.len() as u64) <= MAX_ENV_TEMPLATE_BYTES {
                env_templates.push(DeployEnvTemplateView { name, content });
            }
        }
    }

    let mut out = DeployComposeDiscovery {
        suggested_files: Vec::new(),
        services: Vec::new(),
        images: Vec::new(),
        image_tag_variable: None,
        notes: Vec::new(),
        files,
        env_templates,
    };
    let Some(main) = compose::main_file(&out.files).or_else(|| out.files.first().cloned()) else {
        out.notes
            .push("Nenhum arquivo compose na raiz do repositório: informe o caminho à mão".into());
        return Ok(out);
    };
    out.suggested_files.push(main.clone());
    let Some(content) = client
        .raw(&format!(
            "/repos/{repository}/contents/{}{query}",
            encode(&main)
        ))
        .await?
    else {
        return Ok(out);
    };
    match compose::analyze(&content, &main, repository) {
        Ok(analysis) => {
            out.services = analysis.services;
            out.images = analysis.images;
            out.image_tag_variable = analysis.image_tag_variable;
            out.notes = analysis.notes;
        }
        Err(reason) => out.notes.push(reason),
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_arquivos_env_e_ignora_envrc() {
        for valid in [
            ".env",
            ".env.example",
            ".env.sample",
            ".env.production.example",
            ".env.prod",
            ".ENV.EXAMPLE",
        ] {
            assert!(is_env_template_file(valid), "deveria aceitar {valid}");
        }
        for invalid in [".envrc", ".env.", "docker-compose.yml", "README.md"] {
            assert!(
                !is_env_template_file(invalid),
                "não deveria aceitar {invalid}"
            );
        }
    }

    #[test]
    fn ordena_modelos_env_com_example_primeiro() {
        let mut names = vec![
            ".env.local",
            ".env",
            ".env.sample",
            ".env.production.example",
            ".env.example",
            ".env.prod",
        ];
        names.sort_by_key(|name| env_template_priority(name));
        assert_eq!(
            names,
            vec![
                ".env.example",
                ".env.production.example",
                ".env.sample",
                ".env.prod",
                ".env",
                ".env.local",
            ]
        );
    }
}
