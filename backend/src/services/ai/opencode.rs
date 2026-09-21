//! Cliente e serviços de catálogo de modelos para o provedor OpenCode Go / Zen.

use std::time::Duration;

use serde::Deserialize;

use crate::dtos::ai::{OpenCodeModelItem, OpenCodeModelsResponse};

/// Lista padrão curada de modelos suportados pelo OpenCode Zen.
pub fn curated_opencode_models() -> Vec<OpenCodeModelItem> {
    vec![
        OpenCodeModelItem {
            id: "muse-spark-1.3-contributor-free".to_string(),
            name: Some("Muse Spark 1.3 Contributor (Gratuito)".to_string()),
            is_free: true,
            description: Some("Modelo recomendado gratuito com excelente raciocínio".to_string()),
            supports_tools: Some(true),
        },
        OpenCodeModelItem {
            id: "mimo-v2.6-flash-free".to_string(),
            name: Some("Mimo v2.6 Flash (Gratuito)".to_string()),
            is_free: true,
            description: Some(
                "Modelo ultra-rápido gratuito otimizado para chamadas e código".to_string(),
            ),
            supports_tools: Some(true),
        },
        OpenCodeModelItem {
            id: "jev-1.13-free".to_string(),
            name: Some("Jev 1.13 (Gratuito)".to_string()),
            is_free: true,
            description: Some("Modelo gratuito de uso geral para diagnósticos".to_string()),
            supports_tools: Some(true),
        },
        OpenCodeModelItem {
            id: "deepseek-v4.1-flash".to_string(),
            name: Some("DeepSeek v4.1 Flash".to_string()),
            is_free: false,
            description: Some("Alta performance em análise de rede e scripts".to_string()),
            supports_tools: Some(true),
        },
        OpenCodeModelItem {
            id: "gemini-3.8-flash".to_string(),
            name: Some("Gemini 3.8 Flash".to_string()),
            is_free: false,
            description: Some("Latência reduzida e raciocínio avançado".to_string()),
            supports_tools: Some(true),
        },
        OpenCodeModelItem {
            id: "glm-5.3-flash".to_string(),
            name: Some("GLM 5.3 Flash".to_string()),
            is_free: false,
            description: Some("Excelente para tarefas de diagnóstico e suporte".to_string()),
            supports_tools: Some(true),
        },
        OpenCodeModelItem {
            id: "gpt-6-astra".to_string(),
            name: Some("GPT-6 Astra".to_string()),
            is_free: false,
            description: Some("Modelo de ponta para análise profunda e playbooks".to_string()),
            supports_tools: Some(true),
        },
        OpenCodeModelItem {
            id: "qwen3.8-flash".to_string(),
            name: Some("Qwen 3.8 Flash".to_string()),
            is_free: false,
            description: Some("Modelo rápido para consultas e suporte operacional".to_string()),
            supports_tools: Some(true),
        },
    ]
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsListPayload {
    data: Vec<OpenAiModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelEntry {
    id: String,
}

/// Consulta a lista de modelos em tempo real da API do OpenCode (`GET /models`).
pub async fn list_models(base_url: &str, api_key: Option<&str>) -> OpenCodeModelsResponse {
    let clean_url = format!("{}/models", base_url.trim_end_matches('/'));

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return OpenCodeModelsResponse {
                models: curated_opencode_models(),
                is_online: false,
                error_message: Some(format!("Erro ao criar cliente HTTP: {e}")),
            };
        }
    };

    let mut req = client.get(&clean_url);
    if let Some(key) = api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            req = req.header("Authorization", format!("Bearer {trimmed}"));
        }
    }

    let response = match req.send().await {
        Ok(resp) => resp,
        Err(e) => {
            return OpenCodeModelsResponse {
                models: curated_opencode_models(),
                is_online: false,
                error_message: Some(format!(
                    "Não foi possível conectar ao OpenCode em '{clean_url}': {e}"
                )),
            };
        }
    };

    let status = response.status();
    if !status.is_success() {
        return OpenCodeModelsResponse {
            models: curated_opencode_models(),
            is_online: false,
            error_message: Some(format!("OpenCode retornou status {status}")),
        };
    }

    let payload: OpenAiModelsListPayload = match response.json().await {
        Ok(data) => data,
        Err(e) => {
            return OpenCodeModelsResponse {
                models: curated_opencode_models(),
                is_online: false,
                error_message: Some(format!("Falha ao interpretar resposta do OpenCode: {e}")),
            };
        }
    };

    let models = payload
        .data
        .into_iter()
        .map(|entry| {
            let is_free = entry.id.to_lowercase().contains("free");
            OpenCodeModelItem {
                name: Some(entry.id.clone()),
                id: entry.id,
                is_free,
                description: None,
                supports_tools: Some(true),
            }
        })
        .collect();

    OpenCodeModelsResponse {
        models,
        is_online: true,
        error_message: None,
    }
}
