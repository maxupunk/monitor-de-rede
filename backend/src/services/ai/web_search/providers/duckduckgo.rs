//! DuckDuckGo pela versão HTML (`html.duckduckgo.com`): não pede chave nem
//! conta, e a página é estável o bastante para ler título, link e trecho.
//!
//! Anúncios (`y.js`) ficam de fora e o link de redirecionamento (`/l/?uddg=`)
//! vira o endereço de destino. Quando o DuckDuckGo pede a verificação
//! anti-robô, a resposta diz isso e aponta o SearXNG como saída.

use std::sync::LazyLock;

use async_trait::async_trait;
use regex::Regex;
use reqwest::{StatusCode, Url};

use super::{fetch, rejected_status, ProviderError, SearchProvider};
use crate::services::{
    ai::web_search::{query::SearchQuery, untrusted::RawResult},
    shared::text::decode_html_entities,
};

pub const DEFAULT_ENDPOINT: &str = "https://html.duckduckgo.com/html/";
const SOURCE: &str = "DuckDuckGo";

static RESULT_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<a\b([^>]*\bclass="[^"]*\bresult__a\b[^"]*"[^>]*)>(.*?)</a>"#)
        .expect("regex do link do resultado")
});
static RESULT_SNIPPET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<(a|td|div)\b[^>]*\bclass="[^"]*\bresult__snippet\b[^"]*"[^>]*>(.*?)</(?:a|td|div)>"#)
        .expect("regex do trecho do resultado")
});
static HREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\bhref="([^"]*)""#).expect("regex do href"));

pub struct DuckDuckGo {
    endpoint: Url,
}

impl DuckDuckGo {
    #[must_use]
    pub const fn new(endpoint: Url) -> Self {
        Self { endpoint }
    }
}

impl Default for DuckDuckGo {
    fn default() -> Self {
        Self::new(Url::parse(DEFAULT_ENDPOINT).expect("endpoint do DuckDuckGo"))
    }
}

#[async_trait]
impl SearchProvider for DuckDuckGo {
    fn id(&self) -> &'static str {
        "duckduckgo"
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<RawResult>, ProviderError> {
        let mut url = self.endpoint.clone();
        url.query_pairs_mut().append_pair("q", &query.text());
        let fetched = fetch(SOURCE, url, "text/html").await?;
        if is_bot_challenge(fetched.status, &fetched.body) {
            return Err(ProviderError::Rejected(format!(
                "o {SOURCE} pediu verificação anti-robô; para pesquisas estáveis, aponte um SearXNG em AI_WEB_SEARCH_URL"
            )));
        }
        if !fetched.status.is_success() {
            return Err(rejected_status(SOURCE, fetched.status));
        }
        Ok(parse(&fetched.body))
    }
}

/// O DuckDuckGo responde 202 (ou 200 com o desafio) quando desconfia de robô.
fn is_bot_challenge(status: StatusCode, body: &str) -> bool {
    status == StatusCode::ACCEPTED
        || body.contains("anomaly-modal")
        || body.contains("challenge-form")
}

/// Título, link e trecho de cada resultado orgânico, na ordem da página.
fn parse(html: &str) -> Vec<RawResult> {
    let links: Vec<_> = RESULT_LINK.captures_iter(html).collect();
    links
        .iter()
        .enumerate()
        .filter_map(|(index, link)| {
            let whole = link.get(0)?;
            let next_start = links
                .get(index + 1)
                .and_then(|next| next.get(0))
                .map_or(html.len(), |next| next.start());
            let href = HREF.captures(&link[1])?.get(1)?.as_str();
            let url = target_url(href)?;
            let snippet = RESULT_SNIPPET
                .captures(&html[whole.end()..next_start])
                .and_then(|captures| captures.get(2))
                .map_or_else(String::new, |found| found.as_str().to_string());
            Some(RawResult {
                title: link[2].to_string(),
                url,
                snippet,
            })
        })
        .collect()
}

/// O destino do link: o `uddg` do redirecionamento, ou o próprio href.
/// Anúncio (`/y.js`) não é resultado.
fn target_url(href: &str) -> Option<String> {
    let href = decode_html_entities(href);
    let absolute = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href
    };
    let url = Url::parse(&absolute).ok()?;
    let is_duckduckgo = url
        .host_str()
        .is_some_and(|host| host == "duckduckgo.com" || host.ends_with(".duckduckgo.com"));
    if !is_duckduckgo {
        return Some(url.to_string());
    }
    if url.path().starts_with("/y.js") {
        return None;
    }
    url.query_pairs()
        .find(|(key, _)| key == "uddg")
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorte fiel da página HTML do DuckDuckGo: um anúncio e dois resultados.
    const PAGE: &str = r#"
<div class="result results_links results_links_deep result--ad">
  <div class="links_main links_deep result__body">
    <h2 class="result__title"><a rel="nofollow" class="result__a" href="https://duckduckgo.com/y.js?ad_domain=x.com&amp;u3=1">Anúncio</a></h2>
    <a class="result__snippet" href="https://duckduckgo.com/y.js?ad_domain=x.com">Compre já</a>
  </div>
</div>
<div class="result results_links results_links_deep web-result">
  <div class="links_main links_deep result__body">
    <h2 class="result__title">
      <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fforum.mikrotik.com%2Fviewtopic.php%3Ft%3D208123&amp;rut=abc"><b>RouterOS</b> 7.15.2 BGP hold timer expired</a>
    </h2>
    <a class="result__snippet" href="//duckduckgo.com/l/?uddg=x">Confirmed regression with <b>BFD</b>; fixed in 7.15.3.</a>
  </div>
</div>
<div class="result results_links results_links_deep web-result">
  <div class="links_main links_deep result__body">
    <h2 class="result__title"><a rel="nofollow" class="result__a" href="https://help.mikrotik.com/docs/display/ROS/BGP">BGP - RouterOS</a></h2>
  </div>
</div>
"#;

    #[test]
    fn le_resultados_organicos_e_pula_anuncio() {
        let results = parse(PAGE);
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].url,
            "https://forum.mikrotik.com/viewtopic.php?t=208123"
        );
        assert_eq!(
            results[0].title,
            "<b>RouterOS</b> 7.15.2 BGP hold timer expired"
        );
        assert_eq!(
            results[0].snippet,
            "Confirmed regression with <b>BFD</b>; fixed in 7.15.3."
        );
        assert_eq!(
            results[1].url,
            "https://help.mikrotik.com/docs/display/ROS/BGP"
        );
        assert!(
            results[1].snippet.is_empty(),
            "o trecho do vizinho não é emprestado"
        );
    }

    #[test]
    fn pagina_sem_resultado_nao_e_erro() {
        assert!(parse("<html><body>No results.</body></html>").is_empty());
    }

    #[test]
    fn reconhece_o_desafio_anti_robo() {
        assert!(is_bot_challenge(StatusCode::ACCEPTED, ""));
        assert!(is_bot_challenge(
            StatusCode::OK,
            "<div class=\"anomaly-modal\">"
        ));
        assert!(!is_bot_challenge(StatusCode::OK, PAGE));
    }
}
