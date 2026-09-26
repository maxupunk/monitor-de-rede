//! Integração com a API nativa do Ollama para gerenciamento e instalação de modelos.
//!
//! Fornece consulta de modelos instalados (`/api/tags`), catálogo de modelos
//! recomendados para o NetMonitor e streaming em tempo real do progresso de
//! download (`/api/pull`).

use std::time::Duration;

use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    dtos::ai::{OllamaModelItem, OllamaModelsResponse, OllamaPullProgress, OllamaRecommendedModel},
    services::shared::errors::{AppError, AppResult},
};

/// Modelos recomendados para execução no Ollama com o NetMonitor.
pub static RECOMMENDED_MODELS: &[(&str, &str, &str, &str, bool)] = &[
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

    let installed_names_lower: Vec<String> = installed_items
        .iter()
        .map(|m| m.name.to_lowercase())
        .collect();

    let recommended: Vec<OllamaRecommendedModel> = RECOMMENDED_MODELS
        .iter()
        .map(
            |&(name, desc, param_size, context_window, tool_optimized)| {
                let name_lower = name.to_lowercase();
                let is_installed = installed_names_lower.iter().any(|inst| {
                    inst == &name_lower
                        || inst == &format!("{name_lower}:latest")
                        || (name_lower.ends_with(":latest")
                            && inst == &name_lower[..name_lower.len() - 7])
                        || inst.starts_with(&format!("{name_lower}:"))
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

#[derive(Debug, Deserialize)]
struct OllamaPullRaw {
    status: Option<String>,
    digest: Option<String>,
    total: Option<u64>,
    completed: Option<u64>,
    error: Option<String>,
}

/// Inicia o download (pull) de um modelo no Ollama e transmite o progresso via stream assíncrono.
pub async fn pull_model_stream(
    base_url: &str,
    model: &str,
) -> AppResult<ReceiverStream<OllamaPullProgress>> {
    let root_url = normalize_ollama_base_url(base_url);
    let pull_url = format!("{root_url}/api/pull");
    let model_name = model.trim().to_string();

    if model_name.is_empty() {
        return Err(AppError::validation("Nome do modelo não pode ser vazio"));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3600)) // Downloads de modelos grandes podem levar vários minutos
        .build()
        .map_err(|e| AppError::service_unavailable(format!("Erro ao criar cliente HTTP: {e}")))?;

    let res = client
        .post(&pull_url)
        .json(&serde_json::json!({
            "name": model_name,
            "stream": true
        }))
        .send()
        .await
        .map_err(|e| {
            AppError::service_unavailable(format!(
                "Não foi possível conectar ao Ollama em '{root_url}': {e}"
            ))
        })?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(AppError::service_unavailable(format!(
            "Ollama retornou status {status} ao tentar baixar o modelo: {body}"
        )));
    }

    let (tx, rx) = mpsc::channel::<OllamaPullProgress>(100);

    tokio::spawn(async move {
        let mut byte_stream = res.bytes_stream();
        let mut line_buffer = String::new();

        while let Some(item) = byte_stream.next().await {
            match item {
                Ok(bytes) => {
                    let chunk_str = String::from_utf8_lossy(&bytes);
                    line_buffer.push_str(&chunk_str);

                    while let Some(pos) = line_buffer.find('\n') {
                        let line = line_buffer[..pos].trim().to_string();
                        line_buffer.drain(..=pos);

                        if line.is_empty() {
                            continue;
                        }

                        if let Ok(raw) = serde_json::from_str::<OllamaPullRaw>(&line) {
                            if let Some(err) = raw.error {
                                let _ = tx
                                    .send(OllamaPullProgress {
                                        status: "Erro".to_string(),
                                        digest: None,
                                        total: None,
                                        completed: None,
                                        percentage: None,
                                        done: true,
                                        error: Some(err),
                                    })
                                    .await;
                                return;
                            }

                            let status_text =
                                raw.status.unwrap_or_else(|| "Processando...".to_string());
                            let is_success = status_text.to_lowercase() == "success";

                            let percentage = match (raw.completed, raw.total) {
                                (Some(comp), Some(tot)) if tot > 0 => {
                                    let pct = ((comp as f64) / (tot as f64)) * 100.0;
                                    Some((pct * 10.0).round() / 10.0)
                                }
                                _ => None,
                            };

                            let progress = OllamaPullProgress {
                                status: status_text,
                                digest: raw.digest,
                                total: raw.total,
                                completed: raw.completed,
                                percentage,
                                done: is_success,
                                error: None,
                            };

                            let is_done = progress.done;
                            if tx.send(progress).await.is_err() {
                                return;
                            }

                            if is_done {
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(OllamaPullProgress {
                            status: "Falha de rede".to_string(),
                            digest: None,
                            total: None,
                            completed: None,
                            percentage: None,
                            done: true,
                            error: Some(format!("Erro na transmissão de dados: {e}")),
                        })
                        .await;
                    return;
                }
            }
        }

        // Se terminou o stream sem enviar success
        let _ = tx
            .send(OllamaPullProgress {
                status: "Finalizado".to_string(),
                digest: None,
                total: None,
                completed: None,
                percentage: Some(100.0),
                done: true,
                error: None,
            })
            .await;
    });

    Ok(ReceiverStream::new(rx))
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
