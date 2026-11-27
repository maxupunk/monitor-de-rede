pub mod openai_compatible;
pub mod traits;

use std::sync::Arc;

use futures::StreamExt;

use crate::{
    context_window::sources::{ModelsDevLookup, OllamaLookup, OpenRouterLookup, MODELS_DEV_URL},
    dtos::TestConnectionInput,
    ollama,
    settings::{self, AiSettings, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_MODEL},
};
use netmonitor_core::shared::errors::{AppError, AppResult};
use openai_compatible::OpenAiCompatibleDriver;
use traits::{AiChatOptions, AiDriver, AiMessage};

const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Monta o driver para `POST /api/ai/test-connection` sem vazar chaves salvas
/// para URLs diferentes da configurada.
///
/// # Errors
///
/// Driver desconhecido ou chave de API ausente.
pub async fn from_test_input(
    existing: &AiSettings,
    input: TestConnectionInput,
) -> AppResult<Option<Box<dyn AiDriver>>> {
    match input.driver.as_str() {
        "opencode" => {
            let base_url = input
                .base_url
                .filter(|u| !u.trim().is_empty())
                .or_else(|| existing.opencode_base_url.clone())
                .unwrap_or_else(|| settings::OPENCODE_DEFAULT_BASE_URL.to_string());
            let mut api_key = input.api_key;
            if api_key.as_deref() == Some(settings::MASKED_KEY) {
                if settings::opencode_url_matches(
                    Some(&base_url),
                    existing.opencode_base_url.as_deref(),
                ) {
                    api_key.clone_from(&existing.opencode_api_key);
                } else {
                    api_key = None;
                }
            }
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| AppError::validation("Informe a API Key do OpenCode Go / Zen"))?;
            let model = if input.model.trim().is_empty() {
                settings::DEFAULT_OPENCODE_MODEL.to_string()
            } else {
                input.model
            };
            Ok(Some(Box::new(OpenAiCompatibleDriver::new(
                "opencode",
                "OpenCode Go / Zen",
                base_url,
                Some(key),
                model,
                vec![],
            ))))
        }
        "openrouter" => {
            let mut api_key = input.api_key;
            if api_key.as_deref() == Some(settings::MASKED_KEY) {
                api_key.clone_from(&existing.openrouter_api_key);
            }
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| AppError::validation("Informe a API Key do OpenRouter"))?;
            let model = if input.model.trim().is_empty()
                || input.model == "meta-llama/llama-3.3-70b-instruct:free"
            {
                settings::DEFAULT_OPENROUTER_MODEL.to_string()
            } else {
                input.model
            };
            Ok(Some(Box::new(OpenAiCompatibleDriver::new(
                "openrouter",
                "OpenRouter",
                OPENROUTER_BASE_URL.to_string(),
                Some(key),
                model,
                vec![
                    (
                        "HTTP-Referer".to_string(),
                        "https://netmonitor.app".to_string(),
                    ),
                    ("X-Title".to_string(), "NetMonitor AI Assistant".to_string()),
                ],
            ))))
        }
        "ollama" => {
            let configured = input
                .base_url
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string());
            let base_url = ollama::locate_openai_base(&configured).await;
            Ok(Some(Box::new(OpenAiCompatibleDriver::new(
                "ollama",
                "Ollama Local",
                base_url,
                None,
                input.model,
                vec![],
            ))))
        }
        _ => Ok(None),
    }
}

/// O driver do provedor configurado, falando com o Ollama onde ele de fato
/// está quando a URL da tela aponta para esta máquina
/// ([`crate::ollama::locate_openai_base`]). É a porta de entrada
/// de quem vai conversar; [`create_driver`] só monta, sem rede.
///
/// # Errors
///
/// Provedor desconhecido ou não configurado.
pub async fn connect(settings: &AiSettings) -> AppResult<Box<dyn AiDriver>> {
    if settings.active_driver != "ollama" {
        return create_driver(settings);
    }
    let configured = settings
        .ollama_base_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or(DEFAULT_OLLAMA_BASE_URL);
    let located = AiSettings {
        ollama_base_url: Some(ollama::locate_openai_base(configured).await),
        ..settings.clone()
    };
    create_driver(&located)
}

/// Uma resposta inteira, sem ferramentas: manda `messages`, junta os deltas
/// de texto do stream e devolve o texto sem espaço nas pontas (vazio quando o
/// provedor não escreveu nada — quem chama decide se isso é erro).
///
/// # Errors
///
/// Provedor indisponível ou erro no meio do stream.
pub async fn complete(
    driver: &dyn AiDriver,
    messages: &[AiMessage],
    options: AiChatOptions,
) -> AppResult<String> {
    let mut stream = driver.chat_stream(messages, &[], options).await?;
    let mut text = String::new();
    while let Some(chunk) = stream.next().await {
        if let Some(delta) = chunk?.text_delta {
            text.push_str(&delta);
        }
    }
    Ok(text.trim().to_string())
}

pub fn create_driver(settings: &AiSettings) -> AppResult<Box<dyn AiDriver>> {
    match settings.active_driver.as_str() {
        "opencode" => {
            let base_url = settings
                .opencode_base_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| crate::settings::OPENCODE_DEFAULT_BASE_URL.to_string());

            let model = settings
                .opencode_model
                .clone()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| crate::settings::DEFAULT_OPENCODE_MODEL.to_string());

            Ok(Box::new(
                OpenAiCompatibleDriver::new(
                    "opencode",
                    "OpenCode Go / Zen",
                    base_url,
                    settings.opencode_api_key.clone(),
                    model,
                    vec![],
                )
                // O `/models` do Zen só lista ids: a janela vem do catálogo
                // `models.dev`, mantido pelo projeto OpenCode.
                .with_window_lookup(Arc::new(ModelsDevLookup {
                    url: MODELS_DEV_URL.to_string(),
                    provider: "opencode".to_string(),
                })),
            ))
        }
        "openrouter" => {
            let api_key = settings
                .openrouter_api_key
                .clone()
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| AppError::validation("API Key do OpenRouter não configurada"))?;

            let model = settings
                .openrouter_model
                .clone()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| crate::settings::DEFAULT_OPENROUTER_MODEL.to_string());

            let lookup = Arc::new(OpenRouterLookup {
                base_url: OPENROUTER_BASE_URL.to_string(),
                api_key: Some(api_key.clone()),
            });
            Ok(Box::new(
                OpenAiCompatibleDriver::new(
                    "openrouter",
                    "OpenRouter",
                    OPENROUTER_BASE_URL.to_string(),
                    Some(api_key),
                    model,
                    vec![
                        (
                            "HTTP-Referer".to_string(),
                            "https://netmonitor.app".to_string(),
                        ),
                        ("X-Title".to_string(), "NetMonitor AI Assistant".to_string()),
                    ],
                )
                .with_window_lookup(lookup),
            ))
        }
        "ollama" => {
            let base_url = settings
                .ollama_base_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string());

            let model = settings
                .ollama_model
                .clone()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());

            let num_ctx = settings
                .ollama_num_ctx
                .unwrap_or(crate::settings::DEFAULT_OLLAMA_NUM_CTX);

            let lookup =
                Arc::new(OllamaLookup::from_openai_base(&base_url).with_num_ctx(Some(num_ctx)));
            Ok(Box::new(
                OpenAiCompatibleDriver::new(
                    "ollama",
                    "Ollama Local",
                    base_url,
                    None,
                    model,
                    vec![],
                )
                .with_num_ctx(num_ctx)
                .with_reasoning_effort(ollama::reasoning_effort(settings))
                .with_min_output_tokens(ollama::min_output_tokens(settings))
                .with_window_lookup(lookup),
            ))
        }
        outro => Err(AppError::validation(format!(
            "Driver de IA não suportado: '{outro}'"
        ))),
    }
}
