//! Cliente HTTP dos provedores de decisão.
//!
//! Um `reqwest::Client` só para o processo inteiro: a decisão roda a cada
//! pergunta do chat, e o pool de conexões é o que a mantém barata. O tempo
//! limite vai por requisição, porque vem da configuração.
//!
//! A decisão vale para todo provedor (o formato vem de `laya::protocols`);
//! listar, baixar e carregar modelos é só do Ollaya.

use std::{
    collections::BTreeMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

use reqwest::RequestBuilder;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    catalog,
    config::AiLayaSettings,
    protocols::{self, wire_for},
    schema::{DecideResponse, Question, TagsResponse},
};
use crate::{
    dtos::ai::ModelPullProgress,
    services::{
        ai::local_models::{self, PullTarget},
        network_tools::icmp_probe::duration_to_ms,
        shared::errors::{AppError, AppResult},
    },
};

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

    /// Quem responde, para as mensagens ("TypeSafe (Jev)", "Ollaya"…).
    fn service(&self) -> &'static str {
        self.settings.preset().label
    }

    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        match self.settings.effective_api_key() {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }

    /// Listar, baixar e carregar modelos só existe no Ollaya.
    fn ensure_manages_models(&self) -> AppResult<()> {
        if self.settings.manages_models() {
            return Ok(());
        }
        Err(AppError::validation(format!(
            "{} não baixa nem carrega modelos: eles já estão prontos na nuvem.",
            self.service()
        )))
    }

    /// Resposta de erro, com o que fazer quando a chave foi recusada.
    async fn failure(&self, res: reqwest::Response, action: &str) -> AppError {
        let status = res.status();
        let detail = protocols::error_message(&res.text().await.unwrap_or_default());
        let hint = match status.as_u16() {
            401 | 403 => " — confira a chave de acesso (e, na Cloudflare, o ID da conta).",
            402 => " — a conta está sem crédito.",
            404 => " — confira a URL e o nome do modelo.",
            429 => " — limite de uso atingido; tente de novo em instantes.",
            _ => "",
        };
        AppError::service_unavailable(format!(
            "{} respondeu {status} {action}: {detail}{hint}",
            self.service()
        ))
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(u64::from(self.settings.timeout_ms))
    }

    fn unreachable(&self, error: &reqwest::Error) -> AppError {
        self.unreachable_after(error, self.timeout())
    }

    fn unreachable_after(&self, error: &reqwest::Error, waited: Duration) -> AppError {
        let reason = if error.is_timeout() {
            format!("sem resposta em {} ms", waited.as_millis())
        } else {
            error.to_string()
        };
        AppError::service_unavailable(format!(
            "Não foi possível falar com {} em '{}': {reason}",
            self.service(),
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
        let wire = wire_for(self.settings.protocol());
        let call = wire.request(self.settings, state, questions)?;
        let request = http()
            .post(call.url)
            .timeout(self.timeout())
            .json(&call.body);
        let res = self
            .authorized(request)
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;
        if !res.status().is_success() {
            return Err(self.failure(res, "à decisão").await);
        }
        let body = res.json::<serde_json::Value>().await.map_err(|e| {
            AppError::service_unavailable(format!("Resposta inválida de {}: {e}", self.service()))
        })?;
        let response: DecideResponse = wire.parse(self.settings, body)?;
        Ok(Decided {
            response,
            latency_ms: duration_to_ms(started.elapsed()),
        })
    }

    /// Modelos carregados na memória agora (`GET /api/ps`).
    pub async fn loaded_models(&self) -> AppResult<Vec<String>> {
        self.ensure_manages_models()?;
        let request = http().get(self.url("/api/ps")).timeout(self.timeout());
        let res = self
            .authorized(request)
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;
        if !res.status().is_success() {
            return Err(self.failure(res, "ao listar os modelos carregados").await);
        }
        let loaded = res.json::<TagsResponse>().await.map_err(|e| {
            AppError::service_unavailable(format!("Resposta inválida de {}: {e}", self.service()))
        })?;
        Ok(loaded.models.into_iter().map(|tag| tag.name).collect())
    }

    /// Carrega o modelo na memória sem responder nada: `/api/decide` sem
    /// `state` nem perguntas (contrato do Ollaya). Devolve quanto levou.
    ///
    /// O prazo é o do modelo ([`catalog::load_timeout`]), não o "tempo máximo
    /// por pergunta": um Laya sai do disco em 5 a 20 s em CPU, um Kev 9B
    /// tem quase 20 GB.
    pub async fn load(&self) -> AppResult<f64> {
        self.ensure_manages_models()?;
        let started = Instant::now();
        let load_timeout = catalog::load_timeout(&self.settings.model);
        let request = http()
            .post(self.url("/api/decide"))
            .timeout(load_timeout)
            .json(&serde_json::json!({ "model": self.settings.model }));
        let res = self
            .authorized(request)
            .send()
            .await
            .map_err(|e| self.unreachable_after(&e, load_timeout))?;
        if !res.status().is_success() {
            return Err(self.failure(res, "ao carregar o modelo").await);
        }
        Ok(duration_to_ms(started.elapsed()))
    }

    /// Modelos instalados no servidor.
    pub async fn installed_models(&self) -> AppResult<Vec<String>> {
        self.ensure_manages_models()?;
        let request = http().get(self.url("/api/tags")).timeout(self.timeout());
        let res = self
            .authorized(request)
            .send()
            .await
            .map_err(|e| self.unreachable(&e))?;
        if !res.status().is_success() {
            return Err(self.failure(res, "ao listar os modelos").await);
        }
        let tags = res.json::<TagsResponse>().await.map_err(|e| {
            AppError::service_unavailable(format!("Resposta inválida de {}: {e}", self.service()))
        })?;
        Ok(tags.models.into_iter().map(|tag| tag.name).collect())
    }

    /// Baixa o modelo configurado (o roteador `laya` baixa as duas variantes).
    pub async fn pull(&self) -> AppResult<ReceiverStream<ModelPullProgress>> {
        self.ensure_manages_models()?;
        let key = self.settings.effective_api_key();
        local_models::stream_pull(PullTarget {
            url: self.url("/api/pull"),
            body: serde_json::json!({ "model": self.settings.model, "stream": true }),
            bearer: key.as_deref(),
            service: self.service(),
        })
        .await
    }
}
