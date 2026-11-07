//! NVD (NIST): a descrição e a gravidade oficiais de um CVE citado no termo.
//!
//! O buscador geral acha discussão e correção; o NVD dá o fato — o que é a
//! falha, a nota CVSS e quando foi publicada. Só entra quando o termo tem um
//! `CVE-AAAA-NNNN`, e consulta um por vez (o limite público é de poucas
//! requisições por janela; o cache poupa o resto).

use async_trait::async_trait;
use reqwest::{StatusCode, Url};
use serde::Deserialize;

use super::{fetch, rejected_status, ProviderError, SearchProvider};
use crate::web_search::{query::SearchQuery, untrusted::RawResult};

pub const DEFAULT_ENDPOINT: &str = "https://services.nvd.nist.gov/rest/json/cves/2.0";
const DETAIL_URL: &str = "https://nvd.nist.gov/vuln/detail/";
const SOURCE: &str = "NVD";

pub struct Nvd {
    endpoint: Url,
}

impl Nvd {
    #[must_use]
    pub const fn new(endpoint: Url) -> Self {
        Self { endpoint }
    }

    async fn lookup(&self, id: &str) -> Result<Option<RawResult>, ProviderError> {
        let mut url = self.endpoint.clone();
        url.query_pairs_mut().append_pair("cveId", id);
        let fetched = fetch(SOURCE, url, "application/json").await?;
        if fetched.status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !fetched.status.is_success() {
            return Err(rejected_status(SOURCE, fetched.status));
        }
        parse(&fetched.body)
    }
}

impl Default for Nvd {
    fn default() -> Self {
        Self::new(Url::parse(DEFAULT_ENDPOINT).expect("endpoint do NVD"))
    }
}

#[async_trait]
impl SearchProvider for Nvd {
    fn id(&self) -> &'static str {
        "nvd"
    }

    fn applies(&self, query: &SearchQuery) -> bool {
        !query.cve_ids().is_empty()
    }

    /// Falha só quando nenhum CVE pôde ser consultado; CVE inexistente
    /// simplesmente não aparece.
    async fn search(&self, query: &SearchQuery) -> Result<Vec<RawResult>, ProviderError> {
        let mut results = Vec::new();
        let mut last_error = None;
        let mut answered = false;
        for id in query.cve_ids() {
            match self.lookup(&id).await {
                Ok(found) => {
                    answered = true;
                    results.extend(found);
                }
                Err(error) => last_error = Some(error),
            }
        }
        match last_error {
            Some(error) if !answered => Err(error),
            _ => Ok(results),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    vulnerabilities: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    cve: Cve,
}

#[derive(Debug, Deserialize)]
struct Cve {
    id: String,
    #[serde(default)]
    published: Option<String>,
    #[serde(default)]
    descriptions: Vec<Description>,
    #[serde(default)]
    metrics: Metrics,
}

#[derive(Debug, Deserialize)]
struct Description {
    lang: String,
    value: String,
}

/// As versões do CVSS, da mais nova para a mais antiga.
#[derive(Debug, Default, Deserialize)]
struct Metrics {
    #[serde(rename = "cvssMetricV40", default)]
    v40: Vec<Metric>,
    #[serde(rename = "cvssMetricV31", default)]
    v31: Vec<Metric>,
    #[serde(rename = "cvssMetricV30", default)]
    v30: Vec<Metric>,
    #[serde(rename = "cvssMetricV2", default)]
    v2: Vec<Metric>,
}

#[derive(Debug, Deserialize)]
struct Metric {
    #[serde(rename = "cvssData")]
    data: CvssData,
    /// No CVSS v2 a gravidade fica fora do `cvssData`.
    #[serde(rename = "baseSeverity", default)]
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CvssData {
    #[serde(rename = "baseScore")]
    score: f64,
    #[serde(rename = "baseSeverity", default)]
    severity: Option<String>,
}

impl Metrics {
    /// "CVSS 8.6 HIGH" da versão mais nova disponível.
    fn summary(&self) -> Option<String> {
        let metric = [&self.v40, &self.v31, &self.v30, &self.v2]
            .into_iter()
            .find_map(|metrics| metrics.first())?;
        let severity = metric
            .data
            .severity
            .as_deref()
            .or(metric.severity.as_deref())
            .map(|severity| format!(" {severity}"))
            .unwrap_or_default();
        Some(format!("CVSS {:.1}{severity}", metric.data.score))
    }
}

fn parse(body: &str) -> Result<Option<RawResult>, ProviderError> {
    let response: Response = serde_json::from_str(body).map_err(|_| {
        ProviderError::Rejected(format!("o {SOURCE} devolveu uma resposta ilegível"))
    })?;
    Ok(response.vulnerabilities.into_iter().next().map(|item| {
        let cve = item.cve;
        let title = match cve.metrics.summary() {
            Some(score) => format!("{} — {score}", cve.id),
            None => cve.id.clone(),
        };
        let description = cve
            .descriptions
            .iter()
            .find(|description| description.lang == "en")
            .or_else(|| cve.descriptions.first())
            .map_or("", |description| description.value.as_str());
        let snippet = match cve.published.as_deref().and_then(|date| date.get(..10)) {
            Some(date) => format!("Publicado em {date}. {description}"),
            None => description.to_string(),
        };
        RawResult {
            url: format!("{DETAIL_URL}{}", cve.id),
            title,
            snippet,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_descricao_e_a_nota_mais_nova() {
        let body = r#"{"resultsPerPage":1,"vulnerabilities":[{"cve":{
            "id":"CVE-2024-21626","published":"2024-01-31T22:15:53.780",
            "descriptions":[{"lang":"es","value":"runc …"},{"lang":"en","value":"runc is a CLI tool for spawning containers."}],
            "metrics":{
              "cvssMetricV31":[{"cvssData":{"baseScore":8.6,"baseSeverity":"HIGH"}}],
              "cvssMetricV2":[{"cvssData":{"baseScore":6.8},"baseSeverity":"MEDIUM"}]
            }}}]}"#;
        let result = parse(body).unwrap().unwrap();
        assert_eq!(result.title, "CVE-2024-21626 — CVSS 8.6 HIGH");
        assert_eq!(
            result.url,
            "https://nvd.nist.gov/vuln/detail/CVE-2024-21626"
        );
        assert_eq!(
            result.snippet,
            "Publicado em 2024-01-31. runc is a CLI tool for spawning containers."
        );
    }

    #[test]
    fn cvss_v2_tem_a_gravidade_fora_dos_dados() {
        let body = r#"{"vulnerabilities":[{"cve":{"id":"CVE-2010-0001",
            "descriptions":[{"lang":"en","value":"x"}],
            "metrics":{"cvssMetricV2":[{"cvssData":{"baseScore":5.0},"baseSeverity":"MEDIUM"}]}}}]}"#;
        let result = parse(body).unwrap().unwrap();
        assert_eq!(result.title, "CVE-2010-0001 — CVSS 5.0 MEDIUM");
        assert_eq!(result.snippet, "x");
    }

    #[test]
    fn cve_inexistente_nao_e_erro() {
        assert_eq!(
            parse(r#"{"totalResults":0,"vulnerabilities":[]}"#).unwrap(),
            None
        );
        assert!(parse("<html>").is_err());
    }
}
