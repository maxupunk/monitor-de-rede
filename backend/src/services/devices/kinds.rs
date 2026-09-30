//! Os tipos de dispositivo que o cadastro aceita, num lugar só.
//!
//! O formulário grava estes ids; a descoberta ainda fala `access_point`,
//! `web_device` e `unknown` e a tela traduz (`frontend/src/utils/deviceTypes.ts`,
//! espelho desta lista). A descrição em inglês é o que o Laya lê para decidir.

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
        laya_hint: "Router or gateway that routes between networks (MikroTik, OpenWrt, edge router, ISP CPE).",
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
        laya_hint: "Server, NAS or virtualization host (Linux, Windows Server, VMware, Proxmox, Synology).",
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
        id: "other",
        label: "Outro",
        laya_hint: "Anything else: IoT, UPS, solar charge controller, phone, workstation, smart TV.",
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
