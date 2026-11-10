//! Nomes de repositório e de tag, pelas regras da OCI Distribution Spec.
//!
//! `<name>` é `[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*(\/[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*)*`
//! e `<reference>` (tag) é `[a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}`.

use std::sync::OnceLock;

use regex::Regex;

/// Teto do nome inteiro (o que cabe na coluna e no caminho da URL).
pub const MAX_NAME: usize = 255;

fn name_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*(/[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*)*$")
            .expect("regex do nome")
    })
}

fn tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}$").expect("regex da tag"))
}

#[must_use]
pub fn valid_name(name: &str) -> bool {
    name.len() <= MAX_NAME && name_regex().is_match(name)
}

#[must_use]
pub fn valid_tag(tag: &str) -> bool {
    tag_regex().is_match(tag)
}

/// Tag de commit (40 hexadecimais): imutável (ADR 022 RF-04).
#[must_use]
pub fn is_commit_tag(tag: &str) -> bool {
    tag.len() == 40 && tag.bytes().all(|b| b.is_ascii_hexdigit())
}

pub use netmonitor_servers::deploys::images::component;

/// O repositório de uma imagem de aplicação: `<slug>/<imagem>` (a regra é do
/// deploy, que nomeia o que guarda).
#[must_use]
pub fn application_repository(slug: &str, image: &str) -> String {
    netmonitor_servers::deploys::images::repository_for(slug, image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_da_especificacao() {
        for good in ["loja/api", "a", "acme/loja-api", "a/b/c", "x__y", "a.b-c"] {
            assert!(valid_name(good), "{good}");
        }
        for bad in [
            "", "Loja/api", "/api", "api/", "a//b", "a/../b", "-a", "a_", "a b",
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn tags() {
        assert!(valid_tag("latest"));
        assert!(valid_tag(&"a".repeat(128)));
        assert!(!valid_tag(&"a".repeat(129)));
        assert!(!valid_tag(".oculta"));
        assert!(is_commit_tag(&"ab12".repeat(10)));
        assert!(!is_commit_tag("main"));
    }

    #[test]
    fn repositorio_da_aplicacao() {
        assert_eq!(
            application_repository("Loja Online", "API_v2"),
            "loja-online/api-v2"
        );
        assert!(valid_name(&application_repository("sgc", "sgc-api")));
    }
}
