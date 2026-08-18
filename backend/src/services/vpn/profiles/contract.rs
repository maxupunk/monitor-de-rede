//! Contrato dos geradores de configuração por perfil de equipamento (§8.10.5).
//!
//! Adicionar suporte a um novo equipamento significa criar uma nova
//! implementação e registrá-la — nenhuma existente é alterada (OCP).

use serde::Serialize;
use ts_rs::TS;

pub use crate::services::vpn::secret_store::PRIVATE_KEY_UNAVAILABLE;

/// Keepalive fixo: sem ele o NAT do cliente expira e o servidor perde o caminho
/// de volta.
pub const PERSISTENT_KEEPALIVE_SECONDS: i32 = 25;

/// Nome da interface/túnel criado nos clientes que instalam por script.
///
/// Precisa valer nos dois mundos: no Linux vira `wg-quick@<nome>` e no Windows
/// vira o nome do serviço do túnel — ambos limitados a 15 caracteres do conjunto
/// `[A-Za-z0-9_=+.-]`. Fixo (e não derivado do nome do dispositivo) justamente
/// para nunca cair fora dessa janela.
pub const WG_TUNNEL_NAME: &str = "netmonitor";

/// Tudo que um gerador precisa saber para montar o artefato.
#[derive(Debug, Clone)]
pub struct PeerConfigContext {
    /// Nome do dispositivo, usado em comentários do script.
    pub peer_name: String,
    /// IP fixo do peer dentro da VPN (ex.: `10.8.0.11`).
    pub peer_ip_address: String,
    /// Faixa da VPN (ex.: `10.8.0.0/24`) — único valor aceito em `AllowedIPs`.
    pub vpn_cidr: String,
    /// IP do servidor NetMonitor dentro da VPN (ex.: `10.8.0.1`).
    pub server_vpn_address: String,
    /// Chave privada do cliente — existe apenas em memória, nunca é persistida.
    pub client_private_key: String,
    pub server_public_key: String,
    pub preshared_key: Option<String>,
    pub endpoint_host: String,
    pub endpoint_port: i32,
    pub mtu: i32,
    pub dns_servers: Option<String>,
    pub snmp_enabled: bool,
    pub snmp_community: Option<String>,
}

impl PeerConfigContext {
    /// Prefixo do CIDR como texto (`24`), usado em `Address = <ip>/<prefixo>`.
    #[must_use]
    pub fn prefix_length(&self) -> &str {
        self.vpn_cidr.split('/').nth(1).unwrap_or("24")
    }

    #[must_use]
    pub fn community(&self) -> &str {
        self.snmp_community
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("public")
    }

    #[must_use]
    pub fn sanitized_community(&self) -> String {
        let raw = self.community();
        let cleaned: String = raw
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect();
        if cleaned.is_empty() {
            "public".to_string()
        } else {
            cleaned
        }
    }

    /// `true` quando a chave privada já foi consumida e o artefato é inútil.
    #[must_use]
    pub fn private_key_consumed(&self) -> bool {
        self.client_private_key == PRIVATE_KEY_UNAVAILABLE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ArtifactDelivery {
    Copy,
    Download,
    Qrcode,
}

/// Par rótulo/valor exibido no resumo "os dados do túnel" antes do script.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ArtifactSummaryItem {
    pub label: String,
    pub value: String,
}

/// Alternativa de instalação entregue ao lado do artefato principal — o mesmo
/// túnel, porém escrito para outro gerenciador de pacotes. Cada variante é
/// autocontida: instala o cliente, grava o perfil e sobe o túnel.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ArtifactVariant {
    /// Identificador estável usado como chave de aba no frontend.
    pub id: String,
    pub label: String,
    /// Onde essa variante se aplica (ex.: "Debian, Ubuntu, Mint").
    pub hint: String,
    pub icon: String,
    pub file_name: String,
    pub language: String,
    pub content: String,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct GeneratedArtifact {
    pub profile: String,
    /// Rótulo amigável do equipamento.
    pub label: String,
    /// Como o usuário deve consumir o artefato.
    pub delivery: ArtifactDelivery,
    pub file_name: String,
    /// Dica de linguagem para realce de sintaxe no frontend.
    pub language: String,
    pub content: String,
    pub instructions: Vec<String>,
    /// Perfis móveis podem exibir QR Code.
    pub supports_qr_code: bool,
    /// Parâmetros do túnel em formato legível — **não** contém a chave privada.
    pub summary: Vec<ArtifactSummaryItem>,
    /// Scripts de terminal equivalentes, um por gerenciador de pacotes.
    pub variants: Vec<ArtifactVariant>,
}

pub trait VpnProfileGenerator: Send + Sync {
    fn profile(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Ícone (Material Design Icons) usado nos cards do wizard.
    fn icon(&self) -> &'static str;
    /// Apenas perfis móveis entregam a configuração por QR Code.
    fn supports_qr_code(&self) -> bool;
    /// Artefato principal (script ou arquivo de configuração).
    fn generate(&self, context: &PeerConfigContext) -> GeneratedArtifact;
    /// Regras que liberam ICMP/SNMP na interface da VPN — usadas no diagnóstico
    /// "túnel conectado, mas o dispositivo não responde".
    fn firewall_hints(&self, context: &PeerConfigContext) -> String;
}

/// Remove acentos e pontuação tipográfica.
///
/// O console do RouterOS e o do OpenWrt trabalham em ASCII: um `·`, um travessão
/// ou um nome de dispositivo acentuado chegam truncados no editor de linha e
/// podem levar junto o começo do comando seguinte.
#[must_use]
pub fn ascii_safe(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            '\u{2010}'..='\u{2015}' => '-',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            other if ('\u{20}'..='\u{7e}').contains(&other) => other,
            _ => ' ',
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Cabeçalho dos scripts colados em console de equipamento.
///
/// Quando a chave privada já foi consumida, o script inteiro é inútil — o
/// equipamento responderia apenas "invalid private key". Avisar aqui troca esse
/// erro críptico por uma instrução clara.
#[must_use]
pub fn artifact_header(context: &PeerConfigContext) -> Vec<String> {
    let title = format!(
        "# === NetMonitor - WireGuard - {} ===",
        ascii_safe(&context.peer_name)
    );
    if !context.private_key_consumed() {
        return vec![title];
    }
    vec![
        title,
        "# ATENCAO: a chave privada deste dispositivo ja foi entregue e nao pode ser".to_string(),
        "# exibida outra vez. Este script NAO vai funcionar como esta.".to_string(),
        "# Use \"Rotacionar chaves\" no NetMonitor e copie o script novo.".to_string(),
    ]
}

/// Resumo dos parâmetros do túnel, idêntico para todos os perfis: é o que o
/// usuário confere antes de colar o script (e o que ele digita à mão quando o
/// equipamento não é nenhum dos suportados).
///
/// **Nunca** inclui a chave privada.
#[must_use]
pub fn artifact_summary(context: &PeerConfigContext) -> Vec<ArtifactSummaryItem> {
    let item = |label: &str, value: String| ArtifactSummaryItem {
        label: label.to_string(),
        value,
    };
    let mut summary = vec![
        item("Dispositivo", context.peer_name.clone()),
        item(
            "IP fixo na VPN",
            format!("{}/{}", context.peer_ip_address, context.prefix_length()),
        ),
        item(
            "Endpoint do servidor",
            format!("{}:{}", context.endpoint_host, context.endpoint_port),
        ),
        item(
            "Chave pública do servidor",
            context.server_public_key.clone(),
        ),
        item("Rotas no túnel (AllowedIPs)", context.vpn_cidr.clone()),
        item("NetMonitor na VPN", context.server_vpn_address.clone()),
        item("MTU", context.mtu.to_string()),
        item("Keepalive", format!("{PERSISTENT_KEEPALIVE_SECONDS}s")),
    ];
    if let Some(dns) = context.dns_servers.as_deref().filter(|v| !v.is_empty()) {
        summary.push(item("DNS", dns.to_string()));
    }
    summary.push(item(
        "Chave pré-compartilhada",
        if context.preshared_key.is_some() {
            "sim (incluída no script)".to_string()
        } else {
            "não".to_string()
        },
    ));
    if context.snmp_enabled {
        summary.push(item(
            "SNMP",
            format!("community \"{}\"", context.community()),
        ));
    }
    summary
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(crate) fn contexto() -> PeerConfigContext {
        PeerConfigContext {
            peer_name: "filial-01".into(),
            peer_ip_address: "10.8.0.11".into(),
            vpn_cidr: "10.8.0.0/24".into(),
            server_vpn_address: "10.8.0.1".into(),
            client_private_key: "CHAVE-PRIVADA-DO-CLIENTE".into(),
            server_public_key: "CHAVE-PUBLICA-DO-SERVIDOR".into(),
            preshared_key: Some("CHAVE-PRE-COMPARTILHADA".into()),
            endpoint_host: "vpn.exemplo.com.br".into(),
            endpoint_port: 51_820,
            mtu: 1_420,
            dns_servers: None,
            snmp_enabled: false,
            snmp_community: None,
        }
    }

    #[test]
    fn ascii_safe_remove_acento_e_pontuacao_tipografica() {
        assert_eq!(ascii_safe("Roteador São João"), "Roteador Sao Joao");
        assert_eq!(ascii_safe("Matriz — Sede"), "Matriz - Sede");
        assert_eq!(ascii_safe("“aspas”"), "\"aspas\"");
        assert_eq!(ascii_safe("  Ação  "), "Acao");
    }

    #[test]
    fn o_cabecalho_avisa_quando_a_chave_ja_foi_consumida() {
        let mut context = contexto();
        assert_eq!(artifact_header(&context).len(), 1);

        context.client_private_key = PRIVATE_KEY_UNAVAILABLE.to_string();
        let header = artifact_header(&context);
        assert_eq!(header.len(), 4);
        assert!(header[1].contains("ja foi entregue"));
        assert!(header[3].contains("Rotacionar chaves"));
    }

    #[test]
    fn o_resumo_nunca_traz_a_chave_privada() {
        let summary = artifact_summary(&contexto());
        let serializado = serde_json::to_string(&summary).unwrap();
        assert!(!serializado.contains("CHAVE-PRIVADA-DO-CLIENTE"));
        assert!(serializado.contains("CHAVE-PUBLICA-DO-SERVIDOR"));
    }

    #[test]
    fn o_resumo_cresce_com_dns_e_snmp() {
        let base = artifact_summary(&contexto()).len();
        let mut context = contexto();
        context.dns_servers = Some("10.8.0.1".into());
        context.snmp_enabled = true;
        context.snmp_community = Some("netmon".into());
        let summary = artifact_summary(&context);
        assert_eq!(summary.len(), base + 2);
        assert!(summary.iter().any(|item| item.value.contains("netmon")));
    }

    #[test]
    fn prefixo_e_community_tem_padrao_seguro() {
        let mut context = contexto();
        assert_eq!(context.prefix_length(), "24");
        assert_eq!(context.community(), "public");
        context.vpn_cidr = "sem-barra".into();
        assert_eq!(context.prefix_length(), "24");
    }

    #[test]
    fn a_entrega_serializa_em_minusculas_para_o_frontend() {
        assert_eq!(
            serde_json::to_value(ArtifactDelivery::Qrcode).unwrap(),
            serde_json::json!("qrcode")
        );
    }
}
