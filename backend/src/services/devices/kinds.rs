//! Os tipos de dispositivo que o cadastro aceita, num lugar só.
//!
//! O formulário grava estes ids e a descoberta classifica com eles
//! (`discovery::fingerprints::kind`); só `web_device` e `unknown` são dela, de
//! dúvida. A tela espelha a lista em `frontend/src/utils/deviceTypes.ts`. A
//! descrição em inglês é o que o Laya lê para decidir.

pub struct DeviceKind {
    pub id: &'static str,
    pub label: &'static str,
    /// Critério da pergunta de escolha do Laya — em inglês, a língua do modelo.
    pub laya_hint: &'static str,
}

pub const DEVICE_KINDS: &[DeviceKind] = &[
    DeviceKind {
        id: "router",
        label: "Roteador",
        laya_hint: "Router or gateway that routes between networks: MikroTik RouterOS, OpenWrt (sysDescr like 'Linux <host> <version> #0 SMP ...', Net-SNMP agent, dropbear SSH, LuCI web, DNS on port 53), Banana Pi BPI-R, GL.iNet, edge router, ISP CPE/ONT.",
    },
    DeviceKind {
        id: "switch",
        label: "Switch",
        laya_hint: "Network switch that connects devices on the same LAN (managed or unmanaged).",
    },
    DeviceKind {
        id: "firewall",
        label: "Firewall",
        laya_hint: "Dedicated firewall or security appliance (pfSense, OPNsense, FortiGate, Sophos).",
    },
    DeviceKind {
        id: "ap",
        label: "Access point",
        laya_hint: "Wireless access point or Wi-Fi controller (UniFi, Aruba, Ruckus, TP-Link Omada).",
    },
    DeviceKind {
        id: "server",
        label: "Servidor",
        laya_hint: "General-purpose server or virtualization host: Windows Server, Linux distribution (Debian, Ubuntu, RHEL), VMware ESXi, Proxmox, databases, mail, LDAP.",
    },
    DeviceKind {
        id: "printer",
        label: "Impressora",
        laya_hint: "Network printer or multifunction printer.",
    },
    DeviceKind {
        id: "camera",
        label: "Câmera",
        laya_hint: "IP camera or video recorder (NVR/DVR, Hikvision, Dahua, Intelbras, ONVIF, RTSP).",
    },
    DeviceKind {
        id: "nas",
        label: "Armazenamento (NAS)",
        laya_hint: "Network storage appliance — only with storage evidence: SMB/AFP/NFS shares, Synology DSM (ports 5000/5001), QNAP, TrueNAS, Unraid, WD My Cloud, names like DiskStation.",
    },
    DeviceKind {
        id: "workstation",
        label: "Computador",
        laya_hint: "Desktop or laptop computer used by a person (Windows PC, Mac, Linux desktop).",
    },
    DeviceKind {
        id: "mobile",
        label: "Celular / tablet",
        laya_hint: "Phone or tablet (iPhone, iPad, Android), often with a randomized private MAC.",
    },
    DeviceKind {
        id: "media",
        label: "Smart TV / mídia",
        laya_hint: "Smart TV, streaming stick or box, game console, smart speaker or AV receiver (Chromecast, Roku, Apple TV, Sonos, PlayStation).",
    },
    DeviceKind {
        id: "iot",
        label: "IoT / automação",
        laya_hint: "IoT or home/building automation device: smart plug, bulb, sensor, ESP8266/ESP32 module, Shelly, Tuya, Home Assistant hub, access control, industrial PLC.",
    },
    DeviceKind {
        id: "voip",
        label: "Telefone IP / VoIP",
        laya_hint: "VoIP phone, ATA or PBX (Yealink, Grandstream, Polycom, Asterisk).",
    },
    DeviceKind {
        id: "ups",
        label: "Nobreak / energia",
        laya_hint: "UPS, PDU, inverter or solar charge controller (APC, Eaton, SMS, MPPT).",
    },
    DeviceKind {
        id: "other",
        label: "Outro",
        laya_hint: "Anything that fits none of the other kinds.",
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
            assert!(!kind.laya_hint.is_empty() && !kind.label.is_empty());
        }
    }
}
