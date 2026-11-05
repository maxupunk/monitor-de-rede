//! Quem responde a pesquisa.
//!
//! Cada fonte é um [`SearchProvider`]: diz quando se aplica e devolve
//! resultados crus. Limpeza, cache, junção e mensagens ficam no serviço
//! ([`super::WebSearchService`]) — fonte nova é um tipo novo, sem mexer no
//! resto.
//!
//! - [`duckduckgo`]: buscador geral padrão, sem chave nem instalação.
//! - [`searxng`]: buscador geral próprio (`AI_WEB_SEARCH_URL`).
//! - [`nvd`]: descrição e gravidade oficiais de CVEs citados no termo.

pub mod duckduckgo;
pub mod nvd;
pub mod searxng;

use std::{sync::OnceLock, time::Duration};

use async_trait::async_trait;
use reqwest::{header, StatusCode, Url};

use super::{query::SearchQuery, untrusted::RawResult};

/// Prazo para abrir a conexão: sem rota para a internet, falha logo.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Prazo total de cada requisição — nunca segura o chat nem um resumo.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Página de resultados ou JSON de advisory cabem com folga.
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Por que uma fonte não respondeu.
///
/// A distinção decide a experiência: sem rota para a internet a central para
/// de tentar por um tempo (cada pergunta não espera o prazo de novo); uma
/// recusa (bloqueio, configuração) é dita como está, com o que fazer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// Sem conexão: DNS, rota, recusa ou prazo esgotado.
    Unreachable(String),
    /// Respondeu, mas não com resultados (status de erro, bloqueio, formato).
    Rejected(String),
}

impl ProviderError {
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Unreachable(message) | Self::Rejected(message) => message,
        }
    }
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Nome que aparece nas fontes do resultado.
    fn id(&self) -> &'static str;

    /// Só entra na pesquisa quando tem o que fazer com ela.
    fn applies(&self, _query: &SearchQuery) -> bool {
        true
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<RawResult>, ProviderError>;
}

/// Buscador geral configurado errado: responde o motivo a cada pesquisa,
/// em vez de cair num buscador que o administrador quis evitar.
pub struct Misconfigured {
    reason: String,
}

impl Misconfigured {
    #[must_use]
    pub const fn new(reason: String) -> Self {
        Self { reason }
    }
}

#[async_trait]
impl SearchProvider for Misconfigured {
    fn id(&self) -> &'static str {
        "configuração"
    }

    async fn search(&self, _query: &SearchQuery) -> Result<Vec<RawResult>, ProviderError> {
        Err(ProviderError::Rejected(self.reason.clone()))
    }
}

/// Um cliente HTTP para o processo inteiro (§11: nada de cliente por
/// requisição), com prazos curtos e poucos redirecionamentos.
fn client() -> Result<&'static reqwest::Client, ProviderError> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent(concat!(
            "NetMonitor/",
            env!("CARGO_PKG_VERSION"),
            " web_search"
        ))
        .build()
        .map_err(|error| ProviderError::Rejected(format!("cliente HTTP: {error}")))?;
    Ok(CLIENT.get_or_init(|| client))
}

/// Resposta lida: status e corpo (cortado em [`MAX_BODY_BYTES`]).
struct Fetched {
    status: StatusCode,
    body: String,
}

/// `GET` com o `Accept` dado. O corpo é lido aos pedaços até o teto: uma
/// página inesperadamente grande não ocupa a memória da central.
async fn fetch(source: &str, url: Url, accept: &str) -> Result<Fetched, ProviderError> {
    let mut response = client()?
        .get(url)
        .header(header::ACCEPT, accept)
        .send()
        .await
        .map_err(|error| request_error(source, &error))?;
    let status = response.status();
    let mut body = Vec::new();
    while body.len() < MAX_BODY_BYTES {
        match response.chunk().await {
            Ok(Some(chunk)) => body.extend_from_slice(&chunk),
            Ok(None) => break,
            Err(error) => return Err(request_error(source, &error)),
        }
    }
    body.truncate(MAX_BODY_BYTES);
    Ok(Fetched {
        status,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn request_error(source: &str, error: &reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Unreachable(format!(
            "{source} não respondeu em {} s",
            REQUEST_TIMEOUT.as_secs()
        ))
    } else if error.is_connect() {
        ProviderError::Unreachable(format!("a central não conseguiu conectar ao {source}"))
    } else {
        ProviderError::Rejected(format!("falha ao consultar o {source}: {error}"))
    }
}

/// Status de erro como recusa, com o código.
fn rejected_status(source: &str, status: StatusCode) -> ProviderError {
    ProviderError::Rejected(format!("{source} respondeu {status}"))
}

/// As fontes de verdade contra um servidor HTTP local (nada sai da máquina):
/// o pedido que montam, a leitura da resposta e a classificação das falhas.
#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    use super::{duckduckgo::DuckDuckGo, nvd::Nvd, searxng::Searxng, *};

    /// Atende um pedido com `status` e `body`; devolve a URL e a linha do pedido.
    async fn serve_once(status: &str, body: &str) -> (Url, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = oneshot::channel();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let read = socket.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]);
            let _ = sender.send(request.lines().next().unwrap_or_default().to_string());
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        (
            Url::parse(&format!("http://{address}/search")).unwrap(),
            receiver,
        )
    }

    fn query(terms: &str, domain: Option<&str>) -> SearchQuery {
        SearchQuery::parse(terms, domain).unwrap()
    }

    async fn within_test_timeout<F: std::future::Future>(future: F) -> F::Output {
        tokio::time::timeout(Duration::from_secs(5), future)
            .await
            .expect("teste de rede passou de 5 s")
    }

    #[tokio::test]
    async fn searxng_pede_json_com_o_operador_site() {
        let body =
            r#"{"results":[{"title":"BGP","url":"https://help.mikrotik.com/BGP","content":"x"}]}"#;
        let (url, request) = serve_once("200 OK", body).await;
        let results = within_test_timeout(
            Searxng::new(url).search(&query("bgp flap", Some("help.mikrotik.com"))),
        )
        .await
        .unwrap();
        assert_eq!(results.len(), 1);
        let line = request.await.unwrap();
        assert!(
            line.starts_with("GET /search?q=site%3Ahelp.mikrotik.com+bgp+flap&format=json "),
            "{line}"
        );
    }

    #[tokio::test]
    async fn searxng_sem_json_habilitado_explica_o_ajuste() {
        let (url, _) = serve_once("403 Forbidden", "").await;
        let error = within_test_timeout(Searxng::new(url).search(&query("bgp flap", None)))
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ProviderError::Rejected(message) if message.contains("search.formats"))
        );
    }

    #[tokio::test]
    async fn duckduckgo_le_a_pagina_e_trata_o_desafio() {
        let page = r#"<a rel="nofollow" class="result__a" href="https://a.com/x">A</a><a class="result__snippet">t</a>"#;
        let (url, request) = serve_once("200 OK", page).await;
        let results = within_test_timeout(DuckDuckGo::new(url).search(&query("bgp flap", None)))
            .await
            .unwrap();
        assert_eq!(results[0].url, "https://a.com/x");
        assert!(request
            .await
            .unwrap()
            .starts_with("GET /search?q=bgp+flap "));

        let (url, _) = serve_once("202 Accepted", "").await;
        let error = within_test_timeout(DuckDuckGo::new(url).search(&query("bgp flap", None)))
            .await
            .unwrap_err();
        assert!(
            matches!(&error, ProviderError::Rejected(message) if message.contains("AI_WEB_SEARCH_URL"))
        );
    }

    #[tokio::test]
    async fn nvd_consulta_o_cve_e_ignora_o_inexistente() {
        let (url, request) = serve_once("404 Not Found", "").await;
        let results = within_test_timeout(Nvd::new(url).search(&query("CVE-2099-0001", None)))
            .await
            .unwrap();
        assert!(results.is_empty());
        assert!(request
            .await
            .unwrap()
            .starts_with("GET /search?cveId=CVE-2099-0001 "));
    }

    #[tokio::test]
    async fn porta_fechada_e_sem_internet() {
        let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = closed.local_addr().unwrap();
        drop(closed);
        let url = Url::parse(&format!("http://{address}/")).unwrap();
        let error = within_test_timeout(DuckDuckGo::new(url).search(&query("bgp flap", None)))
            .await
            .unwrap_err();
        assert!(matches!(error, ProviderError::Unreachable(_)), "{error:?}");
    }
}
