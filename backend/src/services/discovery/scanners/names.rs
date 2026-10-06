//! Nomes dos hosts vivos: DNS reverso (PTR) e NetBIOS.
//!
//! O roteador que entrega DHCP costuma registrar o nome de cada cliente no
//! próprio DNS — perguntar o PTR a ele (o gateway) acha "notebook-ana" onde o
//! ping só via um IP. Máquinas Windows e Samba respondem ainda o nome NetBIOS,
//! o grupo de trabalho e o MAC, inclusive em rede roteada, onde não há ARP.
//!
//! Cada protocolo usa **um** socket UDP para a faixa inteira: as consultas
//! saem todas de uma vez e as respostas são casadas pelo id (DNS) ou pelo
//! endereço de origem (NetBIOS).

use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    time::Duration,
};

use hickory_proto::{
    op::{Message, MessageType, OpCode, Query},
    rr::{DNSClass, Name, RData, RecordType},
    serialize::binary::BinDecodable,
};
use tokio::{net::UdpSocket, time::Instant};

use crate::services::{discovery::merger::DiscoveredHost, network_tools::udp_probes::probe_for};

const WINDOW: Duration = Duration::from_millis(1_500);
/// Servidores DNS perguntados — o gateway e os do sistema.
const MAX_DNS_SERVERS: usize = 3;

/// Nome e MAC de cada host vivo que respondeu.
pub async fn resolve(ips: &[IpAddr], gateway: Option<IpAddr>) -> Vec<DiscoveredHost> {
    let ipv4: Vec<Ipv4Addr> = ips
        .iter()
        .filter_map(|ip| match ip {
            IpAddr::V4(ip) => Some(*ip),
            IpAddr::V6(_) => None,
        })
        .collect();
    if ipv4.is_empty() {
        return Vec::new();
    }
    let servers = dns_servers(&ipv4, gateway).await;
    let (ptr, netbios) = tokio::join!(reverse_dns(&ipv4, &servers), netbios(&ipv4));

    let mut hosts = BTreeMap::<Ipv4Addr, DiscoveredHost>::new();
    for (ip, name) in ptr {
        hosts.insert(
            ip,
            DiscoveredHost {
                ip_address: ip.to_string(),
                hostname: Some(name.clone()),
                data: serde_json::json!({ "scanner": "dns", "dnsName": name }),
                ..Default::default()
            },
        );
    }
    for (ip, node) in netbios {
        let host = hosts.entry(ip).or_insert_with(|| DiscoveredHost {
            ip_address: ip.to_string(),
            data: serde_json::json!({ "scanner": "netbios" }),
            ..Default::default()
        });
        host.hostname.get_or_insert_with(|| node.name.clone());
        if host.mac_address.is_none() {
            host.mac_address = node.mac.clone();
        }
        host.data["netbios"] = serde_json::json!({
            "name": node.name,
            "workgroup": node.workgroup,
        });
    }
    hosts.into_values().collect()
}

/// O gateway da rede primeiro (é quem conhece os nomes do DHCP), depois os
/// gateways candidatos da sub-rede (.1) e por fim os servidores do sistema.
async fn dns_servers(ips: &[Ipv4Addr], gateway: Option<IpAddr>) -> Vec<SocketAddr> {
    let mut servers: Vec<SocketAddr> = gateway
        .into_iter()
        .map(|ip| SocketAddr::new(ip, 53))
        .collect();

    // Se o gateway não for informado explicitamente no cadastro, deduz os
    // roteadores/DNS padrão das sub-redes dos próprios IPs (o .1 da sub-rede).
    if servers.is_empty() {
        let mut candidates = std::collections::BTreeSet::new();
        for ip in ips {
            let [a, b, c, _] = ip.octets();
            candidates.insert(Ipv4Addr::new(a, b, c, 1));
        }
        for candidate in candidates {
            servers.push(SocketAddr::new(IpAddr::V4(candidate), 53));
        }
    }

    if let Ok(content) = tokio::fs::read_to_string("/etc/resolv.conf").await {
        servers.extend(
            parse_resolv_conf(&content)
                .into_iter()
                .map(|ip| SocketAddr::new(ip, 53)),
        );
    }
    servers.dedup();
    servers.truncate(MAX_DNS_SERVERS + 2);
    servers
}

fn parse_resolv_conf(content: &str) -> Vec<IpAddr> {
    content
        .lines()
        .filter_map(|line| line.trim().strip_prefix("nameserver"))
        .filter_map(|rest| IpAddr::from_str(rest.trim()).ok())
        .filter(IpAddr::is_ipv4)
        .collect()
}

fn ptr_query(id: u16, ip: Ipv4Addr) -> Option<Vec<u8>> {
    let [a, b, c, d] = ip.octets();
    let name = Name::from_str(&format!("{d}.{c}.{b}.{a}.in-addr.arpa.")).ok()?;
    let mut query = Query::query(name, RecordType::PTR);
    query.set_query_class(DNSClass::IN);
    let mut message = Message::new();
    message
        .set_id(id)
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true)
        .add_query(query);
    message.to_vec().ok()
}

/// Nome de PTR que só repete o IP (`10-0-0-5.provedor.net`) não diz nada.
fn meaningful_ptr(ip: Ipv4Addr, name: &str) -> Option<String> {
    let name = name.trim().trim_end_matches('.');
    let [a, b, c, d] = ip.octets();
    let generic = [
        format!("{a}-{b}-{c}-{d}"),
        format!("{d}-{c}-{b}-{a}"),
        format!("{a}.{b}.{c}.{d}"),
        format!("{d}.{c}.{b}.{a}"),
    ];
    (!name.is_empty()
        && !name.eq_ignore_ascii_case("localhost")
        && !generic
            .iter()
            .any(|pattern| name.contains(pattern.as_str())))
    .then(|| name.to_string())
}

async fn reverse_dns(ips: &[Ipv4Addr], servers: &[SocketAddr]) -> BTreeMap<Ipv4Addr, String> {
    let mut names = BTreeMap::new();
    if servers.is_empty() {
        return names;
    }
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return names;
    };
    for (index, ip) in ips.iter().enumerate() {
        let Ok(id) = u16::try_from(index) else {
            break;
        };
        let Some(query) = ptr_query(id, *ip) else {
            continue;
        };
        for server in servers {
            let _ = socket.send_to(&query, server).await;
        }
    }
    let deadline = Instant::now() + WINDOW;
    let mut buffer = [0_u8; 1_500];
    while names.len() < ips.len() {
        let Ok(Ok((read, _))) =
            tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
        else {
            break;
        };
        let Ok(message) = Message::from_bytes(&buffer[..read]) else {
            continue;
        };
        let Some(ip) = ips.get(usize::from(message.id())) else {
            continue;
        };
        let name = message
            .answers()
            .iter()
            .find_map(|record| match record.data() {
                Some(RData::PTR(ptr)) => meaningful_ptr(*ip, &ptr.0.to_utf8()),
                _ => None,
            });
        if let Some(name) = name {
            names.entry(*ip).or_insert(name);
        }
    }
    names
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetbiosNode {
    name: String,
    workgroup: Option<String>,
    mac: Option<String>,
}

async fn netbios(ips: &[Ipv4Addr]) -> BTreeMap<Ipv4Addr, NetbiosNode> {
    let mut nodes = BTreeMap::new();
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return nodes;
    };
    let query = probe_for(137);
    for ip in ips {
        let _ = socket.send_to(&query, (*ip, 137)).await;
    }
    let deadline = Instant::now() + WINDOW;
    let mut buffer = [0_u8; 1_500];
    while nodes.len() < ips.len() {
        let Ok(Ok((read, source))) =
            tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
        else {
            break;
        };
        let IpAddr::V4(ip) = source.ip() else {
            continue;
        };
        if let Some(node) = parse_node_status(&buffer[..read]) {
            nodes.entry(ip).or_insert(node);
        }
    }
    nodes
}

/// Resposta NBSTAT (RFC 1002 §4.2.18): cabeçalho, o nome consultado, os
/// campos do registro e a tabela de nomes, seguida do "unit id" — o MAC.
fn parse_node_status(packet: &[u8]) -> Option<NetbiosNode> {
    const HEADER: usize = 12;
    let mut offset = match *packet.get(HEADER)? {
        // Nome completo: comprimento 0x20, 32 bytes codificados e o zero final.
        0x20 => HEADER + 34,
        // Ponteiro de compressão.
        byte if byte & 0xC0 == 0xC0 => HEADER + 2,
        _ => return None,
    };
    // tipo (2) + classe (2) + TTL (4) + tamanho dos dados (2)
    offset += 10;
    let count = usize::from(*packet.get(offset)?);
    offset += 1;
    let mut name = None;
    let mut workgroup = None;
    for _ in 0..count {
        let entry = packet.get(offset..offset + 18)?;
        offset += 18;
        let label = String::from_utf8_lossy(&entry[..15]).trim().to_string();
        let suffix = entry[15];
        let group = entry[16] & 0x80 != 0;
        if suffix == 0x00 && !label.is_empty() {
            if group {
                workgroup.get_or_insert(label);
            } else {
                name.get_or_insert(label);
            }
        }
    }
    let mac = packet
        .get(offset..offset + 6)
        .filter(|bytes| bytes.iter().any(|byte| *byte != 0))
        .map(|bytes| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(":")
        });
    Some(NetbiosNode {
        name: name?,
        workgroup,
        mac,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_os_servidores_ipv4_do_resolv_conf() {
        let servers = parse_resolv_conf(
            "# gerado\nnameserver 127.0.0.11\nnameserver ::1\nsearch lan\nnameserver 10.0.0.1\n",
        );
        assert_eq!(
            servers,
            vec![
                "127.0.0.11".parse::<IpAddr>().unwrap(),
                "10.0.0.1".parse().unwrap()
            ]
        );
    }

    #[test]
    fn ptr_generico_e_descartado() {
        let ip = Ipv4Addr::new(10, 0, 0, 5);
        assert_eq!(
            meaningful_ptr(ip, "notebook-ana.lan."),
            Some("notebook-ana.lan".into())
        );
        assert_eq!(meaningful_ptr(ip, "10-0-0-5.provedor.net."), None);
        assert_eq!(meaningful_ptr(ip, "5.0.0.10.in-addr.arpa."), None);
    }

    #[test]
    fn consulta_ptr_usa_o_nome_reverso() {
        let bytes = ptr_query(7, Ipv4Addr::new(192, 168, 1, 20)).unwrap();
        let message = Message::from_bytes(&bytes).unwrap();
        assert_eq!(message.id(), 7);
        assert_eq!(
            message.queries()[0].name().to_utf8(),
            "20.1.168.192.in-addr.arpa."
        );
    }

    #[test]
    fn interpreta_tabela_nbstat() {
        let mut packet = vec![0x80, 0x94, 0x84, 0x00, 0, 0, 0, 1, 0, 0, 0, 0];
        packet.push(0x20);
        packet.extend([b'C'; 32]);
        packet.push(0);
        packet.extend([0x00, 0x21, 0x00, 0x01, 0, 0, 0, 0, 0x00, 0x41]);
        packet.push(2);
        let mut entry = |label: &str, suffix: u8, flags: u8| {
            let mut name = format!("{label:<15}").into_bytes();
            name.push(suffix);
            name.extend([flags, 0x00]);
            packet.extend(name);
        };
        entry("ESTACAO-07", 0x00, 0x04);
        entry("ESCRITORIO", 0x00, 0x84);
        packet.extend([0x00, 0x1b, 0x21, 0xaa, 0xbb, 0xcc]);
        let node = parse_node_status(&packet).unwrap();
        assert_eq!(node.name, "ESTACAO-07");
        assert_eq!(node.workgroup.as_deref(), Some("ESCRITORIO"));
        assert_eq!(node.mac.as_deref(), Some("00:1b:21:aa:bb:cc"));
    }
}
