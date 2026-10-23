//! Cliente HTTP dos provedores de decisão.
//!
//! Um `reqwest::Client` só para o processo inteiro: a decisão roda a cada
//! pergunta do chat, e o pool de conexões é o que a mantém barata. O tempo
//! limite vai por requisição, porque vem da configuração.
//!
//! A decisão vale para todo provedor (o formato vem de `decider::protocols`);
//! listar, baixar e carregar modelos é só do Ollaya.

use std::{
    borrow::Cow,
    collections::BTreeMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

use reqwest::{RequestBuilder, StatusCode};
use tokio_stream::wrappers::ReceiverStream;

use super::{
    catalog,
    config::AiDeciderSettings,
    locate::{self, Located},
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

/// O motivo de fato: o `reqwest` diz só "error sending request", e o que
/// separa "o nome não resolve" de "conexão recusada" está na cadeia de causas.
fn root_cause(error: &reqwest::Error) -> String {
    let mut reason = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        reason = cause.to_string();
        source = cause.source();
    }
    reason
}

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

pub struct DeciderClient<'a> {
    settings: Cow<'a, AiDeciderSettings>,
    /// Onde o Ollaya foi procurado sem responder — vai na mensagem de erro.
    tried: Vec<String>,
}

impl<'a> DeciderClient<'a> {
    /// Fala com a URL configurada, exatamente como está.
    #[must_use]
    pub const fn new(settings: &'a AiDeciderSettings) -> Self {
        Self {
            settings: Cow::Borrowed(settings),
            tried: Vec::new(),
        }
    }

    /// Fala com o Ollaya onde ele de fato está, quando a URL é a padrão do
    /// compose — ver [`locate`].
    pub async fn connect(settings: &'a AiDeciderSettings) -> Self {
        match locate::resolve(settings).await {
            Located::Ready(settings) => Self {
                settings,
                tried: Vec::new(),
            },
            Located::NotFound(settings, tried) => Self {
                settings: Cow::Borrowed(settings),
                tried,
            },
        }
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
        AppError::service_unavailable(failure_message(&self.settings, status, action, &detail))
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
            root_cause(error)
        };
        let tried = if self.tried.len() > 1 {
            format!(" Também procurado em: {}.", self.tried[1..].join(", "))
        } else {
            String::new()
        };
        AppError::service_unavailable(format!(
            "Não foi possível falar com {} em '{}': {reason}.{tried}",
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
        let call = wire.request(&self.settings, state, questions)?;
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
        let response: DecideResponse = wire.parse(&self.settings, body)?;
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
    /// por pergunta": um Decisor sai do disco em 5 a 20 s em CPU, um Kev 9B
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
        if catalog::is_bundled(&self.settings.model) {
            return Err(AppError::validation(format!(
                "{} não está no registro do Ollaya. {}",
                self.settings.model,
                catalog::BUNDLED_HINT
            )));
        }
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

/// A mensagem de uma resposta de erro. Num provedor de nuvem com a URL fora
/// do padrão, quem respondeu foi esse endereço, não o provedor: a mensagem diz
/// qual e aponta o padrão — o caso clássico é a URL do Ollaya esquecida no
/// OpenRouter.
fn failure_message(
    settings: &AiDeciderSettings,
    status: StatusCode,
    action: &str,
    detail: &str,
) -> String {
    let preset = settings.preset();
    let standard = preset.default_base_url;
    let elsewhere = preset.cloud && !standard.is_empty() && settings.base_url != standard;
    let who = if elsewhere {
        format!("O endereço '{}'", settings.base_url)
    } else {
        preset.label.to_string()
    };
    let hint = match status.as_u16() {
        401 | 403 => " — confira a chave de acesso (e, na Cloudflare, o ID da conta).".to_string(),
        402 => " — a conta está sem crédito.".to_string(),
        404 if elsewhere => format!(
            " — esse não é o endereço de {} ({standard}). Use o padrão, a não ser que seja um proxy ou gateway.",
            preset.label
        ),
        404 if preset.cloud => " — confira o nome do modelo.".to_string(),
        404 => " — confira a URL e o nome do modelo.".to_string(),
        429 => " — limite de uso atingido; tente de novo em instantes.".to_string(),
        _ => String::new(),
    };
    format!("{who} respondeu {status} {action}: {detail}{hint}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::decider::providers::DeciderProvider;

    #[tokio::test]
    async fn o_erro_diz_a_causa_e_nao_so_error_sending_request() {
        let settings = AiDeciderSettings {
            base_url: "http://127.0.0.1:9".into(),
            timeout_ms: 2_000,
            ..AiDeciderSettings::default()
        };
        let message = DeciderClient::connect(&settings)
            .await
            .installed_models()
            .await
            .unwrap_err()
            .to_string();
        assert!(message.contains("127.0.0.1:9"), "{message}");
        assert!(!message.contains("error sending request"), "{message}");
    }

    fn openrouter_at(base_url: &str) -> AiDeciderSettings {
        AiDeciderSettings {
            provider: DeciderProvider::Openrouter,
            base_url: base_url.into(),
            ..AiDeciderSettings::default()
        }
    }

    #[test]
    fn url_esquecida_de_outro_provedor_e_apontada_no_404() {
        let settings = openrouter_at("http://127.0.0.1:11435");
        let message = failure_message(&settings, StatusCode::NOT_FOUND, "à decisão", "not found");
        assert!(
            message.starts_with("O endereço 'http://127.0.0.1:11435' respondeu 404 Not Found"),
            "{message}"
        );
        assert!(message.contains("https://openrouter.ai/api"), "{message}");
    }

    #[test]
    fn no_endereco_padrao_quem_responde_e_o_provedor() {
        let settings = openrouter_at("https://openrouter.ai/api");
        let message = failure_message(&settings, StatusCode::NOT_FOUND, "à decisão", "not found");
        assert!(
            message.starts_with("OpenRouter respondeu 404 Not Found"),
            "{message}"
        );
        assert!(message.ends_with("confira o nome do modelo."), "{message}");
    }

    #[test]
    fn ollaya_achado_fora_da_url_padrao_continua_sendo_o_ollaya() {
        let settings = AiDeciderSettings {
            base_url: "http://127.0.0.1:11435".into(),
            ..AiDeciderSettings::default()
        };
        let message = failure_message(&settings, StatusCode::NOT_FOUND, "à decisão", "x");
        assert!(message.starts_with("Ollaya respondeu"), "{message}");
    }
}
