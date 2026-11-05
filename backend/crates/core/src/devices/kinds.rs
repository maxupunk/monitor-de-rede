//! Os tipos de dispositivo que o cadastro aceita, num lugar só.
//!
//! O formulário grava estes ids e a descoberta classifica com eles
//! (`discovery::fingerprints::kind`); só `web_device` e `unknown` são dela, de
//! dúvida. A tela espelha a lista em `frontend/src/utils/deviceTypes.ts`. A
//! descrição em inglês é o que o Decisor lê para decidir.

pub struct DeviceKind {
    pub id: &'static str,
    pub label: &'static str,
    /// Critério da pergunta de escolha do Decisor — em inglês, a língua do modelo.
    pub decider_hint: &'static str,
}

pub const DEVICE_KINDS: &[DeviceKind] = &[
    DeviceKind {
        id: "router",
        label: "Roteador",
        decider_hint: "Router or gateway that routes between networks: MikroTik RouterOS, OpenWrt (sysDescr like 'Linux <host> <version> #0 SMP ...', Net-SNMP agent, dropbear SSH, LuCI web, DNS on port 53), Banana Pi BPI-R, GL.iNet, edge router, ISP CPE/ONT.",
    },
    DeviceKind {
        id: "switch",
        label: "Switch",
        decider_hint: "Network switch that connects devices on the same LAN (managed or unmanaged).",
    },
    DeviceKind {
        id: "firewall",
        label: "Firewall",
        decider_hint: "Dedicated firewall or security appliance (pfSense, OPNsense, FortiGate, Sophos).",
    },
    DeviceKind {
        id: "ap",
        label: "Access point",
        decider_hint: "Wireless access point or Wi-Fi controller (UniFi, Aruba, Ruckus, TP-Link Omada).",
    },
    DeviceKind {
        id: "server",
        label: "Servidor",
        decider_hint: "General-purpose server or virtualization host: Windows Server, Linux distribution (Debian, Ubuntu, RHEL), VMware ESXi, Proxmox, databases, mail, LDAP.",
    },
    DeviceKind {
        id: "printer",
        label: "Impressora",
        decider_hint: "Network printer or multifunction printer.",
    },
    DeviceKind {
        id: "camera",
        label: "Câmera",
        decider_hint: "IP camera or video recorder (NVR/DVR, Hikvision, Dahua, Intelbras, ONVIF, RTSP).",
    },
    DeviceKind {
        id: "nas",
        label: "Armazenamento (NAS)",
        decider_hint: "Network storage appliance — only with storage evidence: SMB/AFP/NFS shares, Synology DSM (ports 5000/5001), QNAP, TrueNAS, Unraid, WD My Cloud, names like DiskStation.",
    },
    DeviceKind {
        id: "workstation",
        label: "Computador",
        decider_hint: "Desktop or laptop computer used by a person (Windows PC, Mac, Linux desktop).",
    },
    DeviceKind {
        id: "mobile",
        label: "Celular / tablet",
        decider_hint: "Phone or tablet (iPhone, iPad, Android), often with a randomized private MAC.",
    },
    DeviceKind {
        id: "media",
        label: "Smart TV / mídia",
        decider_hint: "Smart TV, streaming stick or box, game console, smart speaker or AV receiver (Chromecast, Roku, Apple TV, Sonos, PlayStation).",
    },
    DeviceKind {
        id: "iot",
        label: "IoT / automação",
        decider_hint: "IoT or home/building automation device: smart plug, bulb, sensor, ESP8266/ESP32 module, Shelly, Tuya, Home Assistant hub, access control, industrial PLC.",
    },
    DeviceKind {
        id: "voip",
        label: "Telefone IP / VoIP",
        decider_hint: "VoIP phone, ATA or PBX (Yealink, Grandstream, Polycom, Asterisk).",
    },
    DeviceKind {
        id: "ups",
        label: "Nobreak / energia",
        decider_hint: "UPS, PDU, inverter or solar charge controller (APC, Eaton, SMS, MPPT).",
    },
    DeviceKind {
        id: "other",
        label: "Outro",
        decider_hint: "Anything that fits none of the other kinds.",
    },
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_sao_unicos_e_todos_tem_dica() {
        let mut ids = HashSet::new();
        for kind in DEVICE_KINDS {
            assert!(ids.insert(kind.id), "id repetido: {}", kind.id);
            assert!(!kind.decider_hint.is_empty() && !kind.label.is_empty());
        }
    }
}
