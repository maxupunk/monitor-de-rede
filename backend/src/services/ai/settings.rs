//! Configurações do Assistente IA e gerenciamento de drivers.
//!
//! Armazenado em `system_settings`, chave [`STORAGE_KEY`], em formato JSON.

use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    decider::config::AiDeciderSettings, proactive::config::AiProactiveSettings,
    response_style::AiResponseStyle,
};
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
/// O serviço `ollama` do compose, na rede interna: o único container de IA,
/// que atende o chat e o Decisor (`decider::config::DEFAULT_DECIDER_BASE_URL`).
pub const DEFAULT_OLLAMA_URL: &str = "http://ollama:11434";
/// O mesmo Ollama pela API compatível com a OpenAI, que o driver do chat usa.
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://ollama:11434/v1";
pub const DEFAULT_OLLAMA_MODEL: &str = "ornith-1.5:9b";
pub const DEFAULT_OLLAMA_NUM_CTX: u64 = 16_384;
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
    #[serde(default = "default_ollama_num_ctx")]
    ollama_num_ctx: Option<u64>,
    #[serde(default = "default_true")]
    allow_active_tools: bool,
    #[serde(default)]
    require_tool_confirmation: bool,
    /// Configuração antiga (antes de `actionMode`): `true` vira "pedir permissão".
    #[serde(default)]
    allow_actions: bool,
    #[serde(default)]
    action_mode: Option<AiAutomationMode>,
    #[serde(default)]
    container_actions: AiAutomationMode,
    #[serde(default)]
    response_style: AiResponseStyle,
    #[serde(default)]
    proactive: AiProactiveSettings,
    /// Gravada como `laya` antes do renome para Decisor.
    #[serde(default, alias = "laya")]
    decider: AiDeciderSettings,
    #[serde(default)]
    custom_system_prompt: Option<String>,
}

/// Quanto a IA pode fazer sozinha numa família de ferramentas (ações,
/// containers).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiAutomationMode {
    /// A IA não usa essas ferramentas.
    Off,
    /// A IA propõe e o usuário confirma no chat, sempre.
    #[default]
    Confirm,
    /// Automático com critério: o que é de risco baixo ou médio roda na hora
    /// (aparece no chat e na auditoria); o de risco alto — ou o que o Decisor
    /// julgar perigoso no contexto — volta a pedir confirmação
    /// (`harness::tools::risk`).
    Auto,
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

    /// Janela de contexto solicitada ao Ollama (num_ctx), padrão 16.384 tokens.
    #[ts(type = "number | null")]
    pub ollama_num_ctx: Option<u64>,

    /// Permite executar ferramentas ativas (ping, traceroute, scan de portas)
    pub allow_active_tools: bool,

    /// Exige confirmação do usuário antes de rodar ferramentas ativas
    pub require_tool_confirmation: bool,

    /// Ações (reconhecer/silenciar alerta, janela de manutenção, criar
    /// monitor, regras de alerta): desligadas (padrão), com confirmação ou
    /// automáticas com critério de risco.
    pub action_mode: AiAutomationMode,

    /// Iniciar, parar e reiniciar containers: desligado, com confirmação
    /// (padrão) ou automático com critério de risco.
    pub container_actions: AiAutomationMode,

    /// Quanto texto a IA devolve: direto (mínimo de tokens) ou normal.
    pub response_style: AiResponseStyle,

    /// O que a IA faz sozinha: resumo de incidente e resumo periódico.
    pub proactive: AiProactiveSettings,

    /// Decisor (via Ollama): decide antes do chat quais ferramentas mandar.
    pub decider: AiDeciderSettings,

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
            ollama_num_ctx: h.ollama_num_ctx,
            allow_active_tools: h.allow_active_tools,
            require_tool_confirmation: h.require_tool_confirmation,
            action_mode: h.action_mode.unwrap_or(if h.allow_actions {
                AiAutomationMode::Confirm
            } else {
                AiAutomationMode::Off
            }),
            container_actions: h.container_actions,
            response_style: h.response_style,
            proactive: h.proactive,
            decider: h.decider,
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

fn default_ollama_num_ctx() -> Option<u64> {
    Some(DEFAULT_OLLAMA_NUM_CTX)
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
            ollama_num_ctx: Some(DEFAULT_OLLAMA_NUM_CTX),
            allow_active_tools: true,
            require_tool_confirmation: false,
            action_mode: AiAutomationMode::Off,
            container_actions: AiAutomationMode::default(),
            response_style: AiResponseStyle::default(),
            proactive: AiProactiveSettings::default(),
            decider: AiDeciderSettings::default(),
            custom_system_prompt: None,
        }
    }
}

impl AiSettings {
    /// Devolve uma cópia segura para ser enviada ao frontend, com chaves secretas mascaradas.
    pub fn masked_view(&self) -> Self {
        let mut view = self.clone();
        mask_secret(&mut view.opencode_api_key);
        mask_secret(&mut view.openrouter_api_key);
        view
    }

    /// Mescla os valores mascarados enviados pelo frontend com os valores reais persistidos.
    pub fn merge_unmasked(&mut self, existing: &AiSettings) {
        restore_secret(&mut self.opencode_api_key, &existing.opencode_api_key);
        restore_secret(&mut self.openrouter_api_key, &existing.openrouter_api_key);
    }
}

/// Troca uma chave preenchida pela máscara, para ir ao frontend.
pub fn mask_secret(key: &mut Option<String>) {
    if key.as_deref().is_some_and(|key| !key.trim().is_empty()) {
        *key = Some(MASKED_KEY.to_string());
    }
}

/// A máscara voltou do frontend: vale a chave que já estava gravada.
pub fn restore_secret(key: &mut Option<String>, stored: &Option<String>) {
    if key.as_deref() == Some(MASKED_KEY) {
        key.clone_from(stored);
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
    new_settings.proactive = new_settings.proactive.normalized();
    // O Decisor tem card e endpoint próprios (`save_decider`): o formulário do
    // provedor não o altera, nem com uma cópia desatualizada.
    new_settings.decider = existing.decider;

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
            if new_settings.ollama_num_ctx.is_none() || new_settings.ollama_num_ctx == Some(0) {
                new_settings.ollama_num_ctx = Some(DEFAULT_OLLAMA_NUM_CTX);
            }
        }
        outro => {
            return Err(AppError::validation(format!(
                "Driver desconhecido: '{outro}'. Use 'opencode', 'openrouter' ou 'ollama'."
            )));
        }
    }

    persist(db, &new_settings).await?;
    Ok(new_settings)
}

/// Grava só a parte do Decisor, sem passar pelas validações do provedor: um
/// provedor mal configurado não pode impedir de ligar ou desligar o Decisor.
/// Devolve como a tela vê: a chave mascarada.
pub async fn save_decider(
    db: &DatabaseConnection,
    decider: AiDeciderSettings,
) -> AppResult<AiDeciderSettings> {
    let mut settings = load(db).await?;
    settings.decider = with_stored_key(decider, &settings.decider).normalized();
    persist(db, &settings).await?;
    Ok(masked_decider(settings.decider))
}

/// O Decisor como a tela vê: a chave mascarada.
#[must_use]
pub fn masked_decider(mut decider: AiDeciderSettings) -> AiDeciderSettings {
    mask_secret(&mut decider.api_key);
    decider
}

/// O formulário do Decisor pronto para uso (teste, download, lista de modelos):
/// a máscara volta a ser a chave gravada.
pub async fn resolve_decider<C: ConnectionTrait>(
    db: &C,
    form: AiDeciderSettings,
) -> AppResult<AiDeciderSettings> {
    Ok(with_stored_key(form, &load(db).await?.decider).normalized())
}

/// A máscara vira a chave gravada — só se o provedor é o mesmo: trocar de
/// provedor sem digitar chave nova nunca manda a chave de um para o outro.
fn with_stored_key(mut form: AiDeciderSettings, stored: &AiDeciderSettings) -> AiDeciderSettings {
    if form.provider == stored.provider {
        restore_secret(&mut form.api_key, &stored.api_key);
    } else if form.api_key.as_deref() == Some(MASKED_KEY) {
        form.api_key = None;
    }
    form
}

async fn persist(db: &DatabaseConnection, settings: &AiSettings) -> AppResult<()> {
    let texto = serde_json::to_string(settings)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(texto)).await?;
    Ok(())
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

    #[test]
    fn chave_do_decider_volta_da_mascara_so_no_mesmo_provedor() {
        use crate::services::ai::decider::providers::DeciderProvider;
        let stored = AiDeciderSettings {
            provider: DeciderProvider::Typesafe,
            api_key: Some("ts-secreta".into()),
            ..AiDeciderSettings::default()
        };
        let mesma = with_stored_key(masked_decider(stored.clone()), &stored);
        assert_eq!(mesma.api_key.as_deref(), Some("ts-secreta"));

        let trocou = AiDeciderSettings {
            provider: DeciderProvider::Openai,
            ..masked_decider(stored.clone())
        };
        assert_eq!(
            with_stored_key(trocou, &stored).api_key,
            None,
            "a chave da TypeSafe não vai para a OpenAI"
        );
        assert_eq!(masked_decider(stored).api_key.as_deref(), Some(MASKED_KEY));
    }

    #[test]
    fn configuracao_gravada_como_laya_continua_valendo() {
        let antiga: AiSettings =
            serde_json::from_str(r#"{ "laya": { "enabled": true, "model": "laya:en" } }"#).unwrap();
        assert!(antiga.decider.enabled);
        assert_eq!(antiga.decider.model, "laya:en");
        let gravada = serde_json::to_value(&antiga).unwrap();
        assert!(gravada.get("decider").is_some() && gravada.get("laya").is_none());
    }

    #[test]
    fn provedor_desconhecido_no_decisor_nao_zera_a_configuracao_do_chat() {
        use crate::services::ai::decider::providers::DeciderProvider;

        let gravada: AiSettings = serde_json::from_str(
            r#"{
                "activeDriver": "openrouter",
                "openrouterApiKey": "chave-guardada",
                "ollamaModel": "qwen2.5:7b",
                "decider": { "enabled": true, "provider": "desconhecido", "minConfidence": 70 }
            }"#,
        )
        .expect("um provedor desconhecido não invalida a configuração");
        assert_eq!(gravada.active_driver, "openrouter");
        assert_eq!(
            gravada.openrouter_api_key.as_deref(),
            Some("chave-guardada")
        );
        assert_eq!(gravada.ollama_model.as_deref(), Some("qwen2.5:7b"));
        assert_eq!(gravada.decider.provider, DeciderProvider::default());
        assert!(gravada.decider.enabled);
        assert_eq!(gravada.decider.min_confidence, 70);
    }

    #[test]
    fn acoes_em_container_pedem_confirmacao_por_padrao() {
        let antiga: AiSettings = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert_eq!(antiga.container_actions, AiAutomationMode::Confirm);
        assert_eq!(
            AiSettings::default().container_actions,
            AiAutomationMode::Confirm
        );

        let auto: AiSettings = serde_json::from_str(r#"{ "containerActions": "auto" }"#).unwrap();
        assert_eq!(auto.container_actions, AiAutomationMode::Auto);
    }

    #[test]
    fn configuracao_antiga_sem_estilo_assume_o_modo_direto() {
        let settings: AiSettings = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert_eq!(settings.response_style, AiResponseStyle::Concise);

        let normal: AiSettings = serde_json::from_str(r#"{ "responseStyle": "normal" }"#).unwrap();
        assert_eq!(normal.response_style, AiResponseStyle::Normal);
    }

    #[test]
    fn configuracao_antiga_nao_libera_acoes_nem_rotinas_automaticas() {
        let settings: AiSettings = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert_eq!(settings.action_mode, AiAutomationMode::Off);
        assert!(!settings.proactive.incident_summaries);
    }

    #[test]
    fn acoes_liberadas_na_configuracao_antiga_continuam_pedindo_permissao() {
        let antiga: AiSettings = serde_json::from_str(r#"{ "allowActions": true }"#).unwrap();
        assert_eq!(antiga.action_mode, AiAutomationMode::Confirm);

        let nova: AiSettings =
            serde_json::from_str(r#"{ "allowActions": true, "actionMode": "auto" }"#).unwrap();
        assert_eq!(nova.action_mode, AiAutomationMode::Auto);

        let gravada = serde_json::to_value(&nova).unwrap();
        assert_eq!(gravada["actionMode"], "auto");
        assert!(gravada.get("allowActions").is_none());
    }
}
