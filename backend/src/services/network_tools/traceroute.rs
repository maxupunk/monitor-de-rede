//! Serviço de Traceroute ICMP nativo em Rust.
//!
//! Envia sondas ICMP com TTL incremental (1 a max_hops), capturando mensagens
//! ICMP Time Exceeded (Tipo 11) de roteadores intermediários e Echo Reply do
//! destino final. Não requer privilégios de root (usa SOCK_DGRAM e ADR 003).

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use socket2::Type;
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    dtos::diagnostics::TracerouteHop,
    services::network_tools::{
        dns::wire::{answers, decode_message, encode_query},
        icmp_probe::round_two,
    },
};

pub const DEFAULT_MAX_HOPS: u8 = 30;
pub const MAX_ALLOWED_HOPS: u8 = 64;
pub const DEFAULT_TIMEOUT_MS: u64 = 1_500;
pub const DEFAULT_PROBES_PER_HOP: u8 = 3;
const PAYLOAD_LEN: usize = 56;
const REVERSE_DNS_TIMEOUT: Duration = Duration::from_millis(400);

#[derive(Debug, Clone)]
pub struct TracerouteOptions {
    pub max_hops: u8,
    pub timeout_ms: u64,
    pub probes_per_hop: u8,
}

impl Default for TracerouteOptions {
    fn default() -> Self {
        Self {
            max_hops: DEFAULT_MAX_HOPS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            probes_per_hop: DEFAULT_PROBES_PER_HOP,
        }
    }
}

impl TracerouteOptions {
    pub fn new(max_hops: Option<u8>, timeout_ms: Option<u64>, probes_per_hop: Option<u8>) -> Self {
        Self {
            max_hops: max_hops
                .unwrap_or(DEFAULT_MAX_HOPS)
                .clamp(1, MAX_ALLOWED_HOPS),
            timeout_ms: timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).clamp(200, 5_000),
            probes_per_hop: probes_per_hop.unwrap_or(DEFAULT_PROBES_PER_HOP).clamp(1, 3),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TracerouteEvent {
    Hop(TracerouteHop),
    Done,
}

/// Executa o traceroute ICMP até o endereço IP especificado.
///
/// Cada salto é transmitido pelo canal `sender` assim que suas sondas
/// concluem, permitindo streaming em tempo real no frontend.
pub async fn execute_traceroute(
    target: IpAddr,
    options: TracerouteOptions,
    sender: mpsc::Sender<TracerouteEvent>,
    cancel: CancellationToken,
) -> Vec<TracerouteHop> {
    let mut hops = Vec::new();
    let probe_timeout = Duration::from_millis(options.timeout_ms);
    let payload = vec![0u8; PAYLOAD_LEN];
    let is_ipv4 = target.is_ipv4();

    for hop_num in 1..=options.max_hops {
        if cancel.is_cancelled() {
            break;
        }

        let mut rtts: Vec<Option<f64>> = Vec::with_capacity(usize::from(options.probes_per_hop));
        let mut resolved_ip: Option<IpAddr> = None;
        let mut reached_target = false;

        for seq in 0..options.probes_per_hop {
            if cancel.is_cancelled() {
                break;
            }

            let config = Config::builder()
                .kind(if is_ipv4 { ICMP::V4 } else { ICMP::V6 })
                .sock_type_hint(Type::DGRAM)
                .ttl(u32::from(hop_num))
                .build();

            let client = match Client::new(&config) {
                Ok(c) => c,
                Err(err) => {
                    tracing::warn!(%err, hop = hop_num, "falha ao criar socket ICMP com TTL");
                    break;
                }
            };

            let mut pinger = client.pinger(target, PingIdentifier(rand::random())).await;
            pinger.timeout(probe_timeout);

            match pinger.ping(PingSequence(seq.into()), &payload).await {
                Ok((packet, rtt)) => {
                    let rtt_val = rtt.as_secs_f64() * 1_000.0;
                    rtts.push(Some(round_two(rtt_val)));

                    let (src_ip, is_dest) = match &packet {
                        surge_ping::IcmpPacket::V4(v4) => {
                            let src = IpAddr::V4(v4.get_source());
                            let is_dest = src == target || v4.get_icmp_type().0 == 0;
                            (src, is_dest)
                        }
                        surge_ping::IcmpPacket::V6(v6) => {
                            let src = IpAddr::V6(v6.get_source());
                            let is_dest = src == target || v6.get_icmpv6_type().0 == 129;
                            (src, is_dest)
                        }
                    };

                    if resolved_ip.is_none() {
                        resolved_ip = Some(src_ip);
                    }
                    if is_dest {
                        reached_target = true;
                    }
                }
                Err(_) => {
                    rtts.push(None);
                }
            }
        }

        // Calcula média de RTT das sondas bem-sucedidas
        let successful_rtts: Vec<f64> = rtts.iter().flatten().copied().collect();
        let avg_rtt = if successful_rtts.is_empty() {
            None
        } else {
            let sum: f64 = successful_rtts.iter().sum();
            Some(round_two(sum / successful_rtts.len() as f64))
        };

        let status = if reached_target {
            "reached".to_string()
        } else if resolved_ip.is_some() {
            "intermediate".to_string()
        } else {
            "timeout".to_string()
        };

        // Resolução reversa de DNS assíncrona com timeout estrito
        let hostname = if let Some(ip) = resolved_ip {
            resolve_reverse_dns(ip).await
        } else {
            None
        };

        let hop = TracerouteHop {
            hop: hop_num,
            ip: resolved_ip.map(|ip| ip.to_string()),
            hostname,
            rtt_ms: rtts,
            avg_rtt_ms: avg_rtt,
            status: status.clone(),
        };

        hops.push(hop.clone());
        let _ = sender.send(TracerouteEvent::Hop(hop)).await;

        if reached_target || cancel.is_cancelled() {
            break;
        }
    }

    hops
}

/// Consulta o registro PTR para resolução reversa de hostname.
async fn resolve_reverse_dns(ip: IpAddr) -> Option<String> {
    tokio::time::timeout(REVERSE_DNS_TIMEOUT, async {
        let ptr_name = match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                format!(
                    "{}.{}.{}.{}.in-addr.arpa",
                    octets[3], octets[2], octets[1], octets[0]
                )
            }
            IpAddr::V6(_) => return None,
        };

        let query_bytes = encode_query(&ptr_name, hickory_proto::rr::RecordType::PTR).ok()?;
        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
        let server: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53);
        socket.send_to(&query_bytes, server).await.ok()?;

        let mut buf = [0u8; 512];
        let (len, _) = socket.recv_from(&mut buf).await.ok()?;
        let msg = decode_message(&buf[..len]).ok()?;
        let res = answers(&msg);
        res.into_iter()
            .find(|ans| ans.record_type == "PTR")
            .map(|ans| ans.value.trim_end_matches('.').to_string())
    })
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_respeitam_limites_e_valores_padrao() {
        let opt = TracerouteOptions::new(None, None, None);
        assert_eq!(opt.max_hops, DEFAULT_MAX_HOPS);
        assert_eq!(opt.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(opt.probes_per_hop, DEFAULT_PROBES_PER_HOP);

        let opt_clamped = TracerouteOptions::new(Some(100), Some(50), Some(10));
        assert_eq!(opt_clamped.max_hops, MAX_ALLOWED_HOPS);
        assert_eq!(opt_clamped.timeout_ms, 200);
        assert_eq!(opt_clamped.probes_per_hop, 3);
    }
}
