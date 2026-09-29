//! Configuração do Laya: onde está o Ollaya e quão seguro ele precisa estar.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// O serviço `ollaya` do `docker-compose.yml`, na rede interna.
pub const DEFAULT_LAYA_BASE_URL: &str = "http://ollaya:11435";
/// O roteador: escolhe `laya:en` ou `laya:multilingual` pela língua da pergunta.
pub const DEFAULT_LAYA_MODEL: &str = "laya";
/// Probabilidade mínima (%) para um grupo de ferramentas entrar.
pub const DEFAULT_LAYA_MIN_CONFIDENCE: u8 = 60;
/// Tempo máximo de uma decisão. Estourou, o chat segue sem ela.
pub const DEFAULT_LAYA_TIMEOUT_MS: u32 = 1_500;
/// A mesma variável que o Ollaya lê: o compose entrega o mesmo valor aos dois
/// containers, e ninguém precisa copiá-la para a tela.
pub const API_KEY_ENV: &str = "OLLAYA_API_KEY";

/// A chave do Ollaya, lida no ponto de uso. Vazia = servidor sem autenticação.
#[must_use]
pub fn api_key() -> Option<String> {
    std::env::var(API_KEY_ENV)
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiLayaSettings {
    /// Decide, antes de cada pergunta, quais grupos de ferramentas vão à IA.
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    /// 1–99: quanto maior, menos grupos entram por palpite do Laya.
    pub min_confidence: u8,
    /// 200–10.000 ms.
    pub timeout_ms: u32,
}

impl Default for AiLayaSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: DEFAULT_LAYA_BASE_URL.to_string(),
            model: DEFAULT_LAYA_MODEL.to_string(),
            min_confidence: DEFAULT_LAYA_MIN_CONFIDENCE,
            timeout_ms: DEFAULT_LAYA_TIMEOUT_MS,
        }
    }
}

impl AiLayaSettings {
    /// Traz valores vazios ou fora da faixa de volta para ela.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        let base_url = self.base_url.trim().trim_end_matches('/');
        self.base_url = if base_url.is_empty() {
            DEFAULT_LAYA_BASE_URL.to_string()
        } else {
            base_url.to_string()
        };
        let model = self.model.trim();
        self.model = if model.is_empty() {
            DEFAULT_LAYA_MODEL.to_string()
        } else {
            model.to_string()
        };
        self.min_confidence = self.min_confidence.clamp(1, 99);
        self.timeout_ms = self.timeout_ms.clamp(200, 10_000);
        self
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
        let settings = AiLayaSettings {
            enabled: true,
            base_url: " http://ollaya:11435/ ".into(),
            model: String::new(),
            min_confidence: 0,
            timeout_ms: 50_000,
        }
        .normalized();
        assert_eq!(settings.base_url, "http://ollaya:11435");
        assert_eq!(settings.model, DEFAULT_LAYA_MODEL);
        assert_eq!(settings.min_confidence, 1);
        assert_eq!(settings.timeout_ms, 10_000);
    }

    #[test]
    fn configuracao_antiga_sem_laya_recebe_os_padroes() {
        let settings: AiLayaSettings = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(settings.enabled);
        assert_eq!(settings.min_confidence, DEFAULT_LAYA_MIN_CONFIDENCE);
        assert!((settings.threshold() - 0.6).abs() < f64::EPSILON);
    }
}
