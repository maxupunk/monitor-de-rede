//! Pesquisa web da IA (`web_search`), feita pela própria central.
//!
//! Serve ao chat e às rotinas sem tela — resumo de incidente, resumo
//! periódico, tarefas agendadas —, dentro da mesma rodada do agente: a IA
//! pesquisa e segue o raciocínio sem devolver a vez a ninguém.
//!
//! O caminho de uma pesquisa:
//!
//! 1. [`query::SearchQuery`] tira do termo IP interno, MAC, credencial e
//!    chave — nada disso sai da rede — e valida o site pedido;
//! 2. o [`cache`] responde o que foi pesquisado há pouco;
//! 3. sem internet há pouco, responde na hora que está sem internet, em vez
//!    de cada pergunta esperar o prazo de novo;
//! 4. as fontes que se aplicam ([`providers`]) consultam ao mesmo tempo, com
//!    prazo curto;
//! 5. [`untrusted`] transforma o que voltou em texto puro e curto, e os
//!    resultados se juntam sem repetir link.
//!
//! A central nunca abre os links dos resultados: só repassa título, endereço
//! e trecho. Não há, portanto, como a IA usar a pesquisa para alcançar um
//! endereço interno.

pub mod cache;
pub mod config;
pub mod providers;
pub mod query;
pub mod untrusted;

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use futures::future::join_all;
use serde::Serialize;

use self::{
    cache::TtlCache,
    config::{WebEngine, WebSearchConfig},
    providers::{
        duckduckgo::DuckDuckGo, nvd::Nvd, searxng::Searxng, Misconfigured, ProviderError,
        SearchProvider,
    },
    query::{Redaction, SearchQuery},
    untrusted::WebResult,
};
use crate::services::shared::errors::{AppError, AppResult};

/// Resultados por pesquisa: o bastante para cruzar fontes, pouco para a
/// janela de um modelo local.
pub const MAX_RESULTS: usize = 5;
const CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const CACHE_CAPACITY: usize = 256;
/// Sem rota para a internet, quanto tempo responder "sem internet" sem tentar.
const OFFLINE_BACKOFF: Duration = Duration::from_secs(120);
const NO_RESULTS_HINT: &str =
    "Nenhum resultado. Tente o código de erro exato, menos palavras ou sem `domain`.";

/// O que a IA recebe (e o cartão mostra) de uma pesquisa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WebSearchReport {
    /// O texto que de fato foi pesquisado, já sem dados internos.
    pub query: String,
    pub results: Vec<WebResult>,
    /// Fontes que responderam.
    pub sources: Vec<&'static str>,
    /// O que saiu do termo antes da pesquisa.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub redacted: Vec<Redaction>,
    /// Fonte que falhou enquanto as outras responderam, ou dica de busca.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Veio da memória da última hora, sem nova ida à web.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cached: bool,
}

/// O que fica guardado: o resultado, sem o que é desta chamada.
#[derive(Debug, Clone)]
struct Answer {
    results: Vec<WebResult>,
    sources: Vec<&'static str>,
}

pub struct WebSearchService {
    /// Em ordem de prioridade: o fato oficial (NVD) antes da discussão.
    providers: Vec<Box<dyn SearchProvider>>,
    cache: TtlCache<Answer>,
    offline_until: Mutex<Option<Instant>>,
    offline_backoff: Duration,
}

impl WebSearchService {
    #[must_use]
    pub fn new(providers: Vec<Box<dyn SearchProvider>>) -> Self {
        Self {
            providers,
            cache: TtlCache::new(CACHE_TTL, CACHE_CAPACITY),
            offline_until: Mutex::new(None),
            offline_backoff: OFFLINE_BACKOFF,
        }
    }

    /// As fontes da configuração: NVD e o buscador geral escolhido.
    #[must_use]
    pub fn from_config(config: &WebSearchConfig) -> Self {
        let engine: Box<dyn SearchProvider> = match &config.engine {
            WebEngine::DuckDuckGo => Box::new(DuckDuckGo::default()),
            WebEngine::Searxng(endpoint) => Box::new(Searxng::new(endpoint.clone())),
            WebEngine::Invalid(reason) => Box::new(Misconfigured::new(reason.clone())),
        };
        Self::new(vec![Box::new(Nvd::default()), engine])
    }

    /// Pesquisa `query`.
    ///
    /// # Errors
    ///
    /// `ServiceUnavailable`, com a frase para a IA e para a tela, quando
    /// nenhuma fonte respondeu (sem internet, bloqueio ou configuração).
    pub async fn search(&self, query: &SearchQuery) -> AppResult<WebSearchReport> {
        let key = query.cache_key();
        if let Some(answer) = self.cache.get(&key) {
            return Ok(report(query, answer, Vec::new(), true));
        }
        if let Some(remaining) = self.offline_remaining() {
            return Err(AppError::service_unavailable(offline_message(
                None, remaining,
            )));
        }

        let started = Instant::now();
        let applicable: Vec<&dyn SearchProvider> = self
            .providers
            .iter()
            .map(AsRef::as_ref)
            .filter(|provider| provider.applies(query))
            .collect();
        let outcomes = join_all(
            applicable
                .iter()
                .map(|provider| async move { (provider.id(), provider.search(query).await) }),
        )
        .await;

        let mut answered = Vec::new();
        let mut failures = Vec::new();
        for (id, outcome) in outcomes {
            match outcome {
                Ok(raw) => answered.push((id, raw)),
                Err(error) => failures.push(error),
            }
        }
        tracing::debug!(
            query = %query.text(),
            answered = answered.len(),
            failed = failures.len(),
            elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "pesquisa web da IA"
        );

        if answered.is_empty() {
            return Err(self.unavailable(&failures));
        }
        let answer = merge(answered);
        let notes: Vec<String> = failures
            .iter()
            .map(|failure| format!("Fonte indisponível: {}", failure.message()))
            .collect();
        if notes.is_empty() {
            self.cache.insert(key, answer.clone());
        }
        Ok(report(query, answer, notes, false))
    }

    /// Nenhuma fonte respondeu. Sem rota para a internet, a central para de
    /// tentar por um tempo; recusa é dita como veio, com o que fazer.
    fn unavailable(&self, failures: &[ProviderError]) -> AppError {
        let offline = !failures.is_empty()
            && failures
                .iter()
                .all(|failure| matches!(failure, ProviderError::Unreachable(_)));
        if offline {
            if let Ok(mut until) = self.offline_until.lock() {
                *until = Some(Instant::now() + self.offline_backoff);
            }
            return AppError::service_unavailable(offline_message(
                failures.first().map(ProviderError::message),
                self.offline_backoff,
            ));
        }
        let reasons: Vec<&str> = failures.iter().map(ProviderError::message).collect();
        AppError::service_unavailable(format!(
            "A pesquisa web não trouxe resultado ({}). Siga o diagnóstico com os dados do NetMonitor.",
            reasons.join("; ")
        ))
    }

    fn offline_remaining(&self) -> Option<Duration> {
        let until = (*self.offline_until.lock().ok()?)?;
        until.checked_duration_since(Instant::now())
    }
}

fn offline_message(detail: Option<&str>, retry_in: Duration) -> String {
    let detail = detail
        .map(|detail| format!(" ({detail})"))
        .unwrap_or_default();
    format!(
        "A central está sem acesso à internet para pesquisar agora{detail}. Siga o diagnóstico com os dados do NetMonitor; a pesquisa volta a ser tentada em até {} s.",
        retry_in.as_secs().max(1)
    )
}

/// Junta as fontes na ordem de prioridade, limpa cada resultado e corta em
/// [`MAX_RESULTS`] sem repetir link.
fn merge(answered: Vec<(&'static str, Vec<untrusted::RawResult>)>) -> Answer {
    let mut seen = HashSet::new();
    let mut sources = Vec::new();
    let mut results = Vec::new();
    for (id, raw) in answered {
        sources.push(id);
        results.extend(
            raw.iter()
                .filter_map(WebResult::from_raw)
                .filter(|result| seen.insert(result.url.clone())),
        );
    }
    results.truncate(MAX_RESULTS);
    Answer { results, sources }
}

fn report(
    query: &SearchQuery,
    answer: Answer,
    mut notes: Vec<String>,
    cached: bool,
) -> WebSearchReport {
    if answer.results.is_empty() {
        notes.push(NO_RESULTS_HINT.to_string());
    }
    WebSearchReport {
        query: query.text(),
        results: answer.results,
        sources: answer.sources,
        redacted: query.redactions().to_vec(),
        notes,
        cached,
    }
}

/// A instância do processo, montada da configuração do ambiente.
fn service() -> &'static WebSearchService {
    static SERVICE: OnceLock<WebSearchService> = OnceLock::new();
    SERVICE.get_or_init(|| WebSearchService::from_config(config::current()))
}

/// A pesquisa web está ligada nesta instalação (`AI_WEB_SEARCH`)?
#[must_use]
pub fn is_enabled() -> bool {
    config::current().enabled
}

/// Pesquisa `terms` (opcionalmente só em `domain`) com as fontes do processo.
///
/// # Errors
///
/// `Validation` quando o termo não serve (vazio, só dado interno) ou o
/// domínio não é um site público; `ServiceUnavailable` quando nenhuma fonte
/// respondeu. As duas mensagens são para a IA corrigir ou seguir sem a web.
pub async fn search(terms: &str, domain: Option<&str>) -> AppResult<WebSearchReport> {
    let query = SearchQuery::parse(terms, domain)?;
    service().search(&query).await
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use async_trait::async_trait;

    use super::*;
    use crate::services::ai::web_search::untrusted::RawResult;

    /// Fonte de teste: responde o combinado e conta as chamadas.
    struct Fake {
        id: &'static str,
        only_cve: bool,
        outcome: Result<Vec<RawResult>, ProviderError>,
        calls: Arc<AtomicUsize>,
    }

    impl Fake {
        fn new(
            id: &'static str,
            outcome: Result<Vec<RawResult>, ProviderError>,
        ) -> (Self, Arc<AtomicUsize>) {
            let calls = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    id,
                    only_cve: false,
                    outcome,
                    calls: Arc::clone(&calls),
                },
                calls,
            )
        }
    }

    #[async_trait]
    impl SearchProvider for Fake {
        fn id(&self) -> &'static str {
            self.id
        }

        fn applies(&self, query: &SearchQuery) -> bool {
            !self.only_cve || !query.cve_ids().is_empty()
        }

        async fn search(&self, _query: &SearchQuery) -> Result<Vec<RawResult>, ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.outcome.clone()
        }
    }

    fn raw(title: &str, url: &str) -> RawResult {
        RawResult {
            title: title.into(),
            url: url.into(),
            snippet: format!("trecho de <b>{title}</b>"),
        }
    }

    fn query(terms: &str) -> SearchQuery {
        SearchQuery::parse(terms, None).unwrap()
    }

    fn unreachable() -> ProviderError {
        ProviderError::Unreachable("a central não conseguiu conectar ao DuckDuckGo".into())
    }

    #[tokio::test]
    async fn junta_fontes_na_ordem_sem_repetir_link_e_guarda_no_cache() {
        let (mut nvd, nvd_calls) = Fake::new(
            "nvd",
            Ok(vec![raw("CVE-2024-21626", "https://nvd.nist.gov/x")]),
        );
        nvd.only_cve = true;
        let web_results = (0..6)
            .map(|index| raw(&format!("r{index}"), &format!("https://site{index}.com/")))
            .chain([raw("repetido", "https://nvd.nist.gov/x")])
            .collect();
        let (web, web_calls) = Fake::new("duckduckgo", Ok(web_results));
        let service = WebSearchService::new(vec![Box::new(nvd), Box::new(web)]);

        let first = service
            .search(&query("runc CVE-2024-21626 10.0.0.5"))
            .await
            .unwrap();
        assert_eq!(first.results.len(), MAX_RESULTS);
        assert_eq!(first.results[0].title, "CVE-2024-21626");
        assert_eq!(
            first.results[1].snippet, "trecho de r0",
            "HTML sai do trecho"
        );
        assert_eq!(first.sources, vec!["nvd", "duckduckgo"]);
        assert_eq!(first.redacted, vec![Redaction::InternalAddress]);
        assert_eq!(first.query, "runc CVE-2024-21626");
        assert!(!first.cached);

        let again = service
            .search(&query("RUNC  cve-2024-21626 10.0.0.5"))
            .await
            .unwrap();
        assert!(again.cached, "mesma pesquisa com outra caixa e espaços");
        assert_eq!(again.results, first.results);
        assert_eq!(again.redacted, vec![Redaction::InternalAddress]);
        assert_eq!(nvd_calls.load(Ordering::SeqCst), 1);
        assert_eq!(web_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fonte_que_nao_se_aplica_nem_e_chamada() {
        let (mut nvd, nvd_calls) = Fake::new("nvd", Ok(vec![]));
        nvd.only_cve = true;
        let (web, _) = Fake::new("duckduckgo", Ok(vec![raw("a", "https://a.com")]));
        let service = WebSearchService::new(vec![Box::new(nvd), Box::new(web)]);
        let report = service.search(&query("bgp flap routeros")).await.unwrap();
        assert_eq!(nvd_calls.load(Ordering::SeqCst), 0);
        assert_eq!(report.sources, vec!["duckduckgo"]);
    }

    #[tokio::test]
    async fn falha_parcial_vira_nota_e_nao_fica_no_cache() {
        let (mut nvd, nvd_calls) = Fake::new(
            "nvd",
            Err(ProviderError::Rejected("NVD respondeu 503".into())),
        );
        nvd.only_cve = true;
        let (web, _) = Fake::new("duckduckgo", Ok(vec![raw("a", "https://a.com")]));
        let service = WebSearchService::new(vec![Box::new(nvd), Box::new(web)]);
        let report = service.search(&query("CVE-2024-21626")).await.unwrap();
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.notes, vec!["Fonte indisponível: NVD respondeu 503"]);
        service.search(&query("CVE-2024-21626")).await.unwrap();
        assert_eq!(
            nvd_calls.load(Ordering::SeqCst),
            2,
            "resultado parcial não é guardado"
        );
    }

    #[tokio::test]
    async fn sem_resultado_traz_dica() {
        let (web, _) = Fake::new("duckduckgo", Ok(vec![]));
        let service = WebSearchService::new(vec![Box::new(web)]);
        let report = service.search(&query("termo raro")).await.unwrap();
        assert!(report.results.is_empty());
        assert_eq!(report.notes, vec![NO_RESULTS_HINT]);
    }

    #[tokio::test]
    async fn sem_internet_para_de_tentar_por_um_tempo() {
        let (web, calls) = Fake::new("duckduckgo", Err(unreachable()));
        let service = WebSearchService::new(vec![Box::new(web)]);

        let error = service.search(&query("bgp flap")).await.unwrap_err();
        assert!(matches!(error, AppError::ServiceUnavailable(_)));
        assert!(
            error.to_string().contains("sem acesso à internet"),
            "{error}"
        );
        assert!(error.to_string().contains("DuckDuckGo"), "{error}");

        let again = service.search(&query("outro termo")).await.unwrap_err();
        assert!(again.to_string().contains("sem acesso à internet"));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a segunda pergunta não esperou o prazo"
        );
    }

    #[tokio::test]
    async fn backoff_vencido_tenta_de_novo() {
        let (web, calls) = Fake::new("duckduckgo", Err(unreachable()));
        let mut service = WebSearchService::new(vec![Box::new(web)]);
        service.offline_backoff = Duration::ZERO;
        service.search(&query("bgp flap")).await.unwrap_err();
        service.search(&query("bgp flap")).await.unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn recusa_explica_e_nao_liga_o_modo_sem_internet() {
        let (web, calls) = Fake::new(
            "duckduckgo",
            Err(ProviderError::Rejected(
                "o DuckDuckGo pediu verificação anti-robô".into(),
            )),
        );
        let service = WebSearchService::new(vec![Box::new(web)]);
        let error = service.search(&query("bgp flap")).await.unwrap_err();
        assert!(error.to_string().contains("anti-robô"), "{error}");
        service.search(&query("bgp flap")).await.unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn searxng_invalido_avisa_sem_cair_no_buscador_publico() {
        let config = WebSearchConfig::parse(None, Some("ftp://searxng"));
        let service = WebSearchService::from_config(&config);
        let error = service.search(&query("bgp flap")).await.unwrap_err();
        assert!(error.to_string().contains("AI_WEB_SEARCH_URL"), "{error}");
    }

    #[test]
    fn relatorio_omite_o_que_esta_vazio() {
        let report = WebSearchReport {
            query: "bgp".into(),
            results: vec![],
            sources: vec!["duckduckgo"],
            redacted: vec![],
            notes: vec![],
            cached: false,
        };
        let value = serde_json::to_value(report).unwrap();
        assert!(value.get("redacted").is_none());
        assert!(value.get("notes").is_none());
        assert!(value.get("cached").is_none());
    }
}
