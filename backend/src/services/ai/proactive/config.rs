//! Configuração da IA proativa: o que ela faz sem ser chamada.
//!
//! Tudo nasce desligado — cada resumo automático é uma chamada ao provedor,
//! e quem paga os tokens decide quando vale a pena.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Severidade mínima de um alerta para ganhar resumo automático.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiIncidentSeverity {
    #[default]
    Critical,
    Warning,
}

impl AiIncidentSeverity {
    /// O alerta tem a severidade mínima pedida?
    #[must_use]
    pub fn accepts(self, severity: &str) -> bool {
        match self {
            Self::Critical => severity == "critical",
            Self::Warning => matches!(severity, "critical" | "warning"),
        }
    }
}

/// Frequência do resumo da rede enviado pelos canais de notificação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AiDigestSchedule {
    #[default]
    Off,
    Daily,
    /// Toda segunda-feira.
    Weekly,
}

pub const DEFAULT_MAX_SUMMARIES_PER_HOUR: u32 = 6;
pub const DEFAULT_DIGEST_HOUR: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiProactiveSettings {
    /// Gera um resumo (causa provável e primeira ação) quando um alerta abre.
    pub incident_summaries: bool,
    pub incident_min_severity: AiIncidentSeverity,
    /// Teto de resumos automáticos por hora, para uma tempestade de alertas
    /// não virar uma tempestade de chamadas ao provedor.
    pub max_summaries_per_hour: u32,
    pub digest: AiDigestSchedule,
    /// Hora (0–23, horário do servidor) em que o resumo periódico sai.
    pub digest_hour: u8,
}

impl Default for AiProactiveSettings {
    fn default() -> Self {
        Self {
            incident_summaries: false,
            incident_min_severity: AiIncidentSeverity::default(),
            max_summaries_per_hour: DEFAULT_MAX_SUMMARIES_PER_HOUR,
            digest: AiDigestSchedule::default(),
            digest_hour: DEFAULT_DIGEST_HOUR,
        }
    }
}

impl AiProactiveSettings {
    /// Traz valores fora da faixa de volta para ela.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.max_summaries_per_hour = self.max_summaries_per_hour.clamp(1, 60);
        self.digest_hour = self.digest_hour.min(23);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tudo_nasce_desligado() {
        let config = AiProactiveSettings::default();
        assert!(!config.incident_summaries);
        assert_eq!(config.digest, AiDigestSchedule::Off);
    }

    #[test]
    fn severidade_minima() {
        assert!(AiIncidentSeverity::Critical.accepts("critical"));
        assert!(!AiIncidentSeverity::Critical.accepts("warning"));
        assert!(AiIncidentSeverity::Warning.accepts("warning"));
        assert!(!AiIncidentSeverity::Warning.accepts("info"));
    }

    #[test]
    fn valores_fora_da_faixa_sao_corrigidos() {
        let config = AiProactiveSettings {
            max_summaries_per_hour: 0,
            digest_hour: 40,
            ..AiProactiveSettings::default()
        }
        .normalized();
        assert_eq!(config.max_summaries_per_hour, 1);
        assert_eq!(config.digest_hour, 23);
    }

    #[test]
    fn campos_ausentes_assumem_o_padrao() {
        let config: AiProactiveSettings =
            serde_json::from_str(r#"{ "digest": "weekly" }"#).unwrap();
        assert_eq!(config.digest, AiDigestSchedule::Weekly);
        assert_eq!(config.digest_hour, DEFAULT_DIGEST_HOUR);
    }
}
