//! A forma única com que um palpite do Laya chega à tela: o valor sugerido,
//! quão seguro ele está e quem respondeu. A tela mostra e o usuário decide —
//! nada aplica uma sugestão sozinho.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::schema::Answer;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LayaSuggestion {
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

impl LayaSuggestion {
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

    /// O rótulo escolhido numa resposta de escolha; `None` se faltou a
    /// resposta, veio de outro tipo ou ficou abaixo da confiança mínima.
    #[must_use]
    pub fn from_choice(answer: Option<&Answer>, model: &str, min_confidence: u8) -> Option<Self> {
        let (label, confidence) = answer?.choice()?;
        Self::from_probability(label, confidence, model, min_confidence)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn choice(label: &str, confidence: f64) -> Answer {
        Answer::Choice {
            choice: label.into(),
            confidence,
            probabilities: BTreeMap::new(),
        }
    }

    #[test]
    fn sugere_so_acima_da_confianca_minima() {
        let hit = LayaSuggestion::from_choice(Some(&choice("camera", 0.874)), "laya:en", 60)
            .expect("acima do limiar");
        assert_eq!(hit.value, "camera");
        assert!((hit.confidence - 87.4).abs() < f64::EPSILON);
        assert_eq!(hit.model, "laya:en");

        assert!(LayaSuggestion::from_choice(Some(&choice("camera", 0.59)), "m", 60).is_none());
        assert!(LayaSuggestion::from_choice(None, "m", 60).is_none());
        assert!(LayaSuggestion::from_choice(Some(&Answer::Noul { noul: 0.9 }), "m", 60).is_none());
    }
}
