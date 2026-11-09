//! O repositório da stack no commit pedido.
//!
//! Raso (`--depth 1`) e por SHA: o diretório fica exatamente no commit da
//! imagem, sem histórico. O token **nunca** vai à linha de comando nem ao
//! `.git/config`: entra como cabeçalho HTTP por `GIT_CONFIG_*`, variáveis do
//! processo filho que morrem com ele.

use std::{path::Path, time::Duration};

use base64::Engine as _;

use agent_protocol::docker::stacks::GitSource;

use super::super::{
    cli::{self, Cli},
    maintenance::Progress,
    DockerError,
};

const FETCH_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const LOCAL_TIMEOUT: Duration = Duration::from_secs(60);

/// Variáveis do processo `git`: sem prompt, diretório confiável e, com
/// token, o cabeçalho de autorização.
#[must_use]
pub fn environment(token: Option<&str>) -> Vec<(String, String)> {
    let mut config = vec![("safe.directory".to_string(), "*".to_string())];
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        let basic =
            base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
        config.push((
            "http.extraHeader".to_string(),
            format!("Authorization: Basic {basic}"),
        ));
    }
    let mut envs = vec![
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ("GIT_CONFIG_COUNT".to_string(), config.len().to_string()),
    ];
    for (index, (key, value)) in config.into_iter().enumerate() {
        envs.push((format!("GIT_CONFIG_KEY_{index}"), key));
        envs.push((format!("GIT_CONFIG_VALUE_{index}"), value));
    }
    envs
}

/// `git fetch` raso do commit, direto pela URL (sem remote gravado).
#[must_use]
pub fn fetch_args(source: &GitSource) -> Vec<String> {
    vec![
        "fetch".into(),
        "--depth".into(),
        "1".into(),
        "--no-tags".into(),
        "--".into(),
        source.repository_url.clone(),
        source.commit.clone(),
    ]
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

/// Deixa `dir` no commit pedido e devolve o SHA completo.
///
/// Arquivos fora do Git (dados montados, o `.env`) ficam: o checkout só
/// troca o que é versionado.
///
/// # Errors
///
/// `Unavailable` sem o `git`; `Validation` com a saída quando o fetch ou o
/// checkout falham (repositório privado sem token, commit inexistente).
pub async fn sync(
    dir: &Path,
    source: &GitSource,
    progress: &Progress,
) -> Result<String, DockerError> {
    tokio::fs::create_dir_all(dir).await.map_err(|error| {
        DockerError::Validation(format!("Não foi possível criar {}: {error}", dir.display()))
    })?;
    let envs = environment(source.token.as_ref().map(|token| token.expose()));
    if !dir.join(".git").exists() {
        run(
            dir,
            &args(&["init", "--quiet"]),
            &envs,
            LOCAL_TIMEOUT,
            progress,
            "git init",
        )
        .await?;
    }
    run(
        dir,
        &fetch_args(source),
        &envs,
        FETCH_TIMEOUT,
        progress,
        "git fetch",
    )
    .await?;
    run(
        dir,
        &args(&["checkout", "--force", "--detach", "FETCH_HEAD"]),
        &envs,
        LOCAL_TIMEOUT,
        progress,
        "git checkout",
    )
    .await?;
    let (ok, head) = cli::capture("git", &["rev-parse", "HEAD"], Some(dir), LOCAL_TIMEOUT).await?;
    if ok && !head.is_empty() {
        Ok(head)
    } else {
        Err(DockerError::Validation(
            "O repositório não ficou em commit nenhum".into(),
        ))
    }
}

async fn run(
    dir: &Path,
    args: &[String],
    envs: &[(String, String)],
    timeout: Duration,
    progress: &Progress,
    label: &str,
) -> Result<(), DockerError> {
    let outcome = Cli::new("git", args, timeout)
        .envs(envs)
        .cwd(dir)
        .run(progress)
        .await?;
    if outcome.success {
        Ok(())
    } else {
        Err(DockerError::Validation(format!(
            "{label} falhou: {}",
            outcome.summary()
        )))
    }
}

#[cfg(test)]
mod tests {
    use agent_protocol::docker::stacks::Secret;

    use super::*;

    #[test]
    fn token_vai_no_ambiente_como_cabecalho() {
        let envs = environment(Some("ghp_x"));
        let value = |key: &str| {
            envs.iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(value("GIT_CONFIG_COUNT").as_deref(), Some("2"));
        assert_eq!(
            value("GIT_CONFIG_KEY_1").as_deref(),
            Some("http.extraHeader")
        );
        let header = value("GIT_CONFIG_VALUE_1").expect("cabeçalho");
        assert!(header.starts_with("Authorization: Basic "));
        assert!(!header.contains("ghp_x"), "token em claro no cabeçalho");
        assert_eq!(value("GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
    }

    #[test]
    fn sem_token_nao_ha_cabecalho() {
        let envs = environment(None);
        assert!(envs
            .iter()
            .all(|(_, value)| !value.contains("Authorization")));
        assert!(envs.contains(&("GIT_CONFIG_COUNT".into(), "1".into())));
    }

    #[test]
    fn fetch_nao_leva_token_na_linha_de_comando() {
        let source = GitSource {
            repository_url: "https://github.com/a/b.git".into(),
            commit: "a".repeat(40),
            token: Some(Secret::new("ghp_segredo")),
        };
        let args = fetch_args(&source);
        assert!(args.iter().all(|arg| !arg.contains("ghp_segredo")));
        // `--` antes da URL: nada do pedido vira opção do git.
        let separator = args.iter().position(|arg| arg == "--").expect("--");
        assert_eq!(args[separator + 1], source.repository_url);
        assert_eq!(args[separator + 2], source.commit);
    }
}
