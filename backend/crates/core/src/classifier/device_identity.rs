//! Que aparelho é este e que sistema ele roda: o que a descoberta viu e o
//! palpite que volta. A heurística (`discovery::device_identifier`,
//! `devices::systems::detect`) continua decidindo; o palpite de um
//! [`super::Classifier`] só vira sugestão quando ela ficou em dúvida.

use serde::Serialize;
use serde_json::Value;

use super::suggestion::DeciderSuggestion;

/// Chave do palpite em `details` do resultado e em `data` do host ao vivo.
pub const DECIDER_KEY: &str = "decider";
/// A mesma chave em varreduras gravadas antes do renome para Decisor.
pub const LEGACY_DECIDER_KEY: &str = "laya";

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
    /// O nome de cada porta aberta ("SSH", "RTSP", "Impressão RAW") — o
    /// modelo entende o serviço melhor que o número.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<String>,
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
            services: data
                .get("services")
                .and_then(Value::as_object)
                .map(|labels| {
                    labels
                        .values()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
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

    /// O estado enviado ao Decisor.
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
    pub device_type: Option<DeciderSuggestion>,
    /// Um id de `devices::systems::catalog()`.
    pub operating_system: Option<DeciderSuggestion>,
}

impl IdentitySuggestion {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.device_type.is_none() && self.operating_system.is_none()
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
}
