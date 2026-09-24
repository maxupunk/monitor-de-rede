//! Onde cada provedor informa a janela de contexto dos modelos.
//!
//! | provedor | fonte | campo |
//! |---|---|---|
//! | OpenRouter | `GET {base}/models` | `context_length` (e `top_provider.context_length`) |
//! | Ollama | `GET /api/ps`, `POST /api/show` | `context_length` do modelo carregado; `num_ctx`; `*.context_length` |
//! | OpenCode (e outros) | `GET https://models.dev/api.json` | `{provedor}.models.{id}.limit.context` |
//!
//! O `/models` do OpenCode Zen só lista ids, sem janela — por isso o catálogo
//! público do `models.dev`, que é mantido pelo próprio projeto OpenCode.
//!
//! Os parsers são funções puras sobre o JSON: a parte HTTP é fina e os
//! formatos ficam cobertos por teste sem rede.

use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use serde_json::{json, Value};

/// Janela padrão do Ollama quando nem o modelo carregado nem o `num_ctx`
/// dizem outra coisa (`OLLAMA_CONTEXT_LENGTH` não definido no servidor).
pub const OLLAMA_DEFAULT_NUM_CTX: u64 = 4_096;

/// Catálogo público de modelos por provedor.
pub const MODELS_DEV_URL: &str = "https://models.dev/api.json";

const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// Uma janela aprendida e o quanto ela é certa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearnedWindow {
    pub tokens: u64,
    /// `false` quando é um padrão presumido (o Ollama sem modelo carregado):
    /// vale para esta resposta, mas não merece ficar em cache.
    pub confident: bool,
}

impl LearnedWindow {
    #[must_use]
    pub const fn confident(tokens: u64) -> Self {
        Self {
            tokens,
            confident: true,
        }
    }
}

/// Uma fonte de janelas. Uma consulta pode ensinar vários modelos de uma vez
/// (o catálogo inteiro do provedor): tudo vai para o cache.
#[async_trait]
pub trait WindowLookup: Send + Sync {
    /// Identifica a fonte no cache (provedor + endereço).
    fn cache_key(&self) -> String;

    /// Por quanto tempo o que foi aprendido vale.
    fn ttl(&self) -> Duration {
        Duration::from_secs(6 * 3600)
    }

    /// Janelas aprendidas, por id de modelo. Falha de rede é mapa vazio.
    async fn fetch(&self, model: &str) -> HashMap<String, LearnedWindow>;
}

fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .ok()
}

async fn get_json(url: &str, bearer: Option<&str>) -> Option<Value> {
    let mut request = client()?.get(url);
    if let Some(token) = bearer.map(str::trim).filter(|token| !token.is_empty()) {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json().await.ok()
}

fn positive(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64).filter(|tokens| *tokens > 0)
}

// --- OpenRouter ---------------------------------------------------------------

/// Todas as janelas do catálogo do OpenRouter. Quando o provedor principal
/// do modelo informa uma janela menor (`top_provider`), vale a menor: é por
/// ele que a requisição passa.
#[must_use]
pub fn parse_openrouter(catalog: &Value) -> HashMap<String, LearnedWindow> {
    catalog["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry["id"].as_str()?;
            let listed = positive(entry.get("context_length"));
            let provider = positive(entry.pointer("/top_provider/context_length"));
            let tokens = match (listed, provider) {
                (Some(a), Some(b)) => a.min(b),
                (a, b) => a.or(b)?,
            };
            Some((id.to_string(), LearnedWindow::confident(tokens)))
        })
        .collect()
}

pub struct OpenRouterLookup {
    pub base_url: String,
    pub api_key: Option<String>,
}

#[async_trait]
impl WindowLookup for OpenRouterLookup {
    fn cache_key(&self) -> String {
        format!("openrouter:{}", self.base_url)
    }

    async fn fetch(&self, _model: &str) -> HashMap<String, LearnedWindow> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        get_json(&url, self.api_key.as_deref())
            .await
            .map(|catalog| parse_openrouter(&catalog))
            .unwrap_or_default()
    }
}

// --- models.dev (OpenCode e afins) -------------------------------------------

/// Janelas de um provedor no catálogo do `models.dev`.
#[must_use]
pub fn parse_models_dev(catalog: &Value, provider: &str) -> HashMap<String, LearnedWindow> {
    catalog
        .pointer(&format!("/{provider}/models"))
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(id, model)| {
            let tokens = positive(model.pointer("/limit/context"))?;
            Some((id.clone(), LearnedWindow::confident(tokens)))
        })
        .collect()
}

pub struct ModelsDevLookup {
    pub url: String,
    /// Id do provedor no catálogo (`opencode`).
    pub provider: String,
}

#[async_trait]
impl WindowLookup for ModelsDevLookup {
    fn cache_key(&self) -> String {
        format!("models.dev:{}:{}", self.url, self.provider)
    }

    async fn fetch(&self, _model: &str) -> HashMap<String, LearnedWindow> {
        get_json(&self.url, None)
            .await
            .map(|catalog| parse_models_dev(&catalog, &self.provider))
            .unwrap_or_default()
    }
}

// --- Ollama ------------------------------------------------------------------

/// `llama3.2` e `llama3.2:latest` são o mesmo modelo para o Ollama.
fn same_ollama_model(a: &str, b: &str) -> bool {
    let normalize = |name: &str| {
        let name = name.trim().to_ascii_lowercase();
        if name.contains(':') {
            name
        } else {
            format!("{name}:latest")
        }
    };
    normalize(a) == normalize(b)
}

/// Janela do modelo **carregado agora** (`/api/ps`): é a que o servidor usa
/// de fato, já com `num_ctx`, `OLLAMA_CONTEXT_LENGTH` e o ajuste por VRAM.
#[must_use]
pub fn parse_ollama_ps(running: &Value, model: &str) -> Option<u64> {
    running["models"]
        .as_array()?
        .iter()
        .find(|entry| {
            ["name", "model"].iter().any(|key| {
                entry[*key]
                    .as_str()
                    .is_some_and(|name| same_ollama_model(name, model))
            })
        })
        .and_then(|entry| positive(entry.get("context_length")))
}

/// `(janela máxima do modelo, num_ctx configurado)` do `/api/show`.
#[must_use]
pub fn parse_ollama_show(show: &Value) -> (Option<u64>, Option<u64>) {
    let max = show["model_info"].as_object().and_then(|info| {
        info.iter()
            .find(|(key, _)| key.ends_with(".context_length"))
            .and_then(|(_, value)| positive(Some(value)))
    });
    let num_ctx = show["parameters"].as_str().and_then(|parameters| {
        parameters.lines().find_map(|line| {
            let mut parts = line.split_whitespace();
            (parts.next() == Some("num_ctx"))
                .then(|| parts.next()?.parse::<u64>().ok())
                .flatten()
                .filter(|tokens| *tokens > 0)
        })
    });
    (max, num_ctx)
}

/// A janela que o Ollama vai usar: a do modelo carregado quando há; senão o
/// `num_ctx` do modelo, senão o padrão do servidor — nunca acima do máximo
/// do modelo. Só a do modelo carregado é certa.
#[must_use]
pub fn ollama_effective_window(
    running: Option<u64>,
    max: Option<u64>,
    num_ctx: Option<u64>,
) -> Option<LearnedWindow> {
    if let Some(tokens) = running {
        return Some(LearnedWindow::confident(tokens));
    }
    if max.is_none() && num_ctx.is_none() {
        return None;
    }
    let configured = num_ctx.unwrap_or(OLLAMA_DEFAULT_NUM_CTX);
    Some(LearnedWindow {
        tokens: max.map_or(configured, |max| configured.min(max)),
        confident: false,
    })
}

pub struct OllamaLookup {
    /// Endereço da API nativa (`http://host:11434`), sem o `/v1`.
    pub base_url: String,
}

impl OllamaLookup {
    /// Aceita a URL do modo compatível com OpenAI (`…/v1`) que a tela guarda.
    #[must_use]
    pub fn from_openai_base(base_url: &str) -> Self {
        let trimmed = base_url.trim().trim_end_matches('/');
        let native = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
        Self {
            base_url: native.to_string(),
        }
    }
}

#[async_trait]
impl WindowLookup for OllamaLookup {
    fn cache_key(&self) -> String {
        format!("ollama:{}", self.base_url)
    }

    /// Curto: carregar outro modelo ou mudar `num_ctx` muda a janela.
    fn ttl(&self) -> Duration {
        Duration::from_secs(60)
    }

    async fn fetch(&self, model: &str) -> HashMap<String, LearnedWindow> {
        let running = get_json(&format!("{}/api/ps", self.base_url), None)
            .await
            .and_then(|ps| parse_ollama_ps(&ps, model));
        let (max, num_ctx) = match client() {
            Some(client) => match client
                .post(format!("{}/api/show", self.base_url))
                .json(&json!({ "model": model }))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => response
                    .json::<Value>()
                    .await
                    .map_or((None, None), |show| parse_ollama_show(&show)),
                _ => (None, None),
            },
            None => (None, None),
        };
        ollama_effective_window(running, max, num_ctx)
            .map(|window| HashMap::from([(model.to_string(), window)]))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openrouter_usa_a_menor_entre_o_catalogo_e_o_provedor() {
        let catalogo = json!({ "data": [
            { "id": "z-ai/glm-5", "context_length": 1_000_000,
              "top_provider": { "context_length": 1_000_000 } },
            { "id": "meta/llama", "context_length": 131_072,
              "top_provider": { "context_length": 32_768 } },
            { "id": "openrouter/free", "context_length": 200_000, "top_provider": {} },
            { "id": "sem-janela" }
        ]});
        let janelas = parse_openrouter(&catalogo);
        assert_eq!(janelas["z-ai/glm-5"].tokens, 1_000_000);
        assert_eq!(janelas["meta/llama"].tokens, 32_768);
        assert_eq!(janelas["openrouter/free"].tokens, 200_000);
        assert!(!janelas.contains_key("sem-janela"));
        assert!(janelas.values().all(|janela| janela.confident));
    }

    #[test]
    fn models_dev_le_so_o_provedor_pedido() {
        let catalogo = json!({
            "opencode": { "models": {
                "muse-spark-1.3-contributor-free": { "limit": { "context": 1_048_576, "output": 131_072 } },
                "sem-limite": {}
            }},
            "outro": { "models": { "x": { "limit": { "context": 8 } } } }
        });
        let janelas = parse_models_dev(&catalogo, "opencode");
        assert_eq!(janelas.len(), 1);
        assert_eq!(janelas["muse-spark-1.3-contributor-free"].tokens, 1_048_576);
        assert!(parse_models_dev(&catalogo, "inexistente").is_empty());
    }

    #[test]
    fn ollama_prefere_o_modelo_carregado() {
        let ps = json!({ "models": [
            { "name": "llama3.2:latest", "model": "llama3.2:latest", "context_length": 8192 }
        ]});
        assert_eq!(parse_ollama_ps(&ps, "llama3.2"), Some(8192));
        assert_eq!(parse_ollama_ps(&ps, "qwen3:8b"), None);
        assert_eq!(parse_ollama_ps(&json!({ "models": [] }), "llama3.2"), None);

        let show = json!({
            "parameters": "stop \"<|eot_id|>\"\nnum_ctx 16384",
            "model_info": { "general.architecture": "llama", "llama.context_length": 131_072 }
        });
        assert_eq!(parse_ollama_show(&show), (Some(131_072), Some(16_384)));
        assert_eq!(
            parse_ollama_show(&json!({ "model_info": { "qwen3.context_length": 40_960 } })),
            (Some(40_960), None)
        );
    }

    #[test]
    fn janela_efetiva_do_ollama_e_certa_so_com_o_modelo_carregado() {
        assert_eq!(
            ollama_effective_window(Some(8192), Some(131_072), None),
            Some(LearnedWindow::confident(8192))
        );
        assert_eq!(
            ollama_effective_window(None, Some(131_072), Some(16_384)),
            Some(LearnedWindow {
                tokens: 16_384,
                confident: false
            })
        );
        assert_eq!(
            ollama_effective_window(None, Some(131_072), None),
            Some(LearnedWindow {
                tokens: OLLAMA_DEFAULT_NUM_CTX,
                confident: false
            }),
            "sem num_ctx vale o padrão do servidor"
        );
        assert_eq!(
            ollama_effective_window(None, Some(2048), Some(8192)).map(|janela| janela.tokens),
            Some(2048),
            "nunca acima do máximo do modelo"
        );
        assert_eq!(ollama_effective_window(None, None, None), None);
    }

    #[test]
    fn ollama_aceita_a_url_do_modo_openai() {
        assert_eq!(
            OllamaLookup::from_openai_base("http://localhost:11434/v1/").base_url,
            "http://localhost:11434"
        );
        assert_eq!(
            OllamaLookup::from_openai_base("http://gpu:11434").base_url,
            "http://gpu:11434"
        );
    }
}
