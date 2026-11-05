//! Qual interface é o uplink e quais valem monitorar.
//!
//! Hoje a escolha é toda manual: o operador lê nome, alias e velocidade de
//! cada porta. O Decisor lê o mesmo texto — e acerta o que um nome como
//! `ether1` não diz sozinho quando o alias é `WAN-Vivo`.
//!
//! O texto de cada interface vai nos critérios das perguntas, não no estado:
//! o estado é cortado em 3.000 caracteres e um switch de 48 portas não caberia.

use std::collections::BTreeMap;

pub use netmonitor_core::classifier::interfaces::{
    rank_candidates, InterfaceAdviceOutput, InterfaceCandidate, MonitorSuggestion,
};

use crate::decider::{
    decision::Decision,
    schema::{Answer, Question},
    suggestion::{DeciderSuggestion, FromChoice},
};

pub const UPLINK: &str = "uplink";
/// Perguntas sim/não por chamada: o resto do limite de 256 fica livre, e a
/// latência cresce com o número de perguntas.
const MAX_MONITOR_QUESTIONS: usize = 32;

const MONITOR_INSTRUCTION: &str = "This interface carries important traffic worth monitoring: \
an internet/WAN uplink, a trunk to another switch or router, or the link of a server or access point.";

pub struct InterfaceAdvice<'a> {
    pub ranked: Vec<&'a InterfaceCandidate>,
    pub min_confidence: u8,
}

fn monitor_key(if_index: i32) -> String {
    format!("mon_if{if_index}")
}

impl Decision for InterfaceAdvice<'_> {
    type Output = InterfaceAdviceOutput;

    fn questions(&self) -> BTreeMap<String, Question> {
        let mut questions = BTreeMap::new();
        if self.ranked.len() >= 2 {
            let criteria = self
                .ranked
                .iter()
                .map(|interface| (interface.key(), interface.describe()))
                .collect();
            questions.insert(
                UPLINK.to_string(),
                Question::Choice {
                    instructions: "Which interface is the uplink that carries this device's traffic to the rest of the network or the internet?"
                        .to_string(),
                    criteria,
                },
            );
        }
        for interface in self.ranked.iter().take(MAX_MONITOR_QUESTIONS) {
            questions.insert(
                monitor_key(interface.if_index),
                Question::Noul {
                    instructions: format!(
                        "{MONITOR_INSTRUCTION} Interface: {}.",
                        interface.describe()
                    ),
                },
            );
        }
        questions
    }

    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> InterfaceAdviceOutput {
        let min = self.min_confidence;
        let uplink = DeciderSuggestion::from_choice(answers.get(UPLINK), "", min).and_then(
            |mut suggestion| {
                let if_index = suggestion.value.strip_prefix("if")?.to_string();
                suggestion.value = if_index;
                Some(suggestion)
            },
        );
        let monitor = self
            .ranked
            .iter()
            .take(MAX_MONITOR_QUESTIONS)
            .filter_map(|interface| {
                let probability = answers.get(&monitor_key(interface.if_index))?.yes()?;
                DeciderSuggestion::from_probability("", probability, "", min).map(|suggestion| {
                    MonitorSuggestion {
                        if_index: interface.if_index,
                        confidence: suggestion.confidence,
                    }
                })
            })
            .collect();
        InterfaceAdviceOutput { uplink, monitor }
    }
}

#[cfg(test)]
mod tests {
    use netmonitor_core::classifier::interfaces::MAX_CHOICE_OPTIONS;

    use super::*;

    fn iface(if_index: i32, name: &str, traffic: u64, up: bool) -> InterfaceCandidate {
        InterfaceCandidate {
            if_index,
            name: name.into(),
            if_type: Some(6),
            speed: Some(1_000_000_000),
            oper_up: Some(up),
            in_octets: Some(traffic),
            out_octets: Some(0),
            ..InterfaceCandidate::default()
        }
    }

    #[test]
    fn respeita_os_limites_do_ollaya() {
        let list: Vec<InterfaceCandidate> = (1..=400)
            .map(|n| iface(n, &format!("p{n}"), 0, true))
            .collect();
        let advice = InterfaceAdvice {
            ranked: rank_candidates(&list),
            min_confidence: 60,
        };
        assert_eq!(advice.ranked.len(), MAX_CHOICE_OPTIONS);
        let questions = advice.questions();
        assert!(questions.len() <= 256);
        let Question::Choice { criteria, .. } = &questions[UPLINK] else {
            panic!("uplink é escolha");
        };
        assert!(criteria.len() <= 255);
    }

    #[test]
    fn le_uplink_e_interfaces_acima_do_limiar() {
        let list = [iface(7, "ether7", 5, true), iface(9, "ether9", 1, true)];
        let advice = InterfaceAdvice {
            ranked: rank_candidates(&list),
            min_confidence: 60,
        };
        let answers = BTreeMap::from([
            (
                UPLINK.to_string(),
                Answer::Choice {
                    choice: "if7".into(),
                    confidence: 0.88,
                    probabilities: BTreeMap::new(),
                },
            ),
            (monitor_key(7), Answer::Noul { noul: 0.9 }),
            (monitor_key(9), Answer::Noul { noul: 0.2 }),
        ]);
        let output = advice.interpret(&answers);
        assert_eq!(output.uplink.expect("uplink").value, "7");
        assert_eq!(
            output.monitor,
            [MonitorSuggestion {
                if_index: 7,
                confidence: 90.0
            }]
        );
    }
}
