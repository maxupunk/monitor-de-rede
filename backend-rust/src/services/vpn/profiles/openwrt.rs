//! Gerador de comandos UCI para OpenWrt — §8.10.5.
//!
//! Também é colado no terminal (SSH), já que o LuCI não importa QR Code.
//!
//! **Gerenciador de pacotes.** O `opkg` valeu até a 23.05; a partir da 24.10 (e
//! no SNAPSHOT) o sistema migrou para o `apk`, e um firmware não tem os dois.
//! Por isso o script principal detecta qual existe, e cada variante fixa um
//! deles para quem já sabe a versão do equipamento.

use super::contract::{
    artifact_header, artifact_summary, ArtifactDelivery, ArtifactVariant, GeneratedArtifact,
    PeerConfigContext, VpnProfileGenerator, PERSISTENT_KEEPALIVE_SECONDS,
};

pub const INTERFACE_NAME: &str = "wg_nm";
pub const FIREWALL_ZONE: &str = "vpn_netmonitor";
const PACKAGES: &str = "wireguard-tools luci-proto-wireguard";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageManager {
    Auto,
    Opkg,
    Apk,
}

impl PackageManager {
    fn install(self, packages: &str) -> Vec<String> {
        match self {
            Self::Opkg => vec![format!("opkg update && opkg install {packages}")],
            Self::Apk => vec![format!("apk update && apk add {packages}")],
            Self::Auto => vec![
                "if command -v apk >/dev/null 2>&1; then   # OpenWrt 24.10+ / SNAPSHOT".to_string(),
                format!("  apk update && apk add {packages}"),
                "else                                      # OpenWrt 23.05 e anteriores"
                    .to_string(),
                format!("  opkg update && opkg install {packages}"),
                "fi".to_string(),
            ],
        }
    }

    fn install_snmp(self) -> Vec<String> {
        self.install("snmpd")
    }
}

pub struct OpenWrtProfileGenerator;

fn instructions() -> Vec<String> {
    vec![
        "Acesse o roteador por SSH (ex.: ssh root@192.168.1.1).".to_string(),
        "Cole o bloco completo de comandos e pressione Enter.".to_string(),
        "A rede reinicia ao final e o túnel sobe automaticamente.".to_string(),
    ]
}

impl OpenWrtProfileGenerator {
    fn snmp_section(context: &PeerConfigContext, manager: PackageManager) -> Vec<String> {
        if !context.snmp_enabled {
            return Vec::new();
        }
        let mut lines = vec![
            String::new(),
            "# SNMP (community cadastrada no NetMonitor)".to_string(),
        ];
        lines.extend(manager.install_snmp());
        lines.extend([
            format!("uci set snmpd.public.community='{}'", context.community()),
            format!("uci set snmpd.public.source='{}'", context.vpn_cidr),
            "uci commit snmpd && /etc/init.d/snmpd restart && /etc/init.d/snmpd enable".to_string(),
        ]);
        lines
    }

    /// Corpo do script — idêntico entre as variantes, exceto a instalação.
    fn build_script(&self, context: &PeerConfigContext, manager: PackageManager) -> String {
        let iface = INTERFACE_NAME;
        let mut lines = artifact_header(context);
        lines.extend(manager.install(PACKAGES));
        lines.extend([
            String::new(),
            format!("uci set network.{iface}=interface"),
            format!("uci set network.{iface}.proto='wireguard'"),
            format!(
                "uci set network.{iface}.private_key='{}'",
                context.client_private_key
            ),
            format!("uci set network.{iface}.mtu='{}'", context.mtu),
            format!(
                "uci add_list network.{iface}.addresses='{}/{}'",
                context.peer_ip_address,
                context.prefix_length()
            ),
            String::new(),
            format!("uci add network wireguard_{iface}"),
            format!(
                "uci set network.@wireguard_{iface}[-1].public_key='{}'",
                context.server_public_key
            ),
        ]);
        if let Some(preshared_key) = context.preshared_key.as_deref() {
            lines.push(format!(
                "uci set network.@wireguard_{iface}[-1].preshared_key='{preshared_key}'"
            ));
        }
        lines.extend([
            format!(
                "uci set network.@wireguard_{iface}[-1].endpoint_host='{}'",
                context.endpoint_host
            ),
            format!(
                "uci set network.@wireguard_{iface}[-1].endpoint_port='{}'",
                context.endpoint_port
            ),
            format!(
                "uci set network.@wireguard_{iface}[-1].persistent_keepalive='{PERSISTENT_KEEPALIVE_SECONDS}'"
            ),
            format!("uci set network.@wireguard_{iface}[-1].route_allowed_ips='1'"),
            format!(
                "uci add_list network.@wireguard_{iface}[-1].allowed_ips='{}'",
                context.vpn_cidr
            ),
            String::new(),
            self.firewall_hints(context),
        ]);
        lines.extend(Self::snmp_section(context, manager));
        lines.extend([
            String::new(),
            "uci commit network && uci commit firewall".to_string(),
            "/etc/init.d/network restart && /etc/init.d/firewall restart".to_string(),
        ]);
        format!("{}\n", lines.join("\n"))
    }

    fn build_variants(&self, context: &PeerConfigContext) -> Vec<ArtifactVariant> {
        vec![
            ArtifactVariant {
                id: "opkg".to_string(),
                label: "opkg".to_string(),
                hint: "OpenWrt 23.05, 22.03, 21.02 e anteriores".to_string(),
                icon: "mdi-package-variant-closed".to_string(),
                file_name: format!("netmonitor-{}-opkg.sh", context.peer_name),
                language: "shell".to_string(),
                content: self.build_script(context, PackageManager::Opkg),
                instructions: instructions(),
            },
            ArtifactVariant {
                id: "apk".to_string(),
                label: "apk".to_string(),
                hint: "OpenWrt 24.10+ e SNAPSHOT — o opkg foi substituído pelo apk".to_string(),
                icon: "mdi-package-variant".to_string(),
                file_name: format!("netmonitor-{}-apk.sh", context.peer_name),
                language: "shell".to_string(),
                content: self.build_script(context, PackageManager::Apk),
                instructions: instructions(),
            },
        ]
    }
}

impl VpnProfileGenerator for OpenWrtProfileGenerator {
    fn profile(&self) -> &'static str {
        "openwrt"
    }
    fn label(&self) -> &'static str {
        "OpenWrt"
    }
    fn icon(&self) -> &'static str {
        "mdi-router-wireless"
    }
    fn supports_qr_code(&self) -> bool {
        false
    }

    fn firewall_hints(&self, _context: &PeerConfigContext) -> String {
        [
            "# Zona de firewall permitindo o monitoramento do NetMonitor".to_string(),
            "uci add firewall zone".to_string(),
            format!("uci set firewall.@zone[-1].name='{FIREWALL_ZONE}'"),
            "uci set firewall.@zone[-1].input='ACCEPT'".to_string(),
            "uci set firewall.@zone[-1].output='ACCEPT'".to_string(),
            "uci set firewall.@zone[-1].forward='REJECT'".to_string(),
            format!("uci add_list firewall.@zone[-1].network='{INTERFACE_NAME}'"),
        ]
        .join("\n")
    }

    fn generate(&self, context: &PeerConfigContext) -> GeneratedArtifact {
        GeneratedArtifact {
            profile: self.profile().to_string(),
            label: self.label().to_string(),
            delivery: ArtifactDelivery::Copy,
            file_name: format!("netmonitor-{}.sh", context.peer_name),
            language: "shell".to_string(),
            content: self.build_script(context, PackageManager::Auto),
            instructions: instructions(),
            supports_qr_code: false,
            summary: artifact_summary(context),
            variants: self.build_variants(context),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::contract::tests::contexto;
    use super::*;

    #[test]
    fn o_script_principal_bate_com_o_esperado() {
        let artifact = OpenWrtProfileGenerator.generate(&contexto());
        insta::assert_snapshot!(artifact.content);
    }

    #[test]
    fn o_script_principal_detecta_o_gerenciador_de_pacotes() {
        let content = OpenWrtProfileGenerator.generate(&contexto()).content;
        assert!(content.contains("if command -v apk"));
        assert!(content.contains("opkg update && opkg install"));
    }

    #[test]
    fn cada_variante_fixa_um_gerenciador() {
        let artifact = OpenWrtProfileGenerator.generate(&contexto());
        assert_eq!(artifact.variants.len(), 2);

        let opkg = &artifact.variants[0];
        assert_eq!(opkg.id, "opkg");
        assert!(opkg.content.contains("opkg update && opkg install"));
        assert!(!opkg.content.contains("if command -v apk"));

        let apk = &artifact.variants[1];
        assert_eq!(apk.id, "apk");
        assert!(apk.content.contains("apk update && apk add"));
        assert!(!apk.content.contains("opkg"));
    }

    #[test]
    fn snmp_entra_no_gerenciador_certo_de_cada_variante() {
        let mut context = contexto();
        context.snmp_enabled = true;
        let artifact = OpenWrtProfileGenerator.generate(&context);
        assert!(artifact.variants[0].content.contains("opkg install snmpd"));
        assert!(artifact.variants[1].content.contains("apk add snmpd"));
        assert!(artifact
            .content
            .contains("uci set snmpd.public.source='10.8.0.0/24'"));
    }

    #[test]
    fn a_zona_de_firewall_referencia_a_interface_do_tunel() {
        let hints = OpenWrtProfileGenerator.firewall_hints(&contexto());
        assert!(hints.contains(&format!("network='{INTERFACE_NAME}'")));
        assert!(hints.contains(FIREWALL_ZONE));
    }
}
