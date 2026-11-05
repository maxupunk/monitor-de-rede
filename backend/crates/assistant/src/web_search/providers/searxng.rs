//! SearXNG: metabuscador que a própria empresa hospeda. A central fala com
//! ele em vez de com um buscador público — o que costuma ser exatamente o
//! motivo de alguém configurá-lo (`AI_WEB_SEARCH_URL`).

use async_trait::async_trait;
use reqwest::{StatusCode, Url};
use serde::Deserialize;

use super::{fetch, rejected_status, ProviderError, SearchProvider};
use crate::web_search::{query::SearchQuery, untrusted::RawResult};

const SOURCE: &str = "SearXNG";

pub struct Searxng {
    /// O `…/search` já normalizado pela configuração.
    endpoint: Url,
}

impl Searxng {
    #[must_use]
    pub const fn new(endpoint: Url) -> Self {
        Self { endpoint }
    }
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    results: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

#[async_trait]
impl SearchProvider for Searxng {
    fn id(&self) -> &'static str {
        "searxng"
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<RawResult>, ProviderError> {
        let mut url = self.endpoint.clone();
        url.query_pairs_mut()
            .append_pair("q", &query.text())
            .append_pair("format", "json");
        let fetched = fetch(SOURCE, url, "application/json").await?;
        if fetched.status == StatusCode::FORBIDDEN {
            return Err(ProviderError::Rejected(format!(
                "o {SOURCE} recusou o formato JSON; habilite `json` em `search.formats` no settings.yml dele"
            )));
        }
        if !fetched.status.is_success() {
            return Err(rejected_status(SOURCE, fetched.status));
        }
        parse(&fetched.body)
    }
}

fn parse(body: &str) -> Result<Vec<RawResult>, ProviderError> {
    let response: Response = serde_json::from_str(body).map_err(|_| {
        ProviderError::Rejected(format!(
            "o {SOURCE} não devolveu JSON; confira se AI_WEB_SEARCH_URL aponta para o SearXNG"
        ))
    })?;
    Ok(response
        .results
        .into_iter()
        .map(|item| RawResult {
            title: item.title,
            url: item.url,
            snippet: item.content,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_json_do_searxng() {
        let body = r#"{"query":"x","results":[
            {"title":"BGP - RouterOS","url":"https://help.mikrotik.com/docs/BGP","content":"Hold time…","engine":"bing"},
            {"title":"Sem trecho","url":"https://a.com"}
        ]}"#;
        let results = parse(body).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].url, "https://help.mikrotik.com/docs/BGP");
        assert_eq!(results[0].snippet, "Hold time…");
        assert!(results[1].snippet.is_empty());
    }

    #[test]
    fn pagina_html_no_lugar_do_json_explica_o_que_conferir() {
        let error = parse("<html>login</html>").unwrap_err();
        assert!(error.message().contains("AI_WEB_SEARCH_URL"));
    }
}
