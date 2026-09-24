pub mod openai_compatible;
pub mod traits;

use std::sync::Arc;

use crate::{
    services::ai::{
        context_window::sources::{
            ModelsDevLookup, OllamaLookup, OpenRouterLookup, MODELS_DEV_URL,
        },
        settings::AiSettings,
    },
    services::shared::errors::{AppError, AppResult},
};
use openai_compatible::OpenAiCompatibleDriver;
use traits::AiDriver;

const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

pub fn create_driver(settings: &AiSettings) -> AppResult<Box<dyn AiDriver>> {
    match settings.active_driver.as_str() {
        "opencode" => {
            let base_url = settings
                .opencode_base_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| {
                    crate::services::ai::settings::OPENCODE_DEFAULT_BASE_URL.to_string()
                });

            let model = settings
                .opencode_model
                .clone()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| {
                    crate::services::ai::settings::DEFAULT_OPENCODE_MODEL.to_string()
                });

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
                .unwrap_or_else(|| {
                    crate::services::ai::settings::DEFAULT_OPENROUTER_MODEL.to_string()
                });

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
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string());

            let model = settings
                .ollama_model
                .clone()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "llama3.2".to_string());

            let lookup = Arc::new(OllamaLookup::from_openai_base(&base_url));
            Ok(Box::new(
                OpenAiCompatibleDriver::new(
                    "ollama",
                    "Ollama Local",
                    base_url,
                    None,
                    model,
                    vec![],
                )
                .with_window_lookup(lookup),
            ))
        }
        outro => Err(AppError::validation(format!(
            "Driver de IA não suportado: '{outro}'"
        ))),
    }
}
