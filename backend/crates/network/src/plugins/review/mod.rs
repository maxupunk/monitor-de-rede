//! A avaliação de segurança de um plugin vindo de fora.
//!
//! Plugin importado **não entra direto**: fica em quarentena até o operador
//! ler o relatório e decidir. O relatório junta duas leituras:
//!
//! 1. [`static_scan`] — determinística, sempre disponível;
//! 2. [`ai_review`] — a IA configurada lê manifesto, script e uso e responde se
//!    o script faz o que o uso diz, e se faz algo que não diz. Sem IA
//!    configurada, o relatório diz isso, e a instalação pede confirmação
//!    reforçada.
//!
//! O risco final é o **maior** dos dois. `critical` bloqueia a instalação;
//! `high` exige que o operador marque que revisou; sem IA, também.

pub mod ai_review;
pub mod static_scan;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::package::PluginPackage;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum Severity {
    Info,
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Finding {
    pub severity: Severity,
    /// Identificador da regra (`flash-write`) ou `ai` para achado da IA.
    pub rule: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub excerpt: Option<String>,
}

/// O que a IA concluiu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AiReview {
    pub risk_level: Severity,
    pub summary: String,
    #[serde(default)]
    pub findings: Vec<Finding>,
    /// O script faz algo que o `usage` não descreve.
    #[serde(default)]
    pub mismatch_with_usage: bool,
    #[serde(default)]
    #[ts(optional)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ReviewReport {
    pub risk: Severity,
    pub findings: Vec<Finding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ai: Option<AiReview>,
    /// Por que a IA não revisou (desligada, sem resposta, resposta inválida).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ai_error: Option<String>,
    pub reviewed_at: String,
    /// Checksum do pacote revisado — a revisão só vale para este código.
    pub checksum: String,
}

impl ReviewReport {
    /// A instalação é bloqueada.
    #[must_use]
    pub fn blocks(&self) -> bool {
        self.risk == Severity::Critical
    }

    /// A instalação exige que o operador declare ter revisado.
    #[must_use]
    pub fn needs_acknowledgement(&self) -> bool {
        self.risk >= Severity::High || self.ai.is_none()
    }
}

/// Junta a análise estática e, se houver, a da IA.
#[must_use]
pub fn build_report(package: &PluginPackage, ai: Result<AiReview, String>) -> ReviewReport {
    let findings = static_scan::scan(package);
    let static_risk = findings
        .iter()
        .map(|finding| finding.severity)
        .max()
        .unwrap_or(Severity::Low)
        .max(Severity::Low);
    let (ai, ai_error) = match ai {
        Ok(review) => (Some(review), None),
        Err(error) => (None, Some(error)),
    };
    let risk = ai
        .as_ref()
        .map_or(static_risk, |review| static_risk.max(review.risk_level));
    ReviewReport {
        risk,
        findings,
        ai,
        ai_error,
        reviewed_at: chrono::Utc::now().to_rfc3339(),
        checksum: package.checksum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package(script: &str) -> PluginPackage {
        serde_json::from_value(json!({
            "manifest": { "slug": "x", "name": "x", "version": "1.0.0", "transports": ["ssh"],
                          "actions": [ { "id": "detect", "title": "Detectar", "effect": "write" } ] },
            "script": script,
            "usage": "Detectar"
        }))
        .unwrap()
    }

    #[test]
    fn risco_final_e_o_maior_dos_dois() {
        let ai = AiReview {
            risk_level: Severity::High,
            summary: "altera senha".into(),
            findings: vec![],
            mismatch_with_usage: true,
            model: None,
        };
        let report = build_report(&package("fn detect(device, params) { 1 }"), Ok(ai));
        assert_eq!(report.risk, Severity::High);
        assert!(report.needs_acknowledgement());
        assert!(!report.blocks());
    }

    #[test]
    fn sem_ia_exige_confirmacao_e_critico_bloqueia() {
        let report = build_report(
            &package("fn detect(device, params) { device.run(\"sysupgrade x\") }"),
            Err("IA desligada".into()),
        );
        assert_eq!(report.risk, Severity::Critical);
        assert!(report.blocks());
        assert!(report.needs_acknowledgement());
        assert_eq!(report.ai_error.as_deref(), Some("IA desligada"));
    }

    #[test]
    fn script_limpo_com_ia_tranquila_e_baixo() {
        let ai = AiReview {
            risk_level: Severity::Low,
            summary: "só lê".into(),
            findings: vec![],
            mismatch_with_usage: false,
            model: Some("modelo".into()),
        };
        let report = build_report(&package("fn detect(device, params) { 1 }"), Ok(ai));
        assert_eq!(report.risk, Severity::Low);
        assert!(!report.needs_acknowledgement());
    }
}
