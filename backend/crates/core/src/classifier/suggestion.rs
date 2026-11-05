//! A forma única com que um palpite do classificador chega à tela: o valor
//! sugerido, quão seguro ele está e quem respondeu. A tela mostra e o usuário
//! decide — nada aplica uma sugestão sozinho.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeciderSuggestion {
    pub value: String,
    /// 0–100, uma casa decimal.
    pub confidence: f64,
    /// O modelo que respondeu (`laya:multilingual`…).
    pub model: String,
}

/// Probabilidade (0–1) em percentual com uma casa.
#[must_use]
pub fn percent(probability: f64) -> f64 {
    (probability.clamp(0.0, 1.0) * 1000.0).round() / 10.0
}

impl DeciderSuggestion {
    /// Um valor com a probabilidade dele; `None` abaixo da confiança mínima.
    #[must_use]
    pub fn from_probability(
        value: impl Into<String>,
        probability: f64,
        model: &str,
        min_confidence: u8,
    ) -> Option<Self> {
        let confidence = percent(probability);
        (confidence >= f64::from(min_confidence)).then(|| Self {
            value: value.into(),
            confidence,
            model: model.to_string(),
        })
    }
}
