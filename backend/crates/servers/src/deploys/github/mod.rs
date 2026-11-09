//! A API REST do GitHub, do jeito que o deploy contínuo usa.
//!
//! Um cliente só ([`Client`]) para tudo: os commits entre versões
//! ([`changes`]), o GitHub App criado pelo manifest ([`app`]), o que o
//! assistente descobre no repositório ([`discovery`], [`compose`]) e o PR do
//! workflow ([`publish`]). Quem decide **qual** token usar é
//! [`crate::deploys::access`]; aqui só se fala com o GitHub.
//!
//! A central nunca roda `git`: tudo é HTTP. Quem clona é o agente, com o
//! token que a central lhe passa a cada deploy.

pub mod app;
pub mod changes;
pub mod compose;
pub mod discovery;
pub mod publish;

use std::time::Duration;

use reqwest::{header, Method, RequestBuilder, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use netmonitor_core::shared::errors::AppError;

pub use changes::{changes, compare_url};

/// A API pública do GitHub.
pub const API: &str = "https://api.github.com";
const TIMEOUT: Duration = Duration::from_secs(15);
const USER_AGENT: &str = "netmonitor-deploy";
/// Itens por página nas listagens (o máximo do GitHub).
pub const PER_PAGE: usize = 100;

/// O que deu errado ao falar com o GitHub, já em palavras de quem opera.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GitHubError {
    #[error("GitHub fora de alcance: {0}")]
    Unreachable(String),
    /// 401/403: credencial recusada, sem permissão ou limite de uso.
    #[error("{0}")]
    Denied(String),
    #[error("{0}")]
    NotFound(String),
    /// 409/422 e afins: o GitHub entendeu e recusou.
    #[error("{0}")]
    Rejected(String),
    #[error("Resposta do GitHub ilegível")]
    Unreadable,
}

impl From<GitHubError> for AppError {
    fn from(error: GitHubError) -> Self {
        match error {
            GitHubError::NotFound(message) => Self::not_found(message),
            GitHubError::Denied(message) | GitHubError::Rejected(message) => {
                Self::business_rule(message)
            }
            other => Self::service_unavailable(other.to_string()),
        }
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
}

/// Um cliente da API, com ou sem token.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}

impl Client {
    /// # Errors
    ///
    /// `Unreachable` quando o cliente HTTP não pode ser montado.
    pub fn new(token: Option<String>) -> Result<Self, GitHubError> {
        Self::with_base(API, token)
    }

    /// Outro endereço da API — os testes apontam para um servidor local.
    ///
    /// # Errors
    ///
    /// `Unreachable` quando o cliente HTTP não pode ser montado.
    pub fn with_base(base: &str, token: Option<String>) -> Result<Self, GitHubError> {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|error| GitHubError::Unreachable(error.to_string()))?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            token: token.filter(|token| !token.is_empty()),
        })
    }

    /// O mesmo endereço com outro token (o do app vira o da instalação).
    #[must_use]
    pub fn with_token(&self, token: String) -> Self {
        Self {
            http: self.http.clone(),
            base: self.base.clone(),
            token: Some(token),
        }
    }

    /// O token em uso (o de instalação vai ao agente no `git fetch`).
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        request
    }

    async fn send(request: RequestBuilder) -> Result<Response, GitHubError> {
        let response = request
            .send()
            .await
            .map_err(|error| GitHubError::Unreachable(error.to_string()))?;
        if response.status().is_success() {
            return Ok(response);
        }
        Err(failure(response).await)
    }

    /// # Errors
    ///
    /// [`GitHubError`].
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, GitHubError> {
        Self::send(self.request(Method::GET, path))
            .await?
            .json()
            .await
            .map_err(|_| GitHubError::Unreadable)
    }

    /// Como [`Self::get`], com o 404 virando `None`.
    ///
    /// # Errors
    ///
    /// [`GitHubError`], menos `NotFound`.
    pub async fn find<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, GitHubError> {
        match self.get(path).await {
            Ok(value) => Ok(Some(value)),
            Err(GitHubError::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// O conteúdo cru de um arquivo do repositório; `None` quando não existe.
    ///
    /// # Errors
    ///
    /// [`GitHubError`], menos `NotFound`.
    pub async fn raw(&self, path: &str) -> Result<Option<String>, GitHubError> {
        let request = self
            .request(Method::GET, path)
            .header(header::ACCEPT, "application/vnd.github.raw");
        match Self::send(request).await {
            Ok(response) => response
                .text()
                .await
                .map(Some)
                .map_err(|_| GitHubError::Unreadable),
            Err(GitHubError::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// `POST`/`PUT`/`PATCH` com corpo JSON.
    ///
    /// # Errors
    ///
    /// [`GitHubError`].
    pub async fn write<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<T, GitHubError> {
        Self::send(self.request(method, path).json(body))
            .await?
            .json()
            .await
            .map_err(|_| GitHubError::Unreadable)
    }
}

/// A resposta de erro em palavras: o `message` do GitHub quando ele manda.
async fn failure(response: Response) -> GitHubError {
    let status = response.status();
    let exhausted = response
        .headers()
        .get("x-ratelimit-remaining")
        .is_some_and(|value| value == "0");
    let detail = response
        .json::<ErrorBody>()
        .await
        .ok()
        .and_then(|body| body.message)
        .unwrap_or_default();
    classify(status, exhausted, &detail)
}

fn classify(status: StatusCode, rate_limited: bool, detail: &str) -> GitHubError {
    let detail = detail.trim();
    match status.as_u16() {
        403 | 429 if rate_limited => GitHubError::Denied(
            "O limite de requisições do GitHub acabou: tente de novo em alguns minutos".into(),
        ),
        401 => GitHubError::Denied(
            "O GitHub não aceitou a credencial: o token expirou ou foi revogado".into(),
        ),
        403 => GitHubError::Denied(if detail.is_empty() {
            "O GitHub negou o acesso: falta permissão à credencial".into()
        } else {
            format!("O GitHub negou o acesso: {detail}")
        }),
        404 => GitHubError::NotFound(
            "Não encontrado no GitHub (ou a credencial não enxerga este repositório)".into(),
        ),
        code => GitHubError::Rejected(if detail.is_empty() {
            format!("O GitHub respondeu {code}")
        } else {
            format!("O GitHub recusou: {detail}")
        }),
    }
}

/// `dono/repo`, sem `.git` nem endereço.
#[must_use]
pub fn split_repository(repository: &str) -> Option<(&str, &str)> {
    let (owner, name) = repository.split_once('/')?;
    let valid = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && part != "."
            && part != ".."
    };
    (valid(owner) && valid(name)).then_some((owner, name))
}

/// Um trecho de caminho ou referência para a URL da API.
#[must_use]
pub fn encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erros_em_palavras() {
        assert!(matches!(
            classify(StatusCode::FORBIDDEN, true, ""),
            GitHubError::Denied(message) if message.contains("limite")
        ));
        assert!(matches!(
            classify(StatusCode::UNAUTHORIZED, false, "Bad credentials"),
            GitHubError::Denied(message) if message.contains("expirou")
        ));
        assert!(matches!(
            classify(StatusCode::NOT_FOUND, false, ""),
            GitHubError::NotFound(_)
        ));
        assert_eq!(
            classify(
                StatusCode::UNPROCESSABLE_ENTITY,
                false,
                "Reference already exists"
            ),
            GitHubError::Rejected("O GitHub recusou: Reference already exists".into())
        );
    }

    #[test]
    fn repositorio_valido() {
        assert_eq!(
            split_repository("maxupunk/SGCv4"),
            Some(("maxupunk", "SGCv4"))
        );
        assert_eq!(split_repository("a/b/c"), None);
        assert_eq!(split_repository("a/.."), None);
        assert_eq!(split_repository("semdono"), None);
        assert_eq!(split_repository("a b/c"), None);
    }

    #[test]
    fn caminho_na_url() {
        assert_eq!(encode("feat/x y"), "feat/x%20y");
        assert_eq!(encode("v1.0+build"), "v1.0%2Bbuild");
    }
}
