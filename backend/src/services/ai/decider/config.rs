//! Configuração do Decisor: onde está o Ollama e quão seguro ele precisa estar.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::providers::{self, DeciderProvider, DecisionProtocol, ProviderPreset};

/// O serviço `ollama` do `docker-compose.yml`, na rede interna — o mesmo que
/// atende o chat.
pub const DEFAULT_DECIDER_BASE_URL: &str = crate::services::ai::settings::DEFAULT_OLLAMA_URL;
/// O ajustado nas decisões do NetMonitor (`decider::decisions_benchmark`). Vem
/// com o NetMonitor: o próprio servidor o instala no Ollama
/// (`decider::bundled`).
pub const DEFAULT_DECIDER_MODEL: &str = "netmonitor/laya-network:v2";
/// Probabilidade mínima (%) para um grupo de ferramentas entrar.
pub const DEFAULT_DECIDER_MIN_CONFIDENCE: u8 = 60;
/// Tempo máximo de uma decisão. Estourou, o chat segue sem ela.
pub const DEFAULT_DECIDER_TIMEOUT_MS: u32 = 4_000;

/// Campo que tolera valor desconhecido (gravado por outra versão do NetMonitor):
/// ele vira o padrão do tipo em vez de invalidar a configuração inteira — o
/// `settings::load` cai nos padrões quando o JSON não deserializa, e levaria
/// junto o resto do Decisor e as chaves do chat.
mod tolerant {
    use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: Serialize, S: Serializer>(
        value: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.serialize(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: DeserializeOwned + Default,
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(T::deserialize(value).unwrap_or_default())
    }
}

fn trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Triagem da IA proativa: o Decisor decide se um alerta merece resumo do LLM.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DeciderTriageMode {
    /// Não consulta: vale só severidade e teto por hora.
    #[default]
    Off,
    /// Consulta e registra no alerta, mas resume sempre — para calibrar.
    Shadow,
    /// Pula o resumo quando o Decisor acha que o alerta não merece.
    Enforce,
}

/// Onde o Decisor é consultado. Cada frente só sugere; nenhuma muda dado sozinha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeciderFeatures {
    /// Escolher os grupos de ferramentas de cada pergunta do chat.
    pub chat_tools: bool,
    /// Sugerir tipo e sistema dos dispositivos encontrados na descoberta.
    pub device_identity: bool,
    /// Sugerir o uplink e as interfaces que valem monitorar.
    pub interfaces: bool,
    /// Classificar cada padrão novo de log (falha de login, link, reboot…).
    pub log_events: bool,
    pub incident_triage: DeciderTriageMode,
    /// Revisar as ações que o modo automático rodaria sozinho: o que ele achar
    /// perigoso no contexto volta a pedir confirmação. Só sobe o risco.
    pub action_review: bool,
}

impl Default for DeciderFeatures {
    fn default() -> Self {
        Self {
            // Era o único uso: quem já tinha o Decisor ligado continua igual.
            chat_tools: true,
            device_identity: true,
            interfaces: true,
            // Roda em segundo plano sobre todo log novo: só com escolha explícita.
            log_events: false,
            incident_triage: DeciderTriageMode::Off,
            // Só torna o automático mais cauteloso, e só nas ações de risco médio.
            action_review: true,
        }
    }
}

/// Uma frente, para perguntar se ela está liberada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeciderFeature {
    ChatTools,
    DeviceIdentity,
    Interfaces,
    LogEvents,
    IncidentTriage,
    ActionReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiDeciderSettings {
    /// Chave mestra: desligado, nenhuma frente consulta o Decisor.
    pub enabled: bool,
    /// Quem responde: o Ollama (padrão) ou um serviço na nuvem. Desconhecido
    /// vira o padrão.
    #[serde(with = "tolerant")]
    #[ts(as = "DeciderProvider")]
    pub provider: DeciderProvider,
    /// Formato da API no provedor "Outro"; os demais têm o seu. Desconhecido
    /// vira o padrão.
    #[serde(with = "tolerant")]
    #[ts(as = "DecisionProtocol")]
    pub custom_protocol: DecisionProtocol,
    pub base_url: String,
    /// Chave do provedor. Vai mascarada à tela (como as chaves do chat). O
    /// Ollama local não pede; um atrás de proxy com autenticação, sim.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub api_key: Option<String>,
    /// ID da conta (Cloudflare).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub account_id: Option<String>,
    pub model: String,
    /// 1–99: quanto maior, menos grupos entram por palpite do Decisor.
    pub min_confidence: u8,
    /// 200–10.000 ms.
    pub timeout_ms: u32,
    pub features: DeciderFeatures,
}

impl Default for AiDeciderSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: DeciderProvider::Ollama,
            custom_protocol: DecisionProtocol::Systemone,
            base_url: DEFAULT_DECIDER_BASE_URL.to_string(),
            api_key: None,
            account_id: None,
            model: DEFAULT_DECIDER_MODEL.to_string(),
            min_confidence: DEFAULT_DECIDER_MIN_CONFIDENCE,
            timeout_ms: DEFAULT_DECIDER_TIMEOUT_MS,
            features: DeciderFeatures::default(),
        }
    }
}

impl AiDeciderSettings {
    /// Traz valores vazios ou fora da faixa de volta para ela.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        let preset = self.preset();
        let base_url = self.base_url.trim().trim_end_matches('/');
        self.base_url = if base_url.is_empty() {
            preset.default_base_url.to_string()
        } else {
            base_url.to_string()
        };
        let model = self.model.trim();
        self.model = if model.is_empty() {
            preset.default_model.to_string()
        } else {
            model.to_string()
        };
        // O Ollama gerencia modelos; o "Outro" escolhe entre os formatos de nuvem.
        if self.custom_protocol == DecisionProtocol::Ollama {
            self.custom_protocol = DecisionProtocol::Systemone;
        }
        self.api_key = trimmed(self.api_key);
        self.account_id = trimmed(self.account_id);
        self.min_confidence = self.min_confidence.clamp(1, 99);
        self.timeout_ms = self.timeout_ms.clamp(200, 10_000);
        self
    }

    /// O Decisor está ligado e a frente liberada?
    #[must_use]
    pub fn allows(&self, feature: DeciderFeature) -> bool {
        let features = &self.features;
        self.enabled
            && match feature {
                DeciderFeature::ChatTools => features.chat_tools,
                DeciderFeature::DeviceIdentity => features.device_identity,
                DeciderFeature::Interfaces => features.interfaces,
                DeciderFeature::LogEvents => features.log_events,
                DeciderFeature::IncidentTriage => {
                    features.incident_triage != DeciderTriageMode::Off
                }
                DeciderFeature::ActionReview => features.action_review,
            }
    }

    #[must_use]
    pub fn preset(&self) -> &'static ProviderPreset {
        providers::preset(self.provider)
    }

    /// O formato da API de quem responde.
    #[must_use]
    pub fn protocol(&self) -> DecisionProtocol {
        self.preset().protocol.unwrap_or(self.custom_protocol)
    }

    /// Baixa e carrega modelos (só o Ollama).
    #[must_use]
    pub fn manages_models(&self) -> bool {
        self.protocol() == DecisionProtocol::Ollama
    }

    /// A chave a usar, se houver.
    #[must_use]
    pub fn effective_api_key(&self) -> Option<String> {
        trimmed(self.api_key.clone())
    }

    /// Limiar como probabilidade (0–1).
    #[must_use]
    pub fn threshold(&self) -> f64 {
        f64::from(self.min_confidence) / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_valores_vazios_e_fora_da_faixa() {
        let settings = AiDeciderSettings {
            enabled: true,
            base_url: " http://ollama:11434/ ".into(),
            model: String::new(),
            min_confidence: 0,
            timeout_ms: 50_000,
            api_key: Some("  ".into()),
            ..AiDeciderSettings::default()
        }
        .normalized();
        assert_eq!(settings.api_key, None);
        assert_eq!(settings.base_url, "http://ollama:11434");
        assert_eq!(settings.model, DEFAULT_DECIDER_MODEL);
        assert_eq!(settings.min_confidence, 1);
        assert_eq!(settings.timeout_ms, 10_000);
    }

    #[test]
    fn provedor_de_nuvem_sem_url_nem_modelo_recebe_os_do_preset() {
        let settings = AiDeciderSettings {
            provider: DeciderProvider::Cloudflare,
            base_url: String::new(),
            model: String::new(),
            ..AiDeciderSettings::default()
        }
        .normalized();
        assert_eq!(settings.base_url, "https://api.cloudflare.com/client/v4");
        assert_eq!(settings.model, "clef-flash");
        assert_eq!(settings.protocol(), DecisionProtocol::Cloudflare);
        assert!(!settings.manages_models());
    }

    #[test]
    fn outro_usa_o_formato_escolhido_e_nunca_o_do_ollama() {
        let settings = AiDeciderSettings {
            provider: DeciderProvider::Custom,
            custom_protocol: DecisionProtocol::Ollama,
            ..AiDeciderSettings::default()
        }
        .normalized();
        assert_eq!(settings.protocol(), DecisionProtocol::Systemone);
        let openai = AiDeciderSettings {
            custom_protocol: DecisionProtocol::Openai,
            ..settings
        };
        assert_eq!(openai.protocol(), DecisionProtocol::Openai);
    }

    #[test]
    fn configuracao_antiga_e_ollama() {
        let settings: AiDeciderSettings = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert_eq!(settings.provider, DeciderProvider::Ollama);
        assert!(settings.manages_models());
    }

    #[test]
    fn provedor_ou_formato_desconhecido_vira_o_padrao_sem_perder_o_resto() {
        let settings: AiDeciderSettings = serde_json::from_str(
            r#"{"enabled":true,"provider":"desconhecido","customProtocol":"desconhecido","model":"laya:322m-multilingual-bf16","minConfidence":70}"#,
        )
        .unwrap();
        assert_eq!(settings.provider, DeciderProvider::default());
        assert_eq!(settings.custom_protocol, DecisionProtocol::default());
        assert!(settings.enabled);
        assert_eq!(settings.min_confidence, 70);
        assert_eq!(settings.model, "laya:322m-multilingual-bf16");

        let gravada = serde_json::to_value(&settings).unwrap();
        assert_eq!(gravada["provider"], "ollama");
        assert_eq!(gravada["customProtocol"], "systemone");
    }

    #[test]
    fn ollama_em_outra_maquina_mantem_url_e_modelo() {
        let outra_maquina = AiDeciderSettings {
            base_url: "http://10.0.0.5:11434".into(),
            model: "laya:322m-multilingual-bf16".into(),
            ..AiDeciderSettings::default()
        }
        .normalized();
        assert_eq!(outra_maquina.base_url, "http://10.0.0.5:11434");
        assert_eq!(outra_maquina.model, "laya:322m-multilingual-bf16");
    }

    #[test]
    fn so_a_chave_da_tela_vale() {
        let tela = AiDeciderSettings {
            api_key: Some(" da-tela ".into()),
            ..AiDeciderSettings::default()
        };
        assert_eq!(tela.effective_api_key().as_deref(), Some("da-tela"));
        assert_eq!(AiDeciderSettings::default().effective_api_key(), None);
    }

    #[test]
    fn configuracao_antiga_continua_roteando_o_chat() {
        let settings: AiDeciderSettings = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(settings.allows(DeciderFeature::ChatTools));
        assert!(settings.allows(DeciderFeature::DeviceIdentity));
        assert!(!settings.allows(DeciderFeature::LogEvents));
        assert!(!settings.allows(DeciderFeature::IncidentTriage));
    }

    #[test]
    fn chave_mestra_desligada_bloqueia_toda_frente() {
        let mut settings = AiDeciderSettings::default();
        settings.features.log_events = true;
        settings.features.incident_triage = DeciderTriageMode::Shadow;
        assert!(!settings.allows(DeciderFeature::ChatTools));
        assert!(!settings.allows(DeciderFeature::LogEvents));
        settings.enabled = true;
        assert!(settings.allows(DeciderFeature::LogEvents));
        assert!(settings.allows(DeciderFeature::IncidentTriage));
    }

    #[test]
    fn configuracao_antiga_sem_decider_recebe_os_padroes() {
        let settings: AiDeciderSettings = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(settings.enabled);
        assert_eq!(settings.min_confidence, DEFAULT_DECIDER_MIN_CONFIDENCE);
        assert!((settings.threshold() - 0.6).abs() < f64::EPSILON);
    }
}
