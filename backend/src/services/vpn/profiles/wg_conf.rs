//! Gerador do `.conf` padrão do WireGuard (§8.10.5).
//!
//! Consumido pelos clientes oficiais (Linux, Windows, Android e iOS). O mesmo
//! formato serve ao QR Code.

use super::{
    contract::{
        artifact_summary, ArtifactDelivery, ArtifactVariant, GeneratedArtifact, PeerConfigContext,
        VpnProfileGenerator, PERSISTENT_KEEPALIVE_SECONDS, WG_TUNNEL_NAME,
    },
    variants::{linux_bash_variant, windows_winget_variant},
};

/// Constrói os scripts de terminal a partir do `.conf` já renderizado.
type VariantBuilder = fn(&PeerConfigContext, &str) -> ArtifactVariant;

pub struct WgConfProfileGenerator {
    profile: &'static str,
    label: &'static str,
    icon: &'static str,
    delivery: ArtifactDelivery,
    instructions: Vec<String>,
    supports_qr_code: bool,
    variant_builders: Vec<VariantBuilder>,
}

impl VpnProfileGenerator for WgConfProfileGenerator {
    fn profile(&self) -> &'static str {
        self.profile
    }
    fn label(&self) -> &'static str {
        self.label
    }
    fn icon(&self) -> &'static str {
        self.icon
    }
    fn supports_qr_code(&self) -> bool {
        self.supports_qr_code
    }

    fn firewall_hints(&self, _context: &PeerConfigContext) -> String {
        [
            "# Libera ICMP e SNMP vindos do NetMonitor pela interface WireGuard",
            "iptables -A INPUT -i wg0 -p icmp -j ACCEPT",
            "iptables -A INPUT -i wg0 -p udp --dport 161 -j ACCEPT",
        ]
        .join("\n")
    }

    fn generate(&self, context: &PeerConfigContext) -> GeneratedArtifact {
        let mut lines = vec![
            "[Interface]".to_string(),
            format!("PrivateKey = {}", context.client_private_key),
            format!(
                "Address = {}/{}",
                context.peer_ip_address,
                context.prefix_length()
            ),
            format!("MTU = {}", context.mtu),
        ];
        if let Some(dns) = context.dns_servers.as_deref().filter(|v| !v.is_empty()) {
            lines.push(format!("DNS = {dns}"));
        }
        lines.extend([
            String::new(),
            "[Peer]".to_string(),
            format!("PublicKey = {}", context.server_public_key),
        ]);
        if let Some(preshared_key) = context.preshared_key.as_deref() {
            lines.push(format!("PresharedKey = {preshared_key}"));
        }
        lines.extend([
            // Somente a faixa da VPN: com `0.0.0.0/0` a internet do cliente
            // cairia dentro do túnel.
            format!("AllowedIPs = {}", context.vpn_cidr),
            format!(
                "Endpoint = {}:{}",
                context.endpoint_host, context.endpoint_port
            ),
            format!("PersistentKeepalive = {PERSISTENT_KEEPALIVE_SECONDS}"),
        ]);

        let content = format!("{}\n", lines.join("\n"));
        let variants = self
            .variant_builders
            .iter()
            .map(|build| build(context, &content))
            .collect();

        GeneratedArtifact {
            profile: self.profile.to_string(),
            label: self.label.to_string(),
            delivery: self.delivery,
            file_name: format!("netmonitor-{}.conf", context.peer_name),
            language: "ini".to_string(),
            content,
            instructions: self.instructions.clone(),
            supports_qr_code: self.supports_qr_code,
            summary: artifact_summary(context),
            variants,
        }
    }
}

#[must_use]
pub fn linux_generator() -> WgConfProfileGenerator {
    WgConfProfileGenerator {
        profile: "linux",
        label: "Linux",
        icon: "mdi-linux",
        delivery: ArtifactDelivery::Download,
        instructions: vec![
            format!("Salve o arquivo como /etc/wireguard/{WG_TUNNEL_NAME}.conf (chmod 600)."),
            format!("Suba o túnel com: sudo wg-quick up {WG_TUNNEL_NAME}."),
            format!(
                "Habilite na inicialização com: sudo systemctl enable wg-quick@{WG_TUNNEL_NAME}."
            ),
        ],
        supports_qr_code: false,
        variant_builders: vec![linux_bash_variant],
    }
}

#[must_use]
pub fn windows_generator() -> WgConfProfileGenerator {
    WgConfProfileGenerator {
        profile: "windows",
        label: "Windows",
        icon: "mdi-microsoft-windows",
        delivery: ArtifactDelivery::Download,
        instructions: vec![
            "Instale o aplicativo oficial WireGuard para Windows.".to_string(),
            "Clique em \"Adicionar túnel\" e selecione o arquivo baixado.".to_string(),
            "Clique em \"Ativar\" para conectar.".to_string(),
        ],
        supports_qr_code: false,
        variant_builders: vec![windows_winget_variant],
    }
}

#[must_use]
pub fn mobile_generator() -> WgConfProfileGenerator {
    WgConfProfileGenerator {
        profile: "mobile",
        label: "Celular (Android / iOS)",
        icon: "mdi-cellphone",
        delivery: ArtifactDelivery::Qrcode,
        instructions: vec![
            "Instale o aplicativo WireGuard na loja do seu celular.".to_string(),
            "Toque em \"+\" e escolha \"Ler a partir do código QR\".".to_string(),
            "Aponte a câmera para o código exibido nesta tela.".to_string(),
        ],
        supports_qr_code: true,
        variant_builders: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::contract::tests::contexto;
    use super::*;

    #[test]
    fn o_conf_bate_com_o_esperado() {
        insta::assert_snapshot!(linux_generator().generate(&contexto()).content);
    }

    #[test]
    fn allowed_ips_nunca_captura_a_internet_do_cliente() {
        let content = linux_generator().generate(&contexto()).content;
        assert!(content.contains("AllowedIPs = 10.8.0.0/24"));
        assert!(!content.contains("0.0.0.0/0"));
    }

    #[test]
    fn dns_e_psk_so_aparecem_quando_configurados() {
        let mut context = contexto();
        context.preshared_key = None;
        let sem = linux_generator().generate(&context).content;
        assert!(!sem.contains("DNS ="));
        assert!(!sem.contains("PresharedKey"));

        context.dns_servers = Some("10.8.0.1".into());
        context.preshared_key = Some("PSK".into());
        let com = linux_generator().generate(&context).content;
        assert!(com.contains("DNS = 10.8.0.1"));
        assert!(com.contains("PresharedKey = PSK"));
    }

    #[test]
    fn so_o_perfil_movel_entrega_qr_code() {
        assert!(!linux_generator().supports_qr_code());
        assert!(!windows_generator().supports_qr_code());
        assert!(mobile_generator().supports_qr_code());
        assert_eq!(
            mobile_generator().generate(&contexto()).delivery,
            ArtifactDelivery::Qrcode
        );
    }

    #[test]
    fn cada_perfil_traz_a_variante_do_seu_sistema() {
        assert_eq!(
            linux_generator().generate(&contexto()).variants[0].id,
            "bash"
        );
        assert_eq!(
            windows_generator().generate(&contexto()).variants[0].id,
            "winget"
        );
        // O celular instala pelo QR Code: script de terminal não faz sentido.
        assert!(mobile_generator().generate(&contexto()).variants.is_empty());
    }

    #[test]
    fn os_tres_perfis_compartilham_o_mesmo_conf() {
        let context = contexto();
        let linux = linux_generator().generate(&context).content;
        assert_eq!(linux, windows_generator().generate(&context).content);
        assert_eq!(linux, mobile_generator().generate(&context).content);
    }
}
