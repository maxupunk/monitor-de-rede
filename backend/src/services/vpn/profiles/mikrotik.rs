//! Gerador de script RouterOS v7+ (MikroTik) — §8.10.5.
//!
//! O usuário cola o bloco no terminal do Winbox; MikroTik não lê QR Code.

use super::contract::{
    artifact_header, artifact_summary, ArtifactDelivery, GeneratedArtifact, PeerConfigContext,
    VpnProfileGenerator, PERSISTENT_KEEPALIVE_SECONDS,
};

pub const INTERFACE_NAME: &str = "wg-netmonitor";
/// Porta local padrão do WireGuard no RouterOS.
pub const LOCAL_LISTEN_PORT: i32 = 13_231;

/// Comentário que marca tudo que o NetMonitor cria.
///
/// É o que torna o script repetível: `remove [find comment="..."]` não falha
/// quando não há nada para remover, enquanto `find interface=<nome>` explodiria
/// com "input does not match any value of interface" caso a interface não
/// exista — exatamente o erro em cascata de uma primeira execução malsucedida.
pub const TAG: &str = "NetMonitor";

pub struct MikrotikProfileGenerator;

impl MikrotikProfileGenerator {
    /// Limpeza do que uma execução anterior tenha deixado para trás. Sem isso,
    /// uma segunda tentativa esbarra em "already have interface with such name".
    fn cleanup_section(context: &PeerConfigContext) -> Vec<String> {
        vec![
            "# Limpa uma instalacao anterior (nao falha se nao houver nada)".to_string(),
            format!("/interface/wireguard/peers/remove [find comment=\"{TAG}\"]"),
            format!("/ip/address/remove [find comment=\"{TAG}\"]"),
            // Busca também pelo endereço em si: entradas criadas por versões
            // antigas do script não têm o comentário e sobreviveriam à limpeza
            // acima, deixando um IP duplicado na VPN.
            format!(
                "/ip/address/remove [find address=\"{}/{}\"]",
                context.peer_ip_address,
                context.prefix_length()
            ),
            format!("/interface/wireguard/remove [find name=\"{INTERFACE_NAME}\"]"),
        ]
    }

    fn snmp_section(context: &PeerConfigContext) -> Vec<String> {
        if !context.snmp_enabled {
            return Vec::new();
        }
        vec![
            String::new(),
            "# SNMP (community cadastrada no NetMonitor)".to_string(),
            format!(
                "/snmp/community/set [find default=yes] addresses={} name=\"{}\"",
                context.vpn_cidr,
                context.community()
            ),
            "/snmp/set enabled=yes contact=\"NetMonitor\"".to_string(),
        ]
    }
}

impl VpnProfileGenerator for MikrotikProfileGenerator {
    fn profile(&self) -> &'static str {
        "mikrotik"
    }
    fn label(&self) -> &'static str {
        "MikroTik RouterOS v7+"
    }
    fn icon(&self) -> &'static str {
        "mdi-router-network"
    }
    fn supports_qr_code(&self) -> bool {
        false
    }

    fn firewall_hints(&self, _context: &PeerConfigContext) -> String {
        [
            "# Libera o monitoramento do NetMonitor na interface WireGuard".to_string(),
            format!("/ip/firewall/filter/remove [find comment=\"{TAG} ICMP\"]"),
            format!("/ip/firewall/filter/remove [find comment=\"{TAG} SNMP\"]"),
            format!(
                "/ip/firewall/filter/add chain=input in-interface={INTERFACE_NAME} protocol=icmp \\"
            ),
            format!("    action=accept comment=\"{TAG} ICMP\""),
            format!(
                "/ip/firewall/filter/add chain=input in-interface={INTERFACE_NAME} protocol=udp \\"
            ),
            format!("    dst-port=161 action=accept comment=\"{TAG} SNMP\""),
            "# Sobe as duas regras para o topo da chain (move funciona ate com a chain vazia)"
                .to_string(),
            format!("/ip/firewall/filter/move [find comment=\"{TAG} SNMP\"] destination=0"),
            format!("/ip/firewall/filter/move [find comment=\"{TAG} ICMP\"] destination=0"),
        ]
        .join("\n")
    }

    fn generate(&self, context: &PeerConfigContext) -> GeneratedArtifact {
        let mut lines = artifact_header(context);
        lines.push(String::new());
        lines.extend(Self::cleanup_section(context));
        lines.extend([
            String::new(),
            "# Interface WireGuard e IP fixo dentro da VPN".to_string(),
            format!(
                "/interface/wireguard/add name={INTERFACE_NAME} listen-port={LOCAL_LISTEN_PORT} \\"
            ),
            format!(
                "    private-key=\"{}\" comment=\"{TAG}\"",
                context.client_private_key
            ),
            format!(
                "/ip/address/add address={}/{} interface={INTERFACE_NAME} comment=\"{TAG}\"",
                context.peer_ip_address,
                context.prefix_length()
            ),
            String::new(),
            format!("/interface/wireguard/peers/add interface={INTERFACE_NAME} \\"),
            format!("    public-key=\"{}\" \\", context.server_public_key),
        ]);
        if let Some(preshared_key) = context.preshared_key.as_deref() {
            lines.push(format!("    preshared-key=\"{preshared_key}\" \\"));
        }
        lines.extend([
            format!(
                "    endpoint-address={} endpoint-port={} \\",
                context.endpoint_host, context.endpoint_port
            ),
            format!("    allowed-address={} \\", context.vpn_cidr),
            format!("    persistent-keepalive={PERSISTENT_KEEPALIVE_SECONDS}s comment=\"{TAG}\""),
            String::new(),
            self.firewall_hints(context),
        ]);
        lines.extend(Self::snmp_section(context));
        lines.extend([
            String::new(),
            "# Conferencia: \"last-handshake\" deve aparecer em poucos segundos".to_string(),
            format!("/interface/wireguard/peers/print where interface={INTERFACE_NAME}"),
        ]);

        GeneratedArtifact {
            profile: self.profile().to_string(),
            label: self.label().to_string(),
            delivery: ArtifactDelivery::Copy,
            file_name: format!("netmonitor-{}.rsc", context.peer_name),
            language: "routeros".to_string(),
            content: format!("{}\n", lines.join("\n")),
            instructions: vec![
                "Abra o Winbox e clique em \"New Terminal\".".to_string(),
                "Cole o script completo e pressione Enter.".to_string(),
                "O túnel sobe em poucos segundos e o dispositivo aparece como conectado no NetMonitor.".to_string(),
            ],
            supports_qr_code: false,
            summary: artifact_summary(context),
            variants: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::contract::tests::contexto;
    use super::*;

    #[test]
    fn o_script_completo_bate_com_o_esperado() {
        let artifact = MikrotikProfileGenerator.generate(&contexto());
        insta::assert_snapshot!(artifact.content);
    }

    #[test]
    fn a_limpeza_vem_antes_de_qualquer_criacao() {
        let content = MikrotikProfileGenerator.generate(&contexto()).content;
        let remove = content
            .find("/interface/wireguard/remove")
            .expect("limpeza");
        let add = content.find("/interface/wireguard/add").expect("criação");
        assert!(remove < add, "a limpeza precisa preceder a criação");
    }

    #[test]
    fn sem_preshared_key_a_linha_some() {
        let mut context = contexto();
        context.preshared_key = None;
        let content = MikrotikProfileGenerator.generate(&context).content;
        assert!(!content.contains("preshared-key"));
    }

    #[test]
    fn snmp_so_entra_quando_habilitado() {
        let mut context = contexto();
        assert!(!MikrotikProfileGenerator
            .generate(&context)
            .content
            .contains("/snmp/set"));
        context.snmp_enabled = true;
        let content = MikrotikProfileGenerator.generate(&context).content;
        assert!(content.contains("/snmp/set enabled=yes"));
        assert!(content.contains("name=\"public\""));
    }

    #[test]
    fn so_o_conteudo_e_ascii_o_nome_do_arquivo_preserva_o_titulo() {
        // O console do RouterOS é ASCII, mas o nome do arquivo é só rótulo de
        // download — mantê-lo igual ao do backend anterior evita divergência.
        let mut context = contexto();
        context.peer_name = "Roteador São João".into();
        let artifact = MikrotikProfileGenerator.generate(&context);
        assert_eq!(artifact.file_name, "netmonitor-Roteador São João.rsc");
        assert!(artifact.content.contains("Roteador Sao Joao"));
    }
}
