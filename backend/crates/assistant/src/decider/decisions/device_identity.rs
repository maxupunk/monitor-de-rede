//! Que aparelho é este e que sistema ele roda — o palpite do Decisor sobre o que
//! a descoberta viu (SNMP, portas, SSDP, mDNS, banner).
//!
//! A heurística (`discovery::device_identifier`, `devices::systems::detect`)
//! continua decidindo; isto só vira sugestão quando ela ficou em dúvida.

use std::collections::BTreeMap;

use netmonitor_core::devices::{adapters::registry, kinds::DEVICE_KINDS};

pub use netmonitor_core::classifier::device_identity::{
    DeviceFacts, DeviceIdentity, IdentitySuggestion,
};

use crate::decider::{
    decision::Decision,
    schema::{Answer, Question},
    suggestion::{DeciderSuggestion, FromChoice},
};

pub const DEVICE_TYPE: &str = "device_type";
pub const OPERATING_SYSTEM: &str = "operating_system";

/// Descrição em inglês de cada sistema do catálogo, para a pergunta de escolha.
fn system_hint(id: &str) -> Option<&'static str> {
    Some(match id {
        "routeros" => "MikroTik RouterOS or SwOS (RouterBOARD, CCR, CRS, hAP).",
        "openwrt" => "OpenWrt or derivatives (LEDE, DD-WRT, GL.iNet, Turris), often with dropbear SSH.",
        "ubiquiti" => "Ubiquiti EdgeOS / UniFi / EdgeSwitch / Vyatta.",
        "linux" => "General-purpose Linux (Debian, Ubuntu, CentOS, a NAS or server distribution).",
        "windows" => "Microsoft Windows or Windows Server.",
        "mobile" => "Phone or tablet: Android, iOS, iPadOS.",
        "embedded" => "Embedded firmware of an appliance: UPS, PDU, solar charge controller (MPPT), IoT sensor.",
        "other" => "Something else or not enough evidence (printer firmware, camera firmware, unknown).",
        _ => return None,
    })
}

impl Decision for DeviceIdentity {
    type Output = IdentitySuggestion;

    fn questions(&self) -> BTreeMap<String, Question> {
        let mut questions = BTreeMap::new();
        if self.ask_type {
            let criteria = DEVICE_KINDS
                .iter()
                .map(|kind| (kind.id.to_string(), kind.decider_hint.to_string()))
                .collect();
            questions.insert(
                DEVICE_TYPE.to_string(),
                Question::Choice {
                    instructions: "Which kind of network device is described by this state?"
                        .to_string(),
                    criteria,
                },
            );
        }
        if self.ask_system {
            let criteria = registry::platforms()
                .iter()
                .filter_map(|system| {
                    system_hint(system.id).map(|hint| (system.id.to_string(), hint.to_string()))
                })
                .collect();
            questions.insert(
                OPERATING_SYSTEM.to_string(),
                Question::Choice {
                    instructions: "Which operating system does this device run?".to_string(),
                    criteria,
                },
            );
        }
        questions
    }

    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> IdentitySuggestion {
        let min = self.min_confidence;
        IdentitySuggestion {
            device_type: DeciderSuggestion::from_choice(answers.get(DEVICE_TYPE), "", min),
            operating_system: DeciderSuggestion::from_choice(
                answers.get(OPERATING_SYSTEM),
                "",
                min,
            ),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn todo_sistema_do_catalogo_tem_dica() {
        for system in registry::platforms() {
            assert!(system_hint(system.id).is_some(), "sem dica: {}", system.id);
        }
    }

    #[test]
    fn pergunta_so_o_que_falta_e_le_acima_do_limiar() {
        let only_type = DeviceIdentity {
            ask_type: true,
            ask_system: false,
            min_confidence: 60,
        };
        let questions = only_type.questions();
        assert_eq!(questions.len(), 1);
        let Question::Choice { criteria, .. } = &questions[DEVICE_TYPE] else {
            panic!("tipo é escolha");
        };
        assert!(criteria.contains_key("camera") && criteria.contains_key("ap"));

        let answers = BTreeMap::from([(
            DEVICE_TYPE.to_string(),
            Answer::Choice {
                choice: "camera".into(),
                confidence: 0.9,
                probabilities: BTreeMap::new(),
            },
        )]);
        let suggestion = only_type.interpret(&answers).answered_by("laya:en");
        let device_type = suggestion.device_type.expect("acima do limiar");
        assert_eq!(device_type.value, "camera");
        assert_eq!(device_type.model, "laya:en");
        assert!(suggestion.operating_system.is_none());
    }
}
