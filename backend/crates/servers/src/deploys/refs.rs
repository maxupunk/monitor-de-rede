//! Qual alvo um evento do GitHub alimenta: branch exata (`main`) ou padrão
//! de tag com `*` no fim (`v*`).

use netmonitor_core::shared::errors::{AppError, AppResult};

/// O ref do GitHub (`refs/heads/main`, `refs/tags/v1.2`) como a tela mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitRef {
    Branch(String),
    Tag(String),
}

impl GitRef {
    /// # Errors
    ///
    /// `Validation` para ref que não é de branch nem de tag (pull request).
    pub fn parse(full: &str) -> AppResult<Self> {
        if let Some(branch) = full.strip_prefix("refs/heads/") {
            return Ok(Self::Branch(branch.to_string()));
        }
        if let Some(tag) = full.strip_prefix("refs/tags/") {
            return Ok(Self::Tag(tag.to_string()));
        }
        Err(AppError::validation(format!(
            "O deploy só reage a branch ou tag, não a \"{full}\""
        )))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Branch(name) | Self::Tag(name) => name,
        }
    }
}

/// O filtro do alvo aceita o ref?
///
/// Sem `*`, é uma branch exata. Com `*` no fim, é um prefixo de tag
/// (`v*` casa `v4.2.0`) — padrão de branch não existe de propósito: uma
/// branch nova de alguém não pode virar deploy em produção.
#[must_use]
pub fn matches(filter: &str, reference: &GitRef) -> bool {
    let filter = filter.trim();
    match (filter.strip_suffix('*'), reference) {
        (Some(prefix), GitRef::Tag(tag)) => tag.starts_with(prefix),
        (None, GitRef::Branch(branch)) => branch == filter,
        _ => false,
    }
}

/// # Errors
///
/// `Validation` para filtro vazio, com espaço, ou com `*` fora do fim.
pub fn validate_filter(filter: &str) -> AppResult<()> {
    let filter = filter.trim();
    let star = filter.find('*');
    let valid = !filter.is_empty()
        && filter.len() <= 120
        && !filter.chars().any(char::is_whitespace)
        && !filter.starts_with('-')
        && star.is_none_or(|position| position == filter.len() - 1);
    if valid {
        Ok(())
    } else {
        Err(AppError::validation(
            "Informe a branch (ex.: main) ou um padrão de tag terminado em * (ex.: v*)",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_exata_e_prefixo_de_tag() {
        let main = GitRef::parse("refs/heads/main").expect("branch");
        let tag = GitRef::parse("refs/tags/v4.2.0").expect("tag");
        assert!(matches("main", &main));
        assert!(!matches("main", &tag));
        assert!(matches("v*", &tag));
        assert!(!matches("v*", &GitRef::Branch("v2".into())));
        assert!(!matches("develop", &main));
    }

    #[test]
    fn pull_request_nao_e_deploy() {
        assert!(GitRef::parse("refs/pull/3/merge").is_err());
    }

    #[test]
    fn filtro_valido() {
        assert!(validate_filter("main").is_ok());
        assert!(validate_filter("release/2026").is_ok());
        assert!(validate_filter("v*").is_ok());
        assert!(validate_filter("*").is_ok());
        assert!(validate_filter("v*x").is_err());
        assert!(validate_filter("").is_err());
        assert!(validate_filter("a b").is_err());
    }
}
