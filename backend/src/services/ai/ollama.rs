//! Integração com a API nativa do Ollama para gerenciamento e instalação de modelos.
//!
//! Fornece consulta de modelos instalados (`/api/tags`), catálogo de modelos
//! recomendados para o NetMonitor e streaming em tempo real do progresso de
//! download (`/api/pull`). Enquanto a URL da tela apontar para esta máquina,
//! o Ollama é procurado onde o compose o deixa ([`OLLAMA`], ver
//! [`super::local_server`]).

use std::time::Duration;

use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    local_models::{self, BundledOllamaModel, PullTarget},
    local_server::LocalServer,
    settings::AiSettings,
};
use crate::{
    dtos::ai::{ModelPullProgress, OllamaModelItem, OllamaModelsResponse, OllamaRecommendedModel},
    services::shared::errors::{AppError, AppResult},
};

/// O Ollama do compose: o serviço `ollama`, e `OLLAMA_URL` definida pela
/// instalação (o override de host a define).
pub static OLLAMA: LocalServer = LocalServer::new("Ollama", "ollama", 11434, "OLLAMA_URL");

/// Modelos de chat empacotados pelo NetMonitor: quando o usuário pede o pull
/// pela tela (ou em um Ollama sem o volume pré-populado), os pesos GGUF vêm
/// direto da release do GitHub do NetMonitor (`CHAT_MODELS_URL`).
pub static BUNDLED_MODELS: &[BundledOllamaModel] = &[BundledOllamaModel {
    canonical_name: "lfm3-netmonitor:latest",
    aliases: &[
        "lfm3-netmonitor",
        "netmonitor/lfm3-netmonitor",
        "netmonitor/lfm3-netmonitor:latest",
    ],
    release_url_env: "CHAT_MODELS_URL",
    default_release_url:
        "https://github.com/maxupunk/monitor-de-rede/releases/download/chat-models",
    gguf_filename: "lfm3-netmonitor-q8_0.gguf",
    gguf_digest: "sha256:a1ff35a978ca9f9dc09d1ecf062a66596c1dd1c4c835c4b127778fe5581cd00d",
    gguf_size: 1_846_237_728,
    template: "{{ .Prompt }}",
    renderer: "lfm3",
    parser: "lfm3",
    create_parameters: || {
        serde_json::json!({
            "temperature": 0.1,
            "top_p": 0.1,
            "top_k": 50,
            "repeat_penalty": 1.05,
        })
    },
}];

/// Localiza a receita empacotada para um modelo do NetMonitor, se houver.
#[must_use]
pub fn bundled_model(model: &str) -> Option<&'static BundledOllamaModel> {
    BUNDLED_MODELS.iter().find(|spec| spec.matches(model))
}

/// Modelos recomendados para execução no Ollama com o NetMonitor.
pub static RECOMMENDED_MODELS: &[(&str, &str, &str, &str, bool)] = &[
    (
        "lfm3-netmonitor:latest",
        "LFM3 NetMonitor 1.7B — Agente especialista ajustado para redes (ICMP/TCP/UDP, Wi-Fi IEEE 802.11, fibra óptica GPON/SFP+), roteadores, Docker, sistema de backup, dashboard e as 61 ferramentas do NetMonitor (incluindo web_search via frontend): 1,8 GB (Q8_0), resposta em ~2s sem internet",
        "1.7B",
        "256k",
        true,
    ),
    (
        "ornith-1.5:9b",
        "Ornith 1.5 9B — Modelo recomendado de grande porte: autoaperfeiçoamento, raciocínio avançado, ferramentas de rede e janela nativa de 256k",
        "9B",
        "256k",
        true,
    ),
    (
        "lfm3:latest",
        "LFM3 1.7B — Modelo base tri-híbrido para agentes locais: 1,1 GB, roda em CPU e chama ferramentas (76,8% no benchmark de chat antes do ajuste)",
        "1.7B",
        "256k",
        false,
    ),
    (
        "llama3-groq-tool-use:8b",
        "Llama 3 Groq Tool Use 8B — Especialista em execução de ferramentas de rede e diagnósticos (Function Calling)",
        "8B",
        "128k",
        true,
    ),
    (
        "llama3.2:3b",
        "Llama 3.2 3B — Modelo ultraleve e veloz da Meta, excelente relação entre consumo e acurácia",
        "3B",
        "128k",
        false,
    ),
    (
        "llama3.2:1b",
        "Llama 3.2 1B — Modelo mais compacto da Meta, consome menos de 2GB de RAM",
        "1B",
        "128k",
        false,
    ),
    (
        "llama3.3:70b",
        "Llama 3.3 70B — Estado da arte da Meta em raciocínio, diagnóstico e código",
        "70B",
        "128k",
        true,
    ),
    (
        "deepseek-r1:8b",
        "DeepSeek R1 8B — Especialista em raciocínio lógico avançado e arquitetura de redes",
        "8B",
        "128k",
        false,
    ),
    (
        "deepseek-r1:14b",
        "DeepSeek R1 14B — Raciocínio profundo com alta capacidade analítica para sistemas",
        "14B",
        "128k",
        false,
    ),
    (
        "qwen2.5-coder:7b",
        "Qwen 2.5 Coder 7B — Especialista em automação técnica, scripts e infraestrutura",
        "7B",
        "32k",
        true,
    ),
    (
        "gemma4:e2b",
        "Gemma 4 e2b — Modelo ultraleve do Google, ideal para respostas instantâneas em servidores com recursos modestos",
        "2B",
        "128k",
        false,
    ),
    (
        "gemma4:e4b",
        "Gemma 4 e4b — Equilíbrio ideal entre consumo de memória e capacidade de resolução de problemas",
        "4B",
        "128k",
        false,
    ),
    (
        "qwen3:8b",
        "Qwen 3 8B — Excelente raciocínio lógico, análise estruturada e alta precisão em língua portuguesa",
        "8B",
        "128k",
        false,
    ),
    (
        "qwen3.8:27b",
        "Qwen 3.8 27B — Alta precisão, raciocínio técnico avançado, suporte a ferramentas e janela de 262k",
        "27B",
        "262k",
        true,
    ),
    (
        "granite4.2:latest",
        "Granite 4.2 — Modelo corporativo da IBM otimizado para operações de TI, automação e infraestrutura",
        "8B",
        "128k",
        false,
    ),
    (
        "mixtral:8x7b",
        "Mixtral 8x7B — Arquitetura Mixture-of-Experts para análises complexas e correlações avançadas de rede",
        "46.7B",
        "32k",
        false,
    ),
    (
        "phi4:14b",
        "Phi-4 14B — Modelo da Microsoft com excepcional raciocínio sintético e síntese técnica",
        "14B",
        "16k",
        false,
    ),
];

/// Normaliza a URL base do Ollama, removendo sufixos `/v1` e barras finais.
pub fn normalize_ollama_base_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "http://localhost:11434".to_string();
    }
    let mut url = trimmed.trim_end_matches('/').to_string();
    if url.ends_with("/v1") {
        url = url[..url.len() - 3].trim_end_matches('/').to_string();
    }
    if url.is_empty() {
        "http://localhost:11434".to_string()
    } else {
        url
    }
}

/// Teto mínimo de `max_tokens` com o raciocínio ligado: o que o modelo pensa
/// conta no teto, e os 1.024 do estilo Direto acabavam antes da resposta
/// (benchmark de chat do Laya-tuning, `lfm3`: com 4.096, nenhuma resposta
/// cortada e a nota de 75 para 83).
pub const REASONING_MIN_OUTPUT_TOKENS: u32 = 4096;

/// O piso de `max_tokens` do chat no Ollama: só com o raciocínio ligado.
#[must_use]
pub const fn min_output_tokens(settings: &AiSettings) -> Option<u32> {
    if settings.ollama_reasoning {
        Some(REASONING_MIN_OUTPUT_TOKENS)
    } else {
        None
    }
}

/// O `reasoning_effort` que o chat envia ao Ollama: `none` com o raciocínio
/// desligado (o padrão), nada com ele ligado — aí vale o do modelo.
#[must_use]
pub const fn reasoning_effort(settings: &AiSettings) -> Option<&'static str> {
    if settings.ollama_reasoning {
        None
    } else {
        Some("none")
    }
}

/// A raiz (sem `/v1`) onde o Ollama de fato está. A URL de outra máquina é
/// soberana; a desta máquina, quando ninguém responde nela, é trocada pelo
/// primeiro endereço do compose que responder. Ninguém respondendo, fica a
/// da tela — e o erro da chamada diz onde se tentou.
pub async fn locate_root(configured: &str) -> String {
    let root = normalize_ollama_base_url(configured);
    if !OLLAMA.means_this_machine(&root) {
        return root;
    }
    OLLAMA.locate(&root).await.unwrap_or(root)
}

/// A URL do protocolo OpenAI (`.../v1`) para o chat: a da tela, ou a do
/// endereço onde o Ollama foi achado ([`locate_root`]).
pub async fn locate_openai_base(configured: &str) -> String {
    let root = locate_root(configured).await;
    if root == normalize_ollama_base_url(configured) {
        configured.to_owned()
    } else {
        format!("{root}/v1")
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTagDetails {
    parameter_size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagItem {
    name: String,
    size: Option<u64>,
    modified_at: Option<String>,
    details: Option<OllamaTagDetails>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaTagItem>>,
}

/// Consulta os modelos instalados no Ollama e retorna cruzados com os recomendados.
pub async fn list_models(base_url: &str) -> AppResult<OllamaModelsResponse> {
    let root_url = locate_root(base_url).await;
    let tags_url = format!("{root_url}/api/tags");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::service_unavailable(format!("Erro no cliente HTTP: {e}")))?;

    let response = client.get(&tags_url).send().await;

    let (installed_items, is_online, error_message) = match response {
        Ok(res) if res.status().is_success() => {
            if let Ok(tags) = res.json::<OllamaTagsResponse>().await {
                let items = tags
                    .models
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| OllamaModelItem {
                        name: m.name,
                        size: m.size,
                        parameter_size: m.details.and_then(|d| d.parameter_size),
                        modified_at: m.modified_at,
                    })
                    .collect();
                (items, true, None)
            } else {
                (Vec::new(), true, None)
            }
        }
        Ok(res) => {
            let status = res.status();
            (
                Vec::new(),
                false,
                Some(format!("Ollama retornou status HTTP {status}")),
            )
        }
        Err(e) => (
            Vec::new(),
            false,
            Some(format!(
                "Não foi possível conectar ao Ollama em '{root_url}': {e}"
            )),
        ),
    };

    let installed_names: Vec<&str> = installed_items.iter().map(|m| m.name.as_str()).collect();

    let recommended: Vec<OllamaRecommendedModel> = RECOMMENDED_MODELS
        .iter()
        .map(
            |&(name, desc, param_size, context_window, tool_optimized)| {
                let is_installed = local_models::is_installed(&installed_names, name)
                    || bundled_model(name).is_some_and(|spec| {
                        spec.aliases
                            .iter()
                            .any(|alias| local_models::is_installed(&installed_names, alias))
                    });

                OllamaRecommendedModel {
                    name: name.to_string(),
                    description: desc.to_string(),
                    parameter_size: param_size.to_string(),
                    context_window: context_window.to_string(),
                    is_installed,
                    tool_calling_optimized: tool_optimized,
                }
            },
        )
        .collect();

    Ok(OllamaModelsResponse {
        installed: installed_items,
        recommended,
        is_online,
        error_message,
    })
}

/// Inicia o download (pull) de um modelo no Ollama e transmite o progresso via stream assíncrono.
/// Para modelos ajustados do NetMonitor (`BUNDLED_MODELS`), baixa os pesos da release do
/// repositório do NetMonitor (`CHAT_MODELS_URL`) e registra no Ollama via `/api/blobs` + `/api/create`.
pub async fn pull_model_stream(
    base_url: &str,
    model: &str,
) -> AppResult<ReceiverStream<ModelPullProgress>> {
    let model_name = model.trim();
    if model_name.is_empty() {
        return Err(AppError::validation("Nome do modelo não pode ser vazio"));
    }
    let root = locate_root(base_url).await;
    if let Some(spec) = bundled_model(model_name) {
        return local_models::stream_bundled_ollama_install(root, model_name.to_string(), spec)
            .await;
    }
    local_models::stream_pull(PullTarget {
        url: format!("{root}/api/pull"),
        body: serde_json::json!({ "name": model_name, "stream": true }),
        bearer: None,
        service: "Ollama",
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testa_normalizacao_de_url_do_ollama() {
        assert_eq!(normalize_ollama_base_url(""), "http://localhost:11434");
        assert_eq!(
            normalize_ollama_base_url("http://localhost:11434/v1"),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_ollama_base_url("http://localhost:11434/v1/"),
            "http://localhost:11434"
        );
        assert_eq!(
            normalize_ollama_base_url("http://192.168.1.50:11434/"),
            "http://192.168.1.50:11434"
        );
        assert_eq!(
            normalize_ollama_base_url("https://ollama.internal.net"),
            "https://ollama.internal.net"
        );
    }

    #[test]
    fn a_url_padrao_e_a_do_servico_do_compose() {
        assert_eq!(
            normalize_ollama_base_url(crate::services::ai::settings::DEFAULT_OLLAMA_BASE_URL),
            OLLAMA.default_url()
        );
    }

    #[test]
    fn raciocinio_desligado_por_padrao_vai_como_none() {
        let mut settings = AiSettings::default();
        assert_eq!(reasoning_effort(&settings), Some("none"));
        assert_eq!(min_output_tokens(&settings), None);
        settings.ollama_reasoning = true;
        assert_eq!(reasoning_effort(&settings), None);
        assert_eq!(
            min_output_tokens(&settings),
            Some(REASONING_MIN_OUTPUT_TOKENS)
        );
    }

    #[tokio::test]
    async fn a_url_de_outra_maquina_nao_e_trocada() {
        assert_eq!(
            locate_openai_base("http://10.0.0.5:11434/v1").await,
            "http://10.0.0.5:11434/v1"
        );
    }
}
