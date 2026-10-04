//! mDNS/Bonjour best-effort. Interfaces sem multicast apenas devolvem lista vazia.
//!
//! Uma consulta só pergunta por todos os serviços de
//! [`fingerprints::MDNS_RULES`] — Chromecast, HomeKit, Shelly, impressoras —
//! e cada resposta é atribuída ao IP que a enviou. Cada aparelho se descreve:
//! os serviços que oferece, o nome que o dono deu ("TV da Sala") e, no TXT, o
//! modelo.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

use hickory_proto::{
    op::{Message, MessageType, OpCode, Query},
    rr::{Name, RData, RecordType},
    serialize::binary::BinDecodable,
};
use tokio::net::UdpSocket;

use crate::services::discovery::{fingerprints, merger::DiscoveredHost};

const GROUP: &str = "224.0.0.251:5353";
const WINDOW: Duration = Duration::from_secs(3);
/// Reenvio: multicast perde pacote, e aparelho que dorme acorda com o 1º.
const RESEND_AFTER: Duration = Duration::from_millis(900);
/// Chaves de TXT que costumam trazer o modelo do aparelho.
const MODEL_KEYS: &[&str] = &["md", "model", "mdl", "am", "ty", "product", "usb_mdl"];
/// Chaves de TXT com o nome dado pelo dono.
const FRIENDLY_KEYS: &[&str] = &["fn", "n"];

pub async fn scan() -> Vec<DiscoveredHost> {
    let Some(query) = query() else {
        return vec![];
    };
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return vec![];
    };
    if socket.send_to(&query, GROUP).await.is_err() {
        return vec![];
    }
    let started = tokio::time::Instant::now();
    let deadline = started + WINDOW;
    let mut resent = false;
    let mut seen = BTreeMap::<Ipv4Addr, Seen>::new();
    let mut buffer = [0_u8; 9_000];
    loop {
        let wake = if resent {
            deadline
        } else {
            started + RESEND_AFTER
        };
        match tokio::time::timeout_at(wake, socket.recv_from(&mut buffer)).await {
            Ok(Ok((read, source))) => {
                if let (IpAddr::V4(ip), Ok(message)) =
                    (source.ip(), Message::from_bytes(&buffer[..read]))
                {
                    seen.entry(ip).or_default().read(&message);
                }
            }
            Ok(Err(_)) => break,
            Err(_) if !resent => {
                resent = true;
                let _ = socket.send_to(&query, GROUP).await;
            }
            Err(_) => break,
        }
    }
    seen.into_iter()
        .filter(|(_, seen)| !seen.is_empty())
        .map(|(ip, seen)| seen.into_host(ip))
        .collect()
}

/// Uma mensagem com uma pergunta PTR por serviço conhecido.
fn query() -> Option<Vec<u8>> {
    let mut message = Message::new();
    message
        .set_id(0)
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(false);
    for rule in fingerprints::MDNS_RULES {
        let name = Name::from_ascii(format!("{}.local.", rule.service)).ok()?;
        message.add_query(Query::query(name, RecordType::PTR));
    }
    message.to_vec().ok()
}

/// O que um IP contou de si.
#[derive(Default)]
struct Seen {
    host_name: Option<String>,
    services: BTreeSet<String>,
    instances: BTreeSet<String>,
    model: Option<String>,
    friendly: Option<String>,
}

/// `Sala._googlecast._tcp.local.` → (`Sala`, `_googlecast._tcp`).
fn split_instance(name: &str) -> Option<(String, String)> {
    let name = name.trim_end_matches('.').trim_end_matches(".local");
    let marker = name.find("._")?;
    let instance = name[..marker].replace("\\032", " ");
    let service = name[marker + 1..].to_string();
    Some((instance, service))
}

fn service_type(name: &str) -> String {
    name.trim_end_matches('.')
        .trim_end_matches(".local")
        .to_string()
}

impl Seen {
    fn is_empty(&self) -> bool {
        self.host_name.is_none() && self.services.is_empty() && self.instances.is_empty()
    }

    fn read(&mut self, message: &Message) {
        for record in message.answers().iter().chain(message.additionals()) {
            let owner = record.name().to_utf8();
            match record.data() {
                Some(RData::PTR(target)) => {
                    let target = target.0.to_utf8();
                    if owner.starts_with("_services._dns-sd") {
                        self.services.insert(service_type(&target));
                    } else if let Some((instance, service)) = split_instance(&target) {
                        self.services.insert(service);
                        if !instance.is_empty() {
                            self.instances.insert(instance);
                        }
                    }
                }
                Some(RData::SRV(srv)) => {
                    if let Some((_, service)) = split_instance(&owner) {
                        self.services.insert(service);
                    }
                    self.host_name
                        .get_or_insert_with(|| srv.target().to_utf8().trim_end_matches('.').into());
                }
                Some(RData::A(_)) => {
                    self.host_name = Some(owner.trim_end_matches('.').into());
                }
                Some(RData::TXT(txt)) => {
                    for entry in txt.txt_data() {
                        let entry = String::from_utf8_lossy(entry);
                        let Some((key, value)) = entry.split_once('=') else {
                            continue;
                        };
                        let key = key.to_ascii_lowercase();
                        let value = value.trim();
                        if value.is_empty() {
                            continue;
                        }
                        if MODEL_KEYS.contains(&key.as_str()) {
                            self.model.get_or_insert_with(|| value.into());
                        } else if FRIENDLY_KEYS.contains(&key.as_str()) {
                            self.friendly.get_or_insert_with(|| value.into());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn into_host(self, ip: Ipv4Addr) -> DiscoveredHost {
        let mut instances = self.instances;
        if let Some(friendly) = self.friendly {
            instances.insert(friendly);
        }
        DiscoveredHost {
            ip_address: ip.to_string(),
            mdns_name: self.host_name,
            confidence: 70,
            data: serde_json::json!({
                "scanner": "mdns",
                "mdns": {
                    "services": self.services,
                    "instances": instances,
                    "model": self.model,
                },
            }),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use hickory_proto::rr::{rdata, Record};

    use super::*;

    #[test]
    fn consulta_pergunta_todos_os_servicos() {
        let message = Message::from_bytes(&query().unwrap()).unwrap();
        assert_eq!(message.queries().len(), fingerprints::MDNS_RULES.len());
    }

    #[test]
    fn resposta_vira_servicos_instancia_nome_e_modelo() {
        let service = Name::from_ascii("_googlecast._tcp.local.").unwrap();
        let instance = Name::from_ascii("TV-Sala._googlecast._tcp.local.").unwrap();
        let host = Name::from_ascii("Chromecast-abc.local.").unwrap();
        let mut message = Message::new();
        message.add_answer(Record::from_rdata(
            service,
            120,
            RData::PTR(rdata::PTR(instance.clone())),
        ));
        message.add_additional(Record::from_rdata(
            instance.clone(),
            120,
            RData::TXT(rdata::TXT::new(vec![
                "md=Chromecast Ultra".into(),
                "fn=TV da Sala".into(),
            ])),
        ));
        message.add_additional(Record::from_rdata(
            host,
            120,
            RData::A(rdata::A(Ipv4Addr::new(10, 0, 0, 30))),
        ));
        let mut seen = Seen::default();
        seen.read(&message);
        let host = seen.into_host(Ipv4Addr::new(10, 0, 0, 30));
        assert_eq!(host.mdns_name.as_deref(), Some("Chromecast-abc.local"));
        assert_eq!(
            host.data["mdns"]["services"],
            serde_json::json!(["_googlecast._tcp"])
        );
        assert_eq!(host.data["mdns"]["model"], "Chromecast Ultra");
        assert_eq!(
            host.data["mdns"]["instances"],
            serde_json::json!(["TV da Sala", "TV-Sala"])
        );
    }
}
