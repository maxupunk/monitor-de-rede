//! O termo pesquisado: o que a IA pediu, sem o que não pode sair da rede.
//!
//! O modelo monta a busca a partir de logs e alertas, e log carrega IP
//! interno, MAC, usuário e às vezes senha. Tudo isso sai do termo **antes** de
//! qualquer requisição, e a lista do que saiu volta junto do resultado — a IA
//! e quem lê o cartão sabem que a busca foi feita sem aqueles dados.

use std::{collections::BTreeSet, net::IpAddr, sync::LazyLock};

use regex::{Captures, Regex};
use serde::Serialize;

use crate::services::{
    ai::harness::grounding::LOCAL_SUFFIXES,
    shared::{
        cidr::is_internal_address,
        errors::{AppError, AppResult},
        text::collapse_whitespace,
    },
};

/// Buscadores cortam termos longos; mensagem de log inteira só atrapalha.
pub const MAX_TERMS_CHARS: usize = 300;
/// CVEs consultados no NVD por pesquisa.
pub const MAX_CVE_IDS: usize = 3;
/// Abaixo disso, o termo que sobrou não identifica nada.
const MIN_TERMS_CHARS: usize = 3;
/// Sequência opaca (chave, hash, token) a partir deste tamanho.
const OPAQUE_TOKEN_CHARS: usize = 32;

/// O que foi tirado do termo antes de pesquisar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Redaction {
    InternalAddress,
    MacAddress,
    Credential,
    Email,
    OpaqueToken,
}

impl Redaction {
    /// Como aparece para a IA e no cartão.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InternalAddress => "IP interno",
            Self::MacAddress => "endereço MAC",
            Self::Credential => "credencial",
            Self::Email => "e-mail",
            Self::OpaqueToken => "chave ou token",
        }
    }
}

impl Serialize for Redaction {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.label())
    }
}

static PEM_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)-----BEGIN [A-Z0-9 ]+-----.*?(?:-----END [A-Z0-9 ]+-----|$)")
        .expect("regex de bloco PEM")
});
static URL_USERINFO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*://)[^\s/:@]+(?::[^\s/@]*)?@").expect("regex de userinfo")
});
static SECRET_PAIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b(?:password|passwd|pwd|pass|senha|secret|segredo|token|api[_-]?key|apikey|community|comunidade|psk|(?:pre[_-]?shared|private)[_-]?key|key|auth)\s*[:=]\s*(?:"[^"]*"|'[^']*'|\S+)"#,
    )
    .expect("regex de par com segredo")
});
static AUTH_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]{8,}").expect("regex de autorização")
});
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9-]+(?:\.[a-z0-9-]+)*\.[a-z]{2,}\b")
        .expect("regex de e-mail")
});
static MAC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:[0-9a-f]{2}(?:[:-][0-9a-f]{2}){5}|[0-9a-f]{4}\.[0-9a-f]{4}\.[0-9a-f]{4})\b",
    )
    .expect("regex de MAC")
});
static IPV4: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b((?:\d{1,3}\.){3}\d{1,3})(?:/\d{1,2})?(?::\d{1,5})?\b").expect("regex de IPv4")
});
/// Candidato a IPv6 (com zona e prefixo); quem decide é o parser.
static IPV6: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[?([0-9a-f:]*:[0-9a-f:]*:[0-9a-f.:]*)(?:%[\w.-]+)?\]?(?:/\d{1,3})?")
        .expect("regex de IPv6")
});
static CVE_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bCVE-\d{4}-\d{4,7}\b").expect("regex de CVE"));
static DOMAIN_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$").expect("regex de rótulo de domínio")
});

/// A pesquisa pronta para sair: termo limpo e, opcionalmente, o site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    terms: String,
    domain: Option<String>,
    redactions: Vec<Redaction>,
}

impl SearchQuery {
    /// Limpa o termo e valida o domínio.
    ///
    /// # Errors
    ///
    /// `Validation`, com o que corrigir, quando o termo fica vazio depois da
    /// limpeza ou o domínio não é um site público.
    pub fn parse(terms: &str, domain: Option<&str>) -> AppResult<Self> {
        let (clean, redactions) = redact(terms);
        let clean: String = clean.chars().take(MAX_TERMS_CHARS).collect();
        let clean = clean.trim().to_string();
        if clean.chars().count() < MIN_TERMS_CHARS {
            let removed = if redactions.is_empty() {
                String::new()
            } else {
                format!(
                    " Depois de remover dados internos ({}), não sobrou o que pesquisar.",
                    redactions
                        .iter()
                        .map(|redaction| redaction.label())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            return Err(AppError::validation(format!(
                "Informe em `query` o termo técnico a pesquisar (mensagem de erro, código, modelo/firmware ou CVE).{removed}"
            )));
        }
        Ok(Self {
            terms: clean,
            domain: domain.map(normalize_domain).transpose()?.flatten(),
            redactions,
        })
    }

    #[must_use]
    pub fn terms(&self) -> &str {
        &self.terms
    }

    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    #[must_use]
    pub fn redactions(&self) -> &[Redaction] {
        &self.redactions
    }

    /// O texto enviado ao buscador, com o operador `site:` quando há domínio.
    #[must_use]
    pub fn text(&self) -> String {
        match &self.domain {
            Some(domain) => format!("site:{domain} {}", self.terms),
            None => self.terms.clone(),
        }
    }

    /// Chave do cache: a mesma pesquisa escrita com outra caixa ou espaços.
    #[must_use]
    pub fn cache_key(&self) -> String {
        self.text().to_lowercase()
    }

    /// CVEs citados no termo, em maiúsculas, sem repetição.
    #[must_use]
    pub fn cve_ids(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        CVE_ID
            .find_iter(&self.terms)
            .map(|found| found.as_str().to_ascii_uppercase())
            .filter(|id| seen.insert(id.clone()))
            .take(MAX_CVE_IDS)
            .collect()
    }
}

/// Remove o que identifica a rede ou dá acesso a ela. Devolve o termo e o
/// que saiu, sem repetição e em ordem estável.
fn redact(terms: &str) -> (String, Vec<Redaction>) {
    let mut found = BTreeSet::new();
    let mut text = terms.to_string();
    strip(
        &mut text,
        &mut found,
        &PEM_BLOCK,
        " ",
        Redaction::Credential,
    );
    strip(
        &mut text,
        &mut found,
        &URL_USERINFO,
        "$1",
        Redaction::Credential,
    );
    strip(
        &mut text,
        &mut found,
        &SECRET_PAIR,
        " ",
        Redaction::Credential,
    );
    strip(
        &mut text,
        &mut found,
        &AUTH_HEADER,
        " ",
        Redaction::Credential,
    );
    strip(&mut text, &mut found, &EMAIL, " ", Redaction::Email);
    strip(&mut text, &mut found, &MAC, " ", Redaction::MacAddress);
    strip_internal_addresses(&mut text, &mut found, &IPV4);
    strip_internal_addresses(&mut text, &mut found, &IPV6);

    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|word| {
            let opaque = is_opaque_token(word);
            if opaque {
                found.insert(Redaction::OpaqueToken);
            }
            !opaque
        })
        .collect();
    (
        collapse_whitespace(&words.join(" ")),
        found.into_iter().collect(),
    )
}

/// Troca toda ocorrência de `pattern` por `replacement`, anotando o tipo.
fn strip(
    text: &mut String,
    found: &mut BTreeSet<Redaction>,
    pattern: &Regex,
    replacement: &str,
    kind: Redaction,
) {
    if pattern.is_match(text) {
        found.insert(kind);
        *text = pattern.replace_all(text, replacement).into_owned();
    }
}

/// Tira os endereços internos que `pattern` (grupo 1 = o IP) encontrar; o
/// público fica — `8.8.8.8` num termo de DNS é conhecimento geral.
fn strip_internal_addresses(text: &mut String, found: &mut BTreeSet<Redaction>, pattern: &Regex) {
    let mut internal = false;
    *text = pattern
        .replace_all(text, |captures: &Captures| {
            let is_internal = captures[1]
                .parse::<IpAddr>()
                .is_ok_and(|ip| is_internal_address(ip) || ip.is_unspecified());
            internal |= is_internal;
            if is_internal {
                " ".to_string()
            } else {
                captures[0].to_string()
            }
        })
        .into_owned();
    if internal {
        found.insert(Redaction::InternalAddress);
    }
}

/// Chave, hash ou token: longo, sem espaço e misturando letras e dígitos.
fn is_opaque_token(word: &str) -> bool {
    let word =
        word.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']'));
    word.chars().count() >= OPAQUE_TOKEN_CHARS
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '-' | '='))
        && word.chars().any(|c| c.is_ascii_digit())
        && word.chars().any(|c| c.is_ascii_alphabetic())
}

/// Só o nome do site (`forum.mikrotik.com`), aceitando o que o modelo costuma
/// mandar no lugar: URL inteira, `site:`, `www.` com porta e caminho.
fn normalize_domain(raw: &str) -> AppResult<Option<String>> {
    let lower = raw.trim().to_lowercase();
    let without_operator = lower.strip_prefix("site:").unwrap_or(&lower);
    let without_scheme = without_operator
        .split_once("://")
        .map_or(without_operator, |(_, rest)| rest);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    let host = host.trim_start_matches("*.").trim_end_matches('.');
    if host.is_empty() {
        return Ok(None);
    }
    let invalid = || {
        AppError::validation(format!(
            "`domain` inválido (\"{raw}\"): informe só o site público, ex.: forum.mikrotik.com — ou omita para pesquisar em toda a web."
        ))
    };
    if host.parse::<IpAddr>().is_ok() || LOCAL_SUFFIXES.iter().any(|suffix| host.ends_with(suffix))
    {
        return Err(invalid());
    }
    let labels: Vec<&str> = host.split('.').collect();
    let tld_ok = labels.last().is_some_and(|tld| {
        tld.starts_with("xn--") || (tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
    });
    if host.len() > 253
        || labels.len() < 2
        || !tld_ok
        || !labels.iter().all(|label| DOMAIN_LABEL.is_match(label))
    {
        return Err(invalid());
    }
    Ok(Some(host.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(terms: &str) -> SearchQuery {
        SearchQuery::parse(terms, None).expect("termo válido")
    }

    #[test]
    fn termo_tecnico_passa_intacto() {
        let query = parse("  RouterOS 7.15.2   bgp-keepalive-timer-expired code 6 subcode 3 ");
        assert_eq!(
            query.terms(),
            "RouterOS 7.15.2 bgp-keepalive-timer-expired code 6 subcode 3"
        );
        assert!(query.redactions().is_empty());
    }

    #[test]
    fn ip_interno_sai_e_o_publico_fica() {
        let query = parse("timeout 10.0.0.1/24 192.168.88.1:8291 100.64.1.1 fe80::1%eth0 [fd00::2] 8.8.8.8 2001:4860:4860::8888");
        assert_eq!(query.terms(), "timeout 8.8.8.8 2001:4860:4860::8888");
        assert_eq!(query.redactions(), &[Redaction::InternalAddress]);
    }

    #[test]
    fn horario_e_versao_nao_viram_ip() {
        let query = parse("erro às 10:30:15 no firmware 15.2.4.10 code:6");
        assert_eq!(
            query.terms(),
            "erro às 10:30:15 no firmware 15.2.4.10 code:6"
        );
        assert!(query.redactions().is_empty());
    }

    #[test]
    fn credenciais_mac_email_e_chaves_saem() {
        let query = parse(
            "login failed password=Sup3r! community: publica Bearer abcdefghij123 \
             user admin@empresa.com.br mac AA:BB:CC:DD:EE:FF 0011.2233.4455 \
             https://admin:segredo@host.example.com/x wg key yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=",
        );
        assert_eq!(
            query.terms(),
            "login failed user mac https://host.example.com/x wg key"
        );
        assert_eq!(
            query.redactions(),
            &[
                Redaction::MacAddress,
                Redaction::Credential,
                Redaction::Email,
                Redaction::OpaqueToken
            ]
        );
    }

    #[test]
    fn so_dado_interno_vira_erro_explicando_o_que_saiu() {
        let error = SearchQuery::parse("10.0.0.1 192.168.0.1", None).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("IP interno"), "{message}");
        assert!(SearchQuery::parse("  ", None).is_err());
    }

    #[test]
    fn termo_longo_e_cortado() {
        let query = parse(&"palavra ".repeat(100));
        assert!(query.terms().chars().count() <= MAX_TERMS_CHARS);
    }

    #[test]
    fn dominio_aceita_url_e_vira_operador_site() {
        for raw in [
            "forum.mikrotik.com",
            "https://forum.mikrotik.com/viewtopic.php?t=1",
            "site:Forum.Mikrotik.com",
            "forum.mikrotik.com:443/",
        ] {
            let query = SearchQuery::parse("bgp flap", Some(raw)).expect(raw);
            assert_eq!(query.domain(), Some("forum.mikrotik.com"), "{raw}");
            assert_eq!(query.text(), "site:forum.mikrotik.com bgp flap");
        }
        assert_eq!(SearchQuery::parse("bgp", Some(" ")).unwrap().domain(), None);
    }

    #[test]
    fn dominio_interno_ou_ip_e_recusado() {
        for raw in [
            "10.0.0.1",
            "intranet.corp",
            "nas.local",
            "localhost",
            "a_b.com",
            "x.c0m",
        ] {
            assert!(SearchQuery::parse("bgp flap", Some(raw)).is_err(), "{raw}");
        }
    }

    #[test]
    fn cves_saem_do_termo_sem_repetir() {
        let query = parse(
            "cve-2024-21626 e CVE-2024-21626, CVE-2019-5736 CVE-2021-1 CVE-2020-1472 CVE-2023-4911",
        );
        assert_eq!(
            query.cve_ids(),
            vec!["CVE-2024-21626", "CVE-2019-5736", "CVE-2020-1472"]
        );
        assert_eq!(query.cache_key(), query.text().to_lowercase());
    }
}
