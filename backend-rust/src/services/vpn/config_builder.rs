//! Montagem do conteúdo do `wg0.conf` do servidor (§8.10.2).
//!
//! Função pura: não toca no banco nem no disco, o que a torna trivialmente
//! testável — e é o que permite ter um snapshot do arquivo inteiro no teste.

use crate::services::{shared::errors::AppResult, vpn::cidr::parse_cidr};

#[derive(Debug, Clone)]
pub struct ServerInterfaceInput {
    pub interface_name: String,
    pub address: String,
    pub cidr: String,
    pub listen_port: i32,
    pub private_key: String,
    pub mtu: i32,
    /// Quando falso, aplica isolamento entre peers (recomendado p/ monitoramento).
    pub allow_peer_to_peer: bool,
}

#[derive(Debug, Clone)]
pub struct PeerEntryInput {
    pub name: String,
    pub public_key: String,
    pub preshared_key: Option<String>,
    pub ip_address: String,
    pub enabled: bool,
}

/// Regras de isolamento (matriz de paridade #36).
///
/// Ficam em `PostUp`/`PostDown` porque `wg syncconf` (hot-reload) só aplica
/// peers — o firewall é montado quando a interface sobe.
fn build_isolation_rules(input: &ServerInterfaceInput) -> Vec<String> {
    let iface = &input.interface_name;
    if input.allow_peer_to_peer {
        return vec![
            format!("PostUp = iptables -A FORWARD -i {iface} -o {iface} -j ACCEPT"),
            format!("PostDown = iptables -D FORWARD -i {iface} -o {iface} -j ACCEPT"),
        ];
    }
    let address = &input.address;
    vec![
        format!("PostUp = iptables -A FORWARD -i {iface} -d {address} -j ACCEPT"),
        format!("PostUp = iptables -A FORWARD -i {iface} -o {iface} -j DROP"),
        format!("PostDown = iptables -D FORWARD -i {iface} -d {address} -j ACCEPT"),
        format!("PostDown = iptables -D FORWARD -i {iface} -o {iface} -j DROP"),
    ]
}

/// # Errors
///
/// Falha quando o CIDR do servidor é inválido.
pub fn build_interface_section(input: &ServerInterfaceInput) -> AppResult<String> {
    let prefix_length = parse_cidr(&input.cidr)?.prefix_length;
    let mut lines = vec![
        "[Interface]".to_string(),
        format!("Address = {}/{prefix_length}", input.address),
        format!("ListenPort = {}", input.listen_port),
        format!("PrivateKey = {}", input.private_key),
        format!("MTU = {}", input.mtu),
    ];
    lines.extend(build_isolation_rules(input));
    Ok(lines.join("\n"))
}

#[must_use]
pub fn build_peer_section(peer: &PeerEntryInput) -> String {
    let mut lines = vec![
        format!("# {}", peer.name),
        "[Peer]".to_string(),
        format!("PublicKey = {}", peer.public_key),
    ];
    if let Some(preshared_key) = peer.preshared_key.as_deref().filter(|key| !key.is_empty()) {
        lines.push(format!("PresharedKey = {preshared_key}"));
    }
    // `/32`: cada peer só pode originar tráfego do próprio endereço da VPN.
    lines.push(format!("AllowedIPs = {}/32", peer.ip_address));
    lines.join("\n")
}

/// Gera o arquivo completo, ignorando peers desabilitados (revogação imediata).
///
/// # Errors
///
/// Falha quando o CIDR do servidor é inválido.
pub fn build(server: &ServerInterfaceInput, peers: &[PeerEntryInput]) -> AppResult<String> {
    let mut sections = vec![
        "# Gerado automaticamente pelo NetMonitor — não editar manualmente.".to_string(),
        build_interface_section(server)?,
    ];
    sections.extend(
        peers
            .iter()
            .filter(|peer| peer.enabled)
            .map(build_peer_section),
    );
    Ok(format!("{}\n", sections.join("\n\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn servidor(allow_peer_to_peer: bool) -> ServerInterfaceInput {
        ServerInterfaceInput {
            interface_name: "wg0".into(),
            address: "10.8.0.1".into(),
            cidr: "10.8.0.0/24".into(),
            listen_port: 51_820,
            private_key: "CHAVE-PRIVADA-DO-SERVIDOR".into(),
            mtu: 1_420,
            allow_peer_to_peer,
        }
    }

    fn peer(name: &str, enabled: bool, preshared_key: Option<&str>) -> PeerEntryInput {
        PeerEntryInput {
            name: name.into(),
            public_key: format!("PUB-{name}"),
            preshared_key: preshared_key.map(ToString::to_string),
            ip_address: "10.8.0.11".into(),
            enabled,
        }
    }

    #[test]
    fn isolamento_e_o_padrao_e_usa_drop_entre_peers() {
        let conf = build(&servidor(false), &[]).unwrap();
        assert!(conf.contains("PostUp = iptables -A FORWARD -i wg0 -d 10.8.0.1 -j ACCEPT"));
        assert!(conf.contains("PostUp = iptables -A FORWARD -i wg0 -o wg0 -j DROP"));
        assert!(conf.contains("PostDown = iptables -D FORWARD -i wg0 -o wg0 -j DROP"));
    }

    #[test]
    fn com_peer_to_peer_liberado_nao_ha_drop() {
        let conf = build(&servidor(true), &[]).unwrap();
        assert!(conf.contains("PostUp = iptables -A FORWARD -i wg0 -o wg0 -j ACCEPT"));
        assert!(!conf.contains("DROP"));
    }

    #[test]
    fn peer_desabilitado_some_do_arquivo() {
        // Revogação é imediata: o peer sai do `wg0.conf` e o `syncconf` derruba
        // o túnel dele sem tocar nos outros.
        let conf = build(
            &servidor(false),
            &[peer("ativo", true, None), peer("revogado", false, None)],
        )
        .unwrap();
        assert!(conf.contains("# ativo"));
        assert!(!conf.contains("# revogado"));
    }

    #[test]
    fn a_preshared_key_so_aparece_quando_existe() {
        let com = build_peer_section(&peer("a", true, Some("PSK")));
        assert!(com.contains("PresharedKey = PSK"));
        let sem = build_peer_section(&peer("a", true, None));
        assert!(!sem.contains("PresharedKey"));
        // String vazia também não vira linha — geraria um `PresharedKey = ` que
        // o `wg` recusa ao carregar o arquivo.
        let vazia = build_peer_section(&peer("a", true, Some("")));
        assert!(!vazia.contains("PresharedKey"));
    }

    #[test]
    fn allowed_ips_do_peer_e_sempre_barra_32() {
        assert!(build_peer_section(&peer("a", true, None)).contains("AllowedIPs = 10.8.0.11/32"));
    }

    #[test]
    fn o_arquivo_inteiro_bate_com_o_esperado() {
        let conf = build(&servidor(false), &[peer("filial-01", true, Some("PSK"))]).unwrap();
        insta::assert_snapshot!(conf);
    }
}
