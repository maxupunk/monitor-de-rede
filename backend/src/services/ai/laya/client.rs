//! Cliente HTTP do Ollaya.
//!
//! Um `reqwest::Client` só para o processo inteiro: a decisão roda a cada
//! pergunta do chat, e o pool de conexões é o que a mantém barata. O tempo
//! limite vai por requisição, porque vem da configuração.

use std::{
    collections::BTreeMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

use reqwest::RequestBuilder;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    config::{self, AiLayaSettings},
    schema::{DecideRequest, DecideResponse, ErrorBody, Question, TagsResponse},
};
use crate::{
    dtos::ai::ModelPullProgress,
    services::{
        ai::local_models::{self, PullTarget},
        network_tools::icmp_probe::duration_to_ms,
        shared::errors::{AppError, AppResult},
    },
};

const SERVICE: &str = "Ollaya";

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Uma decisão respondida, com quem respondeu e quanto levou.
#[derive(Debug, Clone)]
pub struct Decided {
    pub response: DecideResponse,
    pub latency_ms: f64,
}

pub struct LayaClient<'a> {
    settings: &'a AiLayaSettings,
}

impl<'a> LayaClient<'a> {
    #[must_use]
    pub const fn new(settings: &'a AiLayaSettings) -> Self {
        Self { settings }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.settings.base_url.trim_end_matches('/'))
    }

    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        match config::api_key() {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(u64::from(self.settings.timeout_ms))
    }

    fn unreachable(&self, error: &reqwest::Error) -> AppError {
        let reason = if error.is_timeout() {
            format!("sem resposta em {} ms", self.settings.timeout_ms)
        } else {
            error.to_string()
        };
        AppError::service_unavailable(format!(
            "Não foi possível falar com o {SERVICE} em '{}': {reason}",
            self.settings.base_url
        ))
    }

    /// Responde todas as perguntas numa passada só.
    pub async fn decide(
        &self,
        state: &str,
        questions: &BTreeMap<String, Question>,
    ) -> AppResult<Decided> {
        let started = Instant::now();
        let request = http()
            .post(self.url("/api/decide"))
            .timeout(self.timeout())
            .json(&DecideRequest {
                model: &self.settings.model,
                state,
                questions,
            });
        let res = self
            .authorized(request)
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            let message =
                serde_json::from_str::<ErrorBody>(&body).map_or(body, |error| match error.code {
                    Some(code) => format!("{} ({code})", error.error),
                    None => error.error,
                });
            return Err(AppError::service_unavailable(format!(
                "{SERVICE} respondeu {status}: {message}"
            )));
        }

        let response = res.json::<DecideResponse>().await.map_err(|e| {
            AppError::service_unavailable(format!("Resposta inválida do {SERVICE}: {e}"))
        })?;
        Ok(Decided {
            response,
            latency_ms: duration_to_ms(started.elapsed()),
        })
    }

    /// Modelos instalados no servidor.
    pub async fn installed_models(&self) -> AppResult<Vec<String>> {
        let request = http().get(self.url("/api/tags")).timeout(self.timeout());
        let res = self
            .authorized(request)
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;
        if !res.status().is_success() {
            return Err(AppError::service_unavailable(format!(
                "{SERVICE} respondeu {} ao listar os modelos",
                res.status()
            )));
        }
        let tags = res.json::<TagsResponse>().await.map_err(|e| {
            AppError::service_unavailable(format!("Resposta inválida do {SERVICE}: {e}"))
        })?;
        Ok(tags.models.into_iter().map(|tag| tag.name).collect())
    }

    /// Baixa o modelo configurado (o roteador `laya` baixa as duas variantes).
    pub async fn pull(&self) -> AppResult<ReceiverStream<ModelPullProgress>> {
        let key = config::api_key();
        local_models::stream_pull(PullTarget {
            url: self.url("/api/pull"),
            body: serde_json::json!({ "model": self.settings.model, "stream": true }),
            bearer: key.as_deref(),
            service: SERVICE,
        })
        .await
    }
}
