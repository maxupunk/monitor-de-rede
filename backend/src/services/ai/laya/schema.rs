//! Contrato HTTP do Ollaya (`POST /api/decide`, compatível com TypeSafe).
//!
//! Só o que o NetMonitor usa. Um tipo de resposta desconhecido vira
//! [`Answer::Other`] em vez de derrubar a decisão inteira.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Uma pergunta tipada. A chave dela no mapa é o id da resposta.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Escolher um rótulo: `criteria` mapeia rótulo → descrição.
    Choice { criteria: BTreeMap<String, String> },
    /// Sim/não: responde a probabilidade de `instructions` valer.
    Noul { instructions: String },
}

#[derive(Debug, Serialize)]
pub struct DecideRequest<'a> {
    pub model: &'a str,
    pub state: &'a str,
    pub questions: &'a BTreeMap<String, Question>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Choice {
        choice: String,
        confidence: f64,
        #[serde(default)]
        probabilities: BTreeMap<String, f64>,
    },
    /// Probabilidade (0–1) de a afirmação valer.
    Noul { noul: f64 },
    #[serde(other)]
    Other,
}

impl Answer {
    /// Probabilidade de "sim", quando a resposta é sim/não.
    #[must_use]
    pub const fn yes(&self) -> Option<f64> {
        match self {
            Self::Noul { noul } => Some(*noul),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DecideResponse {
    /// O modelo que respondeu de fato (o roteador `laya` devolve `laya:en`…).
    pub model: String,
    #[serde(default)]
    pub answers: BTreeMap<String, Answer>,
}

/// Corpo de erro comum a todos os endpoints.
#[derive(Debug, Deserialize)]
pub struct ErrorBody {
    pub error: String,
    #[serde(default)]
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TagsResponse {
    #[serde(default)]
    pub models: Vec<TagItem>,
}

#[derive(Debug, Deserialize)]
pub struct TagItem {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializa_pergunta_sim_nao_no_formato_do_ollaya() {
        let questions = BTreeMap::from([(
            "history".to_string(),
            Question::Noul {
                instructions: "Needs history?".into(),
            },
        )]);
        let body = serde_json::to_value(DecideRequest {
            model: "laya",
            state: "oi",
            questions: &questions,
        })
        .unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "model": "laya",
                "state": "oi",
                "questions": { "history": { "type": "noul", "instructions": "Needs history?" } }
            })
        );
    }

    #[test]
    fn le_respostas_e_tolera_tipo_desconhecido() {
        let response: DecideResponse = serde_json::from_str(
            r#"{"model":"laya:multilingual","answers":{
                "a":{"type":"noul","noul":0.82},
                "b":{"type":"choice","choice":"x","confidence":0.7,"probabilities":{"x":0.7,"y":0.3}},
                "c":{"type":"score","score":2.1,"confidence":0.5}
            },"usage":{"input_tokens":10}}"#,
        )
        .unwrap();
        assert_eq!(response.model, "laya:multilingual");
        assert_eq!(response.answers["a"].yes(), Some(0.82));
        assert!(matches!(response.answers["b"], Answer::Choice { .. }));
        assert_eq!(response.answers["c"], Answer::Other);
    }
}
