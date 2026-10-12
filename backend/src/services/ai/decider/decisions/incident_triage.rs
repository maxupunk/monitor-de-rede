//! Este alerta merece um resumo do LLM?
//!
//! O resumo automático custa uma investigação inteira do LLM, e o teto por
//! hora é por ordem de chegada: numa cascata, os filhos consomem o teto antes
//! da causa raiz. O Laya lê o alerta com o contexto barato (ancestral em
//! alerta, recaídas, outros alertas no mesmo minuto) e diz se ele é um
//! incidente novo, um sintoma de outro ou uma oscilação passageira.

use std::collections::BTreeMap;

use crate::services::ai::laya::{
    decision::Decision,
    schema::{Answer, Question},
};

const ACTIONABLE: &str = "actionable";
const SYMPTOM: &str = "symptom";
const TRANSIENT: &str = "transient";

/// Probabilidades (0–1) de cada leitura do alerta.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TriageVerdict {
    pub actionable: f64,
    pub symptom: f64,
    pub transient: f64,
}

pub struct IncidentTriage;

impl Decision for IncidentTriage {
    type Output = TriageVerdict;

    fn questions(&self) -> BTreeMap<String, Question> {
        [
            (
                ACTIONABLE,
                "This alert is a new, actionable incident that an operator should investigate now.",
            ),
            (
                SYMPTOM,
                "This alert is a downstream symptom of another failure that is already alerting \
(for example, a parent device or uplink is down).",
            ),
            (
                TRANSIENT,
                "This alert is a transient flap that will most likely recover by itself.",
            ),
        ]
        .into_iter()
        .map(|(id, instructions)| {
            (
                id.to_string(),
                Question::Noul {
                    instructions: instructions.to_string(),
                },
            )
        })
        .collect()
    }

    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> TriageVerdict {
        let yes = |id: &str| answers.get(id).and_then(Answer::yes).unwrap_or(0.0);
        TriageVerdict {
            actionable: yes(ACTIONABLE),
            symptom: yes(SYMPTOM),
            transient: yes(TRANSIENT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tres_perguntas_e_resposta_ausente_vale_zero() {
        assert_eq!(IncidentTriage.questions().len(), 3);
        let answers = BTreeMap::from([(ACTIONABLE.to_string(), Answer::Noul { noul: 0.8 })]);
        assert_eq!(
            IncidentTriage.interpret(&answers),
            TriageVerdict {
                actionable: 0.8,
                symptom: 0.0,
                transient: 0.0
            }
        );
    }
}
