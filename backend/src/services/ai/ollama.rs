//! Integração com a API nativa do Ollama para gerenciamento e instalação de modelos.
//!
//! Fornece consulta de modelos instalados (`/api/tags`), catálogo de modelos
//! recomendados para o NetMonitor e streaming em tempo real do progresso de
//! download (`/api/pull`).

use std::time::Duration;

use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;

use super::local_models::{self, PullTarget};
use crate::{
    dtos::ai::{ModelPullProgress, OllamaModelItem, OllamaModelsResponse, OllamaRecommendedModel},
    services::shared::errors::{AppError, AppResult},
};

/// Modelos recomendados para execução no Ollama com o NetMonitor.
pub static RECOMMENDED_MODELS: &[(&str, &str, &str, &str, bool)] = &[
    (
        "ornith-1.5:9b",
        "Ornith 1.5 9B — Modelo mais recomendado para o NetMonitor: autoaperfeiçoamento, raciocínio avançado, ferramentas de rede e janela nativa de 256k",
        "9B",
        "256k",
        true,
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
    let root_url = normalize_ollama_base_url(base_url);
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
                let is_installed = local_models::is_installed(&installed_names, name);

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
pub async fn pull_model_stream(
    base_url: &str,
    model: &str,
) -> AppResult<ReceiverStream<ModelPullProgress>> {
    let model_name = model.trim();
    if model_name.is_empty() {
        return Err(AppError::validation("Nome do modelo não pode ser vazio"));
    }
    local_models::stream_pull(PullTarget {
        url: format!("{}/api/pull", normalize_ollama_base_url(base_url)),
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
}
