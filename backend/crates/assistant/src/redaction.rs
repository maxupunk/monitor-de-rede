//! Padrões de credencial em texto livre — log, console, termo de busca.
//!
//! Um só lugar para reconhecer o que dá acesso a alguma coisa: bloco PEM,
//! usuário e senha embutidos numa URL, par `senha=...`, cabeçalho de
//! autorização e token do GitHub. Quem pesquisa na web
//! ([`crate::web_search::query`]) tira esses trechos do termo; quem manda um
//! console à IA ([`crate::log_analysis`]) os troca por um marcador visível.

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// O que fica no lugar da credencial num texto entregue à IA.
pub const MASK: &str = "[credencial omitida]";

pub(crate) static PEM_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)-----BEGIN [A-Z0-9 ]+-----.*?(?:-----END [A-Z0-9 ]+-----|$)")
        .expect("regex de bloco PEM")
});
pub(crate) static URL_USERINFO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*://)[^\s/:@]+(?::[^\s/@]*)?@").expect("regex de userinfo")
});
pub(crate) static SECRET_PAIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b(?:password|passwd|pwd|pass|senha|secret|segredo|token|api[_-]?key|apikey|community|comunidade|psk|(?:pre[_-]?shared|private)[_-]?key|key|auth)\s*[:=]\s*(?:"[^"]*"|'[^']*'|\S+)"#,
    )
    .expect("regex de par com segredo")
});
pub(crate) static AUTH_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]{8,}").expect("regex de autorização")
});
/// Variável de ambiente com segredo no nome (`DB_PASSWORD=`, `NPM_TOKEN:`),
/// que o `\b` de [`SECRET_PAIR`] não pega por causa do `_`. O nome fica.
static ENV_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b((?:[a-z0-9]+_)+(?:password|passwd|pass|pwd|secret|token|api[_-]?key|apikey|private[_-]?key|access[_-]?key|credentials?|auth)[a-z0-9_]*)\s*[:=]\s*(?:"[^"]*"|'[^']*'|\S+)"#,
    )
    .expect("regex de variável com segredo")
});
/// Tokens do GitHub (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`).
static GITHUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})")
        .expect("regex de token do GitHub")
});

/// Troca as credenciais de `text` por [`MASK`], sem mudar a contagem de
/// linhas (um bloco PEM de 30 linhas vira o marcador seguido das mesmas 30
/// quebras) — quem numera as linhas antes e depois continua de acordo.
/// Devolve também se alguma coisa foi trocada.
#[must_use]
pub fn mask_credentials(text: &str) -> (String, bool) {
    let mut masked = false;
    let mut out = PEM_BLOCK
        .replace_all(text, |captures: &Captures| {
            masked = true;
            let breaks = captures[0].matches('\n').count();
            format!("{MASK}{}", "\n".repeat(breaks))
        })
        .into_owned();
    let mut apply = |pattern: &Regex, replacement: &str| {
        if pattern.is_match(&out) {
            masked = true;
            out = pattern.replace_all(&out, replacement).into_owned();
        }
    };
    apply(&URL_USERINFO, &format!("${{1}}{MASK}@"));
    apply(&ENV_SECRET, &format!("${{1}}={MASK}"));
    apply(&SECRET_PAIR, MASK);
    apply(&AUTH_HEADER, MASK);
    apply(&GITHUB_TOKEN, MASK);
    (out, masked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mascara_senha_url_cabecalho_e_token_do_github() {
        let (text, masked) = mask_credentials(
            "git clone https://x-access-token:ghs_abc@github.com/o/r\n\
             DB_PASSWORD=hunter2\n\
             Authorization: Bearer abcdefghijklmnop\n\
             usando ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
        );
        assert!(masked);
        assert!(!text.contains("ghs_abc"));
        assert!(!text.contains("hunter2"));
        assert!(!text.contains("abcdefghijklmnop"));
        assert!(!text.contains("ghp_ABCDEFGHIJKLMNOP"));
        assert!(text.contains("https://[credencial omitida]@github.com/o/r"));
        assert!(text.contains("DB_PASSWORD=[credencial omitida]"));
    }

    #[test]
    fn bloco_pem_preserva_a_contagem_de_linhas() {
        let original =
            "antes\n-----BEGIN PRIVATE KEY-----\nAAAA\nBBBB\n-----END PRIVATE KEY-----\ndepois";
        let (text, masked) = mask_credentials(original);
        assert!(masked);
        assert_eq!(text.lines().count(), original.lines().count());
        assert!(!text.contains("AAAA"));
        assert_eq!(text.lines().last(), Some("depois"));
    }

    #[test]
    fn texto_sem_credencial_fica_igual() {
        let original = "#5 [build 2/4] RUN npm ci\nsha256:0123456789abcdef";
        assert_eq!(mask_credentials(original), (original.to_string(), false));
    }
}
