//! Qual interface é o uplink e quais valem monitorar.
//!
//! Hoje a escolha é toda manual: o operador lê nome, alias e velocidade de
//! cada porta. O Laya lê o mesmo texto — e acerta o que um nome como
//! `ether1` não diz sozinho quando o alias é `WAN-Vivo`.
//!
//! O texto de cada interface vai nos critérios das perguntas, não no estado:
//! o estado é cortado em 3.000 caracteres e um switch de 48 portas não caberia.

use std::{cmp::Reverse, collections::BTreeMap};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::services::{
    ai::laya::{
        decision::Decision,
        schema::{Answer, Question},
        suggestion::LayaSuggestion,
    },
    snmp::collectors::if_type_label,
};

pub const UPLINK: &str = "uplink";
/// Limite de opções de uma pergunta de escolha no Ollaya.
const MAX_CHOICE_OPTIONS: usize = 254;
/// Perguntas sim/não por chamada: o resto do limite de 256 fica livre, e a
/// latência cresce com o número de perguntas.
const MAX_MONITOR_QUESTIONS: usize = 32;
const IF_TYPE_LOOPBACK: u64 = 24;

const MONITOR_INSTRUCTION: &str = "This interface carries important traffic worth monitoring: \
an internet/WAN uplink, a trunk to another switch or router, or the link of a server or access point.";

/// Uma interface como a tela a conhece — do escaneamento SNMP ou do cadastro.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct InterfaceCandidate {
    pub if_index: i32,
    pub name: String,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub descr: Option<String>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub if_type: Option<u64>,
    /// bits/s.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub speed: Option<u64>,
    #[serde(default)]
    pub oper_up: Option<bool>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub in_octets: Option<u64>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub out_octets: Option<u64>,
}

impl InterfaceCandidate {
    fn key(&self) -> String {
        format!("if{}", self.if_index)
    }

    fn traffic(&self) -> u64 {
        self.in_octets
            .unwrap_or(0)
            .saturating_add(self.out_octets.unwrap_or(0))
    }

    fn is_loopback(&self) -> bool {
        self.if_type == Some(IF_TYPE_LOOPBACK) || self.name.eq_ignore_ascii_case("lo")
    }

    /// Uma linha em inglês com tudo o que ajuda a reconhecer a interface.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut parts = vec![format!("name '{}'", self.name)];
        let distinct = |text: &Option<String>| {
            text.as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty() && *text != self.name)
                .map(str::to_string)
        };
        if let Some(alias) = distinct(&self.alias) {
            parts.push(format!("alias '{alias}'"));
        }
        if let Some(descr) =
            distinct(&self.descr).filter(|descr| Some(descr) != self.alias.as_ref())
        {
            parts.push(format!("description '{descr}'"));
        }
        if let Some(if_type) = self.if_type {
            parts.push(format!("type {}", if_type_label(if_type)));
        }
        if let Some(speed) = self.speed.filter(|speed| *speed > 0) {
            parts.push(format!("speed {}", human_bps(speed)));
        }
        match self.oper_up {
            Some(true) => parts.push("status up".into()),
            Some(false) => parts.push("status down".into()),
            None => {}
        }
        if self.in_octets.is_some() || self.out_octets.is_some() {
            parts.push(format!(
                "traffic in {} / out {}",
                human_bytes(self.in_octets.unwrap_or(0)),
                human_bytes(self.out_octets.unwrap_or(0))
            ));
        }
        parts.join(", ")
    }
}

fn human_bps(bps: u64) -> String {
    scaled(bps, 1_000, &["bps", "Kbps", "Mbps", "Gbps", "Tbps"])
}

fn human_bytes(bytes: u64) -> String {
    scaled(bytes, 1_000, &["B", "KB", "MB", "GB", "TB", "PB"])
}

/// Texto para o modelo ler, não para a tela: inteiro na maior unidade.
fn scaled(value: u64, step: u64, units: &[&str]) -> String {
    let mut value = value;
    let mut unit = 0;
    while value >= step && unit + 1 < units.len() {
        value /= step;
        unit += 1;
    }
    format!("{value} {}", units[unit])
}

/// As interfaces que vale perguntar: sem loopback, as ativas e com mais
/// tráfego primeiro, até o limite de opções de uma escolha.
#[must_use]
pub fn rank_candidates(interfaces: &[InterfaceCandidate]) -> Vec<&InterfaceCandidate> {
    let mut ranked: Vec<&InterfaceCandidate> = interfaces
        .iter()
        .filter(|interface| !interface.is_loopback())
        .collect();
    ranked.sort_by_key(|interface| {
        (
            Reverse(interface.oper_up == Some(true)),
            Reverse(interface.traffic()),
            Reverse(interface.speed.unwrap_or(0)),
            interface.if_index,
        )
    });
    ranked.truncate(MAX_CHOICE_OPTIONS);
    ranked
}

pub struct InterfaceAdvice<'a> {
    pub ranked: Vec<&'a InterfaceCandidate>,
    pub min_confidence: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MonitorSuggestion {
    pub if_index: i32,
    /// 0–100.
    pub confidence: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InterfaceAdviceOutput {
    /// `value` é o ifIndex.
    pub uplink: Option<LayaSuggestion>,
    pub monitor: Vec<MonitorSuggestion>,
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
            questions.insert(UPLINK.to_string(), Question::Choice { criteria });
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
        let uplink =
            LayaSuggestion::from_choice(answers.get(UPLINK), "", min).and_then(|mut suggestion| {
                let if_index = suggestion.value.strip_prefix("if")?.to_string();
                suggestion.value = if_index;
                Some(suggestion)
            });
        let monitor = self
            .ranked
            .iter()
            .take(MAX_MONITOR_QUESTIONS)
            .filter_map(|interface| {
                let probability = answers.get(&monitor_key(interface.if_index))?.yes()?;
                LayaSuggestion::from_probability("", probability, "", min).map(|suggestion| {
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
    fn ranking_tira_loopback_e_poe_ativas_com_trafego_primeiro() {
        let mut lo = iface(1, "lo", 999_999, true);
        lo.if_type = Some(IF_TYPE_LOOPBACK);
        let list = [
            lo,
            iface(2, "ether2", 10, false),
            iface(3, "ether3", 50, true),
            iface(4, "ether4", 900, true),
        ];
        let ranked: Vec<i32> = rank_candidates(&list).iter().map(|i| i.if_index).collect();
        assert_eq!(ranked, [4, 3, 2]);
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
        let Question::Choice { criteria } = &questions[UPLINK] else {
            panic!("uplink é escolha");
        };
        assert!(criteria.len() <= 255);
    }

    #[test]
    fn descreve_o_que_ajuda_a_reconhecer() {
        let mut wan = iface(1, "ether1", 812_000_000_000, true);
        wan.alias = Some("WAN-Vivo".into());
        wan.descr = Some("ether1".into());
        assert_eq!(
            wan.describe(),
            "name 'ether1', alias 'WAN-Vivo', type ethernet, speed 1 Gbps, status up, traffic in 812 GB / out 0 B"
        );
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
