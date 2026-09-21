//! Configurações do Assistente IA e gerenciamento de drivers.
//!
//! Armazenado em `system_settings`, chave [`STORAGE_KEY`], em formato JSON.

use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::system_settings,
    services::shared::errors::{AppError, AppResult},
};

/// Chave em `system_settings`.
pub const STORAGE_KEY: &str = "ai.settings";

/// Valor exibido para chaves mascaradas no frontend.
pub const MASKED_KEY: &str = "********";

pub const OPENCODE_DEFAULT_BASE_URL: &str = "https://opencode.ai/zen/v1";
pub const DEFAULT_OPENCODE_MODEL: &str = "muse-spark-1.3-contributor-free";
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";
pub const DEFAULT_OPENROUTER_MODEL: &str = "openrouter/free";

/// Deserializa campos de modelo de forma resiliente, aceitando tanto uma `String`
/// direta ("slug") quanto um objeto JSON contendo `{ "id": "slug", ... }` ou `{ "name": "slug", ... }`.
pub fn deserialize_optional_model_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ModelInput {
        String(String),
        ObjectWithId { id: String },
        ObjectWithName { name: String },
        Fallback(serde_json::Value),
    }

    let opt = Option::<ModelInput>::deserialize(deserializer)?;
    Ok(opt.and_then(|input| {
        let val = match input {
            ModelInput::String(s) => s,
            ModelInput::ObjectWithId { id } => id,
            ModelInput::ObjectWithName { name } => name,
            ModelInput::Fallback(val) => val
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| val.get("name").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string(),
        };
        let trimmed = val.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiSettingsHelper {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_driver")]
    active_driver: String,
    #[serde(default = "default_opencode_base_url")]
    opencode_base_url: Option<String>,
    #[serde(default)]
    opencode_api_key: Option<String>,
    #[serde(
        default = "default_opencode_model",
        deserialize_with = "deserialize_optional_model_id"
    )]
    opencode_model: Option<String>,
    #[serde(default)]
    openrouter_api_key: Option<String>,
    #[serde(
        default = "default_openrouter_model",
        deserialize_with = "deserialize_optional_model_id"
    )]
    openrouter_model: Option<String>,
    #[serde(default = "default_ollama_base_url")]
    ollama_base_url: Option<String>,
    #[serde(
        default = "default_ollama_model",
        deserialize_with = "deserialize_optional_model_id"
    )]
    ollama_model: Option<String>,
    #[serde(default = "default_true")]
    allow_active_tools: bool,
    #[serde(default)]
    require_tool_confirmation: bool,
    #[serde(default)]
    custom_system_prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiSettings {
    pub enabled: bool,

    /// Driver ativo: "opencode" | "openrouter" | "ollama"
    pub active_driver: String,

    pub opencode_base_url: Option<String>,

    pub opencode_api_key: Option<String>,

    pub opencode_model: Option<String>,

    pub openrouter_api_key: Option<String>,

    pub openrouter_model: Option<String>,

    pub ollama_base_url: Option<String>,

    pub ollama_model: Option<String>,

    /// Permite executar ferramentas ativas (ping, traceroute, scan de portas)
    pub allow_active_tools: bool,

    /// Exige confirmação do usuário antes de rodar ferramentas ativas
    pub require_tool_confirmation: bool,

    pub custom_system_prompt: Option<String>,
}

impl<'de> Deserialize<'de> for AiSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let h = AiSettingsHelper::deserialize(deserializer)?;
        Ok(Self {
            enabled: h.enabled,
            active_driver: h.active_driver,
            opencode_base_url: h.opencode_base_url,
            opencode_api_key: h.opencode_api_key,
            opencode_model: h.opencode_model,
            openrouter_api_key: h.openrouter_api_key,
            openrouter_model: h.openrouter_model,
            ollama_base_url: h.ollama_base_url,
            ollama_model: h.ollama_model,
            allow_active_tools: h.allow_active_tools,
            require_tool_confirmation: h.require_tool_confirmation,
            custom_system_prompt: h.custom_system_prompt,
        })
    }
}

fn default_driver() -> String {
    "ollama".to_string()
}

fn default_opencode_base_url() -> Option<String> {
    Some(OPENCODE_DEFAULT_BASE_URL.to_string())
}

fn default_opencode_model() -> Option<String> {
    Some(DEFAULT_OPENCODE_MODEL.to_string())
}

fn default_openrouter_model() -> Option<String> {
    Some(DEFAULT_OPENROUTER_MODEL.to_string())
}

fn default_ollama_base_url() -> Option<String> {
    Some(DEFAULT_OLLAMA_BASE_URL.to_string())
}

fn default_ollama_model() -> Option<String> {
    Some(DEFAULT_OLLAMA_MODEL.to_string())
}

const fn default_true() -> bool {
    true
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            active_driver: default_driver(),
            opencode_base_url: default_opencode_base_url(),
            opencode_api_key: None,
            opencode_model: default_opencode_model(),
            openrouter_api_key: None,
            openrouter_model: default_openrouter_model(),
            ollama_base_url: default_ollama_base_url(),
            ollama_model: default_ollama_model(),
            allow_active_tools: true,
            require_tool_confirmation: false,
            custom_system_prompt: None,
        }
    }
}

impl AiSettings {
    /// Devolve uma cópia segura para ser enviada ao frontend, com chaves secretas mascaradas.
    pub fn masked_view(&self) -> Self {
        let mut view = self.clone();
        if let Some(key) = &view.opencode_api_key {
            if !key.trim().is_empty() {
                view.opencode_api_key = Some(MASKED_KEY.to_string());
            }
        }
        if let Some(key) = &view.openrouter_api_key {
            if !key.trim().is_empty() {
                view.openrouter_api_key = Some(MASKED_KEY.to_string());
            }
        }
        view
    }

    /// Mescla os valores mascarados enviados pelo frontend com os valores reais persistidos.
    pub fn merge_unmasked(&mut self, existing: &AiSettings) {
        if self.opencode_api_key.as_deref() == Some(MASKED_KEY) {
            self.opencode_api_key = existing.opencode_api_key.clone();
        }
        if self.openrouter_api_key.as_deref() == Some(MASKED_KEY) {
            self.openrouter_api_key = existing.openrouter_api_key.clone();
        }
    }
}

/// Carrega as configurações de IA. Se não existir, retorna os padrões.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<AiSettings> {
    let mut s: AiSettings = system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|linha| linha.value)
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default();

    // Migração de modelos obsoletos / descontinuados no OpenRouter
    if s.openrouter_model.as_deref() == Some("meta-llama/llama-3.3-70b-instruct:free") {
        s.openrouter_model = Some(DEFAULT_OPENROUTER_MODEL.to_string());
    }

    Ok(s)
}

/// Salva as configurações de IA, mesclando chaves mascaradas com as existentes.
pub async fn save(db: &DatabaseConnection, mut new_settings: AiSettings) -> AppResult<AiSettings> {
    let existing = load(db).await?;
    new_settings.merge_unmasked(&existing);

    // Validações básicas
    match new_settings.active_driver.as_str() {
        "opencode" => {
            if new_settings
                .opencode_base_url
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                new_settings.opencode_base_url = Some(OPENCODE_DEFAULT_BASE_URL.to_string());
            }
            if new_settings
                .opencode_model
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                new_settings.opencode_model = Some(DEFAULT_OPENCODE_MODEL.to_string());
            }
            if new_settings.enabled
                && new_settings
                    .opencode_api_key
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
            {
                return Err(AppError::validation(
                    "Informe a API Key para o provedor OpenCode Go / Zen",
                ));
            }
        }
        "openrouter" => {
            if new_settings
                .openrouter_model
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
                || new_settings.openrouter_model.as_deref()
                    == Some("meta-llama/llama-3.3-70b-instruct:free")
            {
                new_settings.openrouter_model = Some(DEFAULT_OPENROUTER_MODEL.to_string());
            }
            if new_settings.enabled
                && new_settings
                    .openrouter_api_key
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
            {
                return Err(AppError::validation(
                    "Informe a API Key para o provedor OpenRouter",
                ));
            }
        }
        "ollama" => {
            if new_settings
                .ollama_model
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                new_settings.ollama_model = Some(DEFAULT_OLLAMA_MODEL.to_string());
            }
            if new_settings.enabled
                && new_settings
                    .ollama_base_url
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
            {
                return Err(AppError::validation("Informe a URL Base para o Ollama"));
            }
        }
        outro => {
            return Err(AppError::validation(format!(
                "Driver desconhecido: '{outro}'. Use 'opencode', 'openrouter' ou 'ollama'."
            )));
        }
    }

    let texto = serde_json::to_string(&new_settings)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(texto)).await?;
    Ok(new_settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_ai_settings_with_object_models() {
        let json_data = r#"{
            "enabled": true,
            "activeDriver": "openrouter",
            "opencodeBaseUrl": "https://opencode.ai/zen/v1",
            "opencodeApiKey": null,
            "opencodeModel": "zen-1",
            "openrouterApiKey": "test-openrouter-key-12345",
            "openrouterModel": {
                "id": "google/gemma-4-26b-a4b-it:free",
                "name": "Google: Gemma 4 26B A4B  (free)",
                "isFree": true,
                "description": "Gemma 4 26B A4B IT is an instruction-tuned...",
                "supportsTools": true
            },
            "ollamaBaseUrl": "http://10.0.0.200:11434/v1",
            "ollamaModel": "gemma4:e4b",
            "allowActiveTools": true,
            "requireToolConfirmation": false,
            "customSystemPrompt": null
        }"#;

        let settings: AiSettings =
            serde_json::from_str(json_data).expect("Must deserialize settings successfully");
        assert_eq!(settings.active_driver, "openrouter");
        assert_eq!(
            settings.openrouter_model.as_deref(),
            Some("google/gemma-4-26b-a4b-it:free")
        );
        assert_eq!(settings.ollama_model.as_deref(), Some("gemma4:e4b"));
    }

    #[test]
    fn test_deserialize_ai_settings_with_plain_string_models() {
        let json_data = r#"{
            "enabled": true,
            "activeDriver": "openrouter",
            "openrouterModel": "meta-llama/llama-3.3-70b-instruct"
        }"#;

        let settings: AiSettings =
            serde_json::from_str(json_data).expect("Must deserialize settings successfully");
        assert_eq!(
            settings.openrouter_model.as_deref(),
            Some("meta-llama/llama-3.3-70b-instruct")
        );
    }

    #[test]
    fn test_deserialize_ai_settings_with_name_object_model() {
        let json_data = r#"{
            "ollamaModel": {
                "name": "qwen2.5:7b"
            }
        }"#;

        let settings: AiSettings =
            serde_json::from_str(json_data).expect("Must deserialize settings successfully");
        assert_eq!(settings.ollama_model.as_deref(), Some("qwen2.5:7b"));
    }

    #[test]
    fn test_deserialize_ai_settings_with_null_and_empty_models() {
        let json_data = r#"{
            "openrouterModel": null,
            "opencodeModel": "",
            "ollamaModel": "   "
        }"#;

        let settings: AiSettings =
            serde_json::from_str(json_data).expect("Must deserialize settings successfully");
        assert_eq!(settings.openrouter_model, None);
        assert_eq!(settings.opencode_model, None);
        assert_eq!(settings.ollama_model, None);
    }
}
