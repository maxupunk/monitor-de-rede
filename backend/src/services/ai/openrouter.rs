//! Cliente e catálogo de modelos para o provedor OpenRouter.

use std::time::Duration;

use serde::Deserialize;

use crate::dtos::ai::{OpenRouterModelItem, OpenRouterModelsResponse};

/// Catálogo curado de modelos populares e gratuitos do OpenRouter.
pub fn curated_openrouter_models() -> Vec<OpenRouterModelItem> {
    vec![
        OpenRouterModelItem {
            id: "openrouter/free".to_string(),
            name: "Free Models Router (Automático Gratuito)".to_string(),
            is_free: true,
            description: Some(
                "Roteia automaticamente para o melhor modelo gratuito disponível com suporte a ferramentas"
                    .to_string(),
            ),
            context_length: Some(128_000),
            supports_tools: Some(true),
        },
        OpenRouterModelItem {
            id: "google/gemma-4-31b-it:free".to_string(),
            name: "Google: Gemma 4 31B (Gratuito)".to_string(),
            is_free: true,
            description: Some("Excelente raciocínio e suporte gratuito".to_string()),
            context_length: Some(131_072),
            supports_tools: Some(true),
        },
        OpenRouterModelItem {
            id: "qwen/qwen3.8-27b:free".to_string(),
            name: "Qwen: Qwen 3.8 27B (Gratuito)".to_string(),
            is_free: true,
            description: Some("Alta performance em código e raciocínio técnico".to_string()),
            context_length: Some(131_072),
            supports_tools: Some(true),
        },
        OpenRouterModelItem {
            id: "nvidia/nemotron-3.5-lightning:free".to_string(),
            name: "NVIDIA: Nemotron 3.5 Lightning (Gratuito)".to_string(),
            is_free: true,
            description: Some("Velocidade e precisão para diagnósticos de rede".to_string()),
            context_length: Some(131_072),
            supports_tools: Some(true),
        },
        OpenRouterModelItem {
            id: "meta-llama/llama-3.3-70b-instruct".to_string(),
            name: "Meta: Llama 3.3 70B Instruct (Padrão / Créditos)".to_string(),
            is_free: false,
            description: Some("Modelo recomendado da Meta para alta complexidade".to_string()),
            context_length: Some(131_072),
            supports_tools: Some(true),
        },
        OpenRouterModelItem {
            id: "openai/gpt-4o-mini".to_string(),
            name: "OpenAI: GPT-4o Mini".to_string(),
            is_free: false,
            description: Some("Rápido, econômico e altamente capaz".to_string()),
            context_length: Some(128_000),
            supports_tools: Some(true),
        },
        OpenRouterModelItem {
            id: "anthropic/claude-3.5-sonnet".to_string(),
            name: "Anthropic: Claude 3.5 Sonnet".to_string(),
            is_free: false,
            description: Some("Estado da arte em raciocínio e engenharia".to_string()),
            context_length: Some(200_000),
            supports_tools: Some(true),
        },
        OpenRouterModelItem {
            id: "deepseek/deepseek-chat".to_string(),
            name: "DeepSeek: DeepSeek Chat (V3)".to_string(),
            is_free: false,
            description: Some("Excelente custo-benefício em análise de sistemas".to_string()),
            context_length: Some(64_000),
            supports_tools: Some(true),
        },
    ]
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsPayload {
    data: Vec<OpenRouterRawEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterRawEntry {
    id: String,
    name: Option<String>,
    description: Option<String>,
    pricing: Option<OpenRouterPricing>,
    context_length: Option<u64>,
    supported_parameters: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterPricing {
    prompt: Option<String>,
    completion: Option<String>,
}

/// Consulta os modelos disponíveis no OpenRouter (`GET /api/v1/models`).
pub async fn list_models(api_key: Option<&str>) -> OpenRouterModelsResponse {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return OpenRouterModelsResponse {
                models: curated_openrouter_models(),
                is_online: false,
                error_message: Some(format!("Erro ao criar cliente HTTP: {e}")),
            };
        }
    };

    let mut req = client.get("https://openrouter.ai/api/v1/models");
    if let Some(key) = api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            req = req.header("Authorization", format!("Bearer {trimmed}"));
        }
    }

    let response = match req.send().await {
        Ok(resp) => resp,
        Err(e) => {
            return OpenRouterModelsResponse {
                models: curated_openrouter_models(),
                is_online: false,
                error_message: Some(format!("Não foi possível conectar ao OpenRouter: {e}")),
            };
        }
    };

    let status = response.status();
    if !status.is_success() {
        return OpenRouterModelsResponse {
            models: curated_openrouter_models(),
            is_online: false,
            error_message: Some(format!("OpenRouter retornou status {status}")),
        };
    }

    let payload: OpenRouterModelsPayload = match response.json().await {
        Ok(data) => data,
        Err(e) => {
            return OpenRouterModelsResponse {
                models: curated_openrouter_models(),
                is_online: false,
                error_message: Some(format!("Falha ao interpretar resposta do OpenRouter: {e}")),
            };
        }
    };

    let mut free_models = Vec::new();
    let mut paid_models = Vec::new();

    // Sempre insere o router automático primeiro
    free_models.push(OpenRouterModelItem {
        id: "openrouter/free".to_string(),
        name: "Free Models Router (Automático Gratuito)".to_string(),
        is_free: true,
        description: Some(
            "Roteia automaticamente para o melhor modelo gratuito disponível com suporte a ferramentas"
                .to_string(),
        ),
        context_length: Some(128_000),
        supports_tools: Some(true),
    });

    for entry in payload.data {
        let is_free = entry.id.contains(":free")
            || entry
                .pricing
                .as_ref()
                .map(|p| p.prompt.as_deref() == Some("0") && p.completion.as_deref() == Some("0"))
                .unwrap_or(false);

        if entry.id == "openrouter/free" {
            continue;
        }

        let supports_tools = entry
            .supported_parameters
            .as_ref()
            .map(|p| p.iter().any(|param| param == "tools"));

        let item = OpenRouterModelItem {
            name: entry.name.unwrap_or_else(|| entry.id.clone()),
            id: entry.id,
            is_free,
            description: entry.description,
            context_length: entry.context_length,
            supports_tools,
        };

        if is_free {
            free_models.push(item);
        } else {
            paid_models.push(item);
        }
    }

    // Unifica priorizando modelos gratuitos, seguidos pelos demais
    free_models.extend(paid_models);

    OpenRouterModelsResponse {
        models: free_models,
        is_online: true,
        error_message: None,
    }
}
