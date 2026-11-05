//! A forma única com que um palpite do Decisor chega à tela — o tipo mora no
//! núcleo (`netmonitor_core::classifier::suggestion`); aqui fica a leitura de
//! uma resposta de escolha do Decisor.

pub use netmonitor_core::classifier::suggestion::{percent, DeciderSuggestion};

use super::schema::Answer;

/// A leitura de uma resposta de escolha, que só o Decisor conhece.
pub trait FromChoice: Sized {
    /// O rótulo escolhido numa resposta de escolha; `None` se faltou a
    /// resposta, veio de outro tipo ou ficou abaixo da confiança mínima.
    fn from_choice(answer: Option<&Answer>, model: &str, min_confidence: u8) -> Option<Self>;
}

impl FromChoice for DeciderSuggestion {
    fn from_choice(answer: Option<&Answer>, model: &str, min_confidence: u8) -> Option<Self> {
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
        let hit = DeciderSuggestion::from_choice(Some(&choice("camera", 0.874)), "laya:en", 60)
            .expect("acima do limiar");
        assert_eq!(hit.value, "camera");
        assert!((hit.confidence - 87.4).abs() < f64::EPSILON);
        assert_eq!(hit.model, "laya:en");

        assert!(DeciderSuggestion::from_choice(Some(&choice("camera", 0.59)), "m", 60).is_none());
        assert!(DeciderSuggestion::from_choice(None, "m", 60).is_none());
        assert!(
            DeciderSuggestion::from_choice(Some(&Answer::Noul { noul: 0.9 }), "m", 60).is_none()
        );
    }
}
