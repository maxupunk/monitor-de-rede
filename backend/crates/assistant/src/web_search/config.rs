//! Configuração da pesquisa web, lida do ambiente uma vez por processo.
//!
//! - `AI_WEB_SEARCH=off` desliga a ferramenta: ela some do catálogo da IA
//!   (chat, rotinas automáticas e tarefas agendadas), sem custar tokens.
//! - `AI_WEB_SEARCH_URL` aponta um SearXNG (próprio ou da empresa). Definida,
//!   ela é o **único** buscador geral: quem configurou um SearXNG normalmente
//!   quer que as pesquisas não saiam direto para um buscador público.

use std::sync::OnceLock;

use reqwest::Url;

pub const ENABLED_VAR: &str = "AI_WEB_SEARCH";
pub const SEARXNG_VAR: &str = "AI_WEB_SEARCH_URL";

/// O buscador geral (o NVD entra à parte, só para CVE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebEngine {
    /// Padrão: não exige chave nem instalação.
    DuckDuckGo,
    /// Endpoint `…/search` de um SearXNG com o formato JSON habilitado.
    Searxng(Url),
    /// `AI_WEB_SEARCH_URL` definida, mas inválida: a pesquisa avisa em vez de
    /// cair num buscador público que o administrador quis evitar.
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchConfig {
    pub enabled: bool,
    pub engine: WebEngine,
}

impl WebSearchConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self::parse(
            std::env::var(ENABLED_VAR).ok().as_deref(),
            std::env::var(SEARXNG_VAR).ok().as_deref(),
        )
    }

    /// As duas variáveis já lidas — puro, para teste.
    #[must_use]
    pub fn parse(enabled: Option<&str>, searxng: Option<&str>) -> Self {
        let enabled = !enabled.is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "false" | "0" | "no" | "off"
            )
        });
        let engine = match searxng.map(str::trim).filter(|value| !value.is_empty()) {
            None => WebEngine::DuckDuckGo,
            Some(raw) => searxng_endpoint(raw).map_or_else(WebEngine::Invalid, WebEngine::Searxng),
        };
        Self { enabled, engine }
    }
}

/// A configuração do processo.
pub fn current() -> &'static WebSearchConfig {
    static CONFIG: OnceLock<WebSearchConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let config = WebSearchConfig::from_env();
        if let WebEngine::Invalid(reason) = &config.engine {
            tracing::error!(%reason, "pesquisa web da IA: {SEARXNG_VAR} inválida");
        }
        config
    })
}

/// Aceita a raiz do SearXNG (`http://searxng:8080`) ou o próprio `/search`.
fn searxng_endpoint(raw: &str) -> Result<Url, String> {
    let mut url = Url::parse(raw).map_err(|error| {
        format!("{SEARXNG_VAR}=\"{raw}\" não é uma URL ({error}); use, por exemplo, http://searxng:8080")
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!(
            "{SEARXNG_VAR}=\"{raw}\" precisa ser http:// ou https:// com o endereço do SearXNG"
        ));
    }
    if !url.path().trim_end_matches('/').ends_with("/search") {
        let path = format!("{}/search", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligada_por_padrao_e_desligada_so_por_valor_explicito() {
        assert!(WebSearchConfig::parse(None, None).enabled);
        assert!(WebSearchConfig::parse(Some("on"), None).enabled);
        assert!(WebSearchConfig::parse(Some("qualquer"), None).enabled);
        for off in ["off", "OFF", " false ", "0", "no"] {
            assert!(!WebSearchConfig::parse(Some(off), None).enabled, "{off}");
        }
    }

    #[test]
    fn sem_searxng_usa_o_duckduckgo() {
        assert_eq!(
            WebSearchConfig::parse(None, None).engine,
            WebEngine::DuckDuckGo
        );
        assert_eq!(
            WebSearchConfig::parse(None, Some("  ")).engine,
            WebEngine::DuckDuckGo
        );
    }

    #[test]
    fn searxng_aceita_a_raiz_ou_o_endpoint() {
        for raw in [
            "http://searxng:8080",
            "http://searxng:8080/",
            "http://searxng:8080/search",
            "http://searxng:8080/search?q=x#y",
        ] {
            let WebEngine::Searxng(url) = WebSearchConfig::parse(None, Some(raw)).engine else {
                panic!("{raw}");
            };
            assert_eq!(url.as_str(), "http://searxng:8080/search", "{raw}");
        }
        let WebEngine::Searxng(url) =
            WebSearchConfig::parse(None, Some("https://busca.empresa.com/searx/")).engine
        else {
            panic!("subcaminho");
        };
        assert_eq!(url.as_str(), "https://busca.empresa.com/searx/search");
    }

    #[test]
    fn searxng_invalido_nao_cai_no_buscador_publico() {
        for raw in ["searxng:8080", "ftp://searxng", "não é url"] {
            assert!(
                matches!(
                    WebSearchConfig::parse(None, Some(raw)).engine,
                    WebEngine::Invalid(_)
                ),
                "{raw}"
            );
        }
    }
}
