//! Que aparelho é este e que sistema ele roda — o palpite do Laya sobre o que
//! a descoberta viu (SNMP, portas, SSDP, mDNS, banner).
//!
//! A heurística (`discovery::device_identifier`, `devices::systems::detect`)
//! continua decidindo; isto só vira sugestão quando ela ficou em dúvida.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::services::{
    ai::laya::{
        decision::Decision,
        schema::{Answer, Question},
        suggestion::LayaSuggestion,
    },
    devices::{kinds::DEVICE_KINDS, systems},
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

/// O que se sabe do aparelho. Campos vazios não vão ao estado.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mdns_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub open_ports: Vec<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssdp_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys_descr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys_object_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sys_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_banner: Option<String>,
    /// `Server` e título da página web do aparelho.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_title: Option<String>,
    /// Fabricante e modelo que a descrição UPnP declara.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upnp_model: Option<String>,
    /// Serviços anunciados por mDNS (`_googlecast._tcp`, `_ipp._tcp`…).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mdns_services: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub netbios_name: Option<String>,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

impl DeviceFacts {
    /// A partir dos campos de um host da descoberta (`data` é o JSON dos
    /// scanners: `identity` do SNMP, `server` do SSDP).
    #[must_use]
    pub fn from_discovery(
        hostname: Option<&str>,
        mdns_name: Option<&str>,
        vendor: Option<&str>,
        open_ports: &[u16],
        data: &Value,
    ) -> Self {
        let identity = data.get("identity").cloned().unwrap_or(Value::Null);
        let http = data.get("http").cloned().unwrap_or(Value::Null);
        let ssdp = data.get("ssdp").cloned().unwrap_or(Value::Null);
        let upnp_model = [text(&ssdp, "manufacturer"), text(&ssdp, "modelName")]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        let clean = |value: Option<&str>| {
            value
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        Self {
            hostname: clean(hostname),
            mdns_name: clean(mdns_name),
            vendor: clean(vendor),
            open_ports: open_ports.to_vec(),
            ssdp_server: text(data, "server"),
            sys_descr: text(&identity, "sysDescr"),
            sys_object_id: text(&identity, "sysObjectId"),
            sys_name: text(&identity, "sysName"),
            hardware_vendor: text(&identity, "hardwareVendor"),
            hardware_model: text(&identity, "hardwareModel"),
            ssh_banner: None,
            web_server: text(&http, "server"),
            web_title: text(&http, "title"),
            upnp_model: (!upnp_model.is_empty()).then_some(upnp_model),
            mdns_services: data
                .pointer("/mdns/services")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            netbios_name: data
                .pointer("/netbios")
                .and_then(|value| text(value, "name")),
        }
    }

    /// Há algo para ler além do endereço? Sem nada, perguntar é chute.
    #[must_use]
    pub fn has_evidence(&self) -> bool {
        self.hostname.is_some()
            || self.mdns_name.is_some()
            || self.vendor.is_some()
            || !self.open_ports.is_empty()
            || self.ssdp_server.is_some()
            || self.sys_descr.is_some()
            || self.ssh_banner.is_some()
            || self.web_server.is_some()
            || self.web_title.is_some()
            || self.upnp_model.is_some()
            || !self.mdns_services.is_empty()
            || self.netbios_name.is_some()
    }

    /// O estado enviado ao Laya.
    #[must_use]
    pub fn state(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// O que perguntar: o tipo, o sistema, ou os dois.
pub struct DeviceIdentity {
    pub ask_type: bool,
    pub ask_system: bool,
    pub min_confidence: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct IdentitySuggestion {
    /// Um id de `devices::kinds::DEVICE_KINDS`.
    pub device_type: Option<LayaSuggestion>,
    /// Um id de `devices::systems::catalog()`.
    pub operating_system: Option<LayaSuggestion>,
}

impl IdentitySuggestion {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.device_type.is_none() && self.operating_system.is_none()
    }
}

impl Decision for DeviceIdentity {
    type Output = IdentitySuggestion;

    fn questions(&self) -> BTreeMap<String, Question> {
        let mut questions = BTreeMap::new();
        if self.ask_type {
            let criteria = DEVICE_KINDS
                .iter()
                .map(|kind| (kind.id.to_string(), kind.laya_hint.to_string()))
                .collect();
            questions.insert(DEVICE_TYPE.to_string(), Question::Choice { criteria });
        }
        if self.ask_system {
            let criteria = systems::catalog()
                .iter()
                .filter_map(|system| {
                    system_hint(system.id).map(|hint| (system.id.to_string(), hint.to_string()))
                })
                .collect();
            questions.insert(OPERATING_SYSTEM.to_string(), Question::Choice { criteria });
        }
        questions
    }

    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> IdentitySuggestion {
        let min = self.min_confidence;
        IdentitySuggestion {
            device_type: LayaSuggestion::from_choice(answers.get(DEVICE_TYPE), "", min),
            operating_system: LayaSuggestion::from_choice(answers.get(OPERATING_SYSTEM), "", min),
        }
    }
}

impl IdentitySuggestion {
    /// Carimba quem respondeu (o `interpret` não recebe o modelo).
    #[must_use]
    pub fn answered_by(mut self, model: &str) -> Self {
        for suggestion in [&mut self.device_type, &mut self.operating_system]
            .into_iter()
            .flatten()
        {
            suggestion.model = model.to_string();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn todo_sistema_do_catalogo_tem_dica() {
        for system in systems::catalog() {
            assert!(system_hint(system.id).is_some(), "sem dica: {}", system.id);
        }
    }

    #[test]
    fn fatos_da_descoberta_viram_estado_enxuto() {
        let data = json!({
            "server": "Linux/3.x UPnP/1.0 Hikvision",
            "identity": { "sysDescr": "  ", "sysName": "cam-portaria" }
        });
        let facts =
            DeviceFacts::from_discovery(Some(""), None, Some("Hikvision"), &[80, 554], &data);
        assert!(facts.has_evidence());
        let state: Value = serde_json::from_str(&facts.state()).unwrap();
        assert_eq!(
            state,
            json!({
                "vendor": "Hikvision",
                "openPorts": [80, 554],
                "ssdpServer": "Linux/3.x UPnP/1.0 Hikvision",
                "sysName": "cam-portaria"
            })
        );
        assert!(!DeviceFacts::default().has_evidence());
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
        let Question::Choice { criteria } = &questions[DEVICE_TYPE] else {
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
