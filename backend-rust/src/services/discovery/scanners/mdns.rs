//! mDNS best-effort. Interfaces sem multicast apenas devolvem lista vazia.

use crate::services::{discovery::merger::DiscoveredHost, network_tools::dns::wire::encode_query};
use hickory_proto::{op::Message, rr::RecordType, serialize::binary::BinDecodable};
use std::{net::Ipv4Addr, time::Duration};
use tokio::net::UdpSocket;

pub async fn scan() -> Vec<DiscoveredHost> {
    let Ok(query) = encode_query("_services._dns-sd._udp.local", RecordType::PTR) else {
        return vec![];
    };
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return vec![];
    };
    if socket.send_to(&query, "224.0.0.251:5353").await.is_err() {
        return vec![];
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut found = Vec::new();
    let mut buffer = [0_u8; 4096];
    while let Ok(Ok((read, _))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
    {
        let Ok(message) = Message::from_bytes(&buffer[..read]) else {
            continue;
        };
        for record in message.answers().iter().chain(message.additionals()) {
            if record.record_type() == RecordType::A {
                if let Some(data) = record.data() {
                    if let Ok(ip) = data.to_string().parse::<Ipv4Addr>() {
                        found.push(DiscoveredHost {
                            ip_address: ip.to_string(),
                            mdns_name: Some(record.name().to_utf8()),
                            confidence: 70,
                            data: serde_json::json!({ "scanner":"mdns" }),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
    found
}
