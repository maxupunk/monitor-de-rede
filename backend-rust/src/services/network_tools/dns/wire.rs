//! Codificação e leitura do wire format DNS com `hickory-proto`.

use std::str::FromStr;

use hickory_proto::{
    op::{Message, MessageType, OpCode, Query},
    rr::{DNSClass, Name, RecordType},
    serialize::binary::BinDecodable,
};

use crate::services::shared::errors::{AppError, AppResult};

pub const DNS_RCODE_MESSAGES: &[(&str, &str)] = &[
    ("NoError", "Consulta DNS concluída"),
    ("FormErr", "Servidor DNS não entendeu a consulta"),
    ("ServFail", "Falha interna do servidor DNS"),
    ("NXDomain", "Nome de domínio não encontrado"),
    ("NotImp", "Servidor DNS não suporta esta consulta"),
    ("Refused", "Servidor DNS recusou a consulta"),
];

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsAnswer {
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub ttl: u32,
    pub value: String,
}

pub fn parse_record_type(raw: Option<&str>) -> AppResult<RecordType> {
    let value = raw.unwrap_or("A").trim().to_ascii_uppercase();
    match value.as_str() {
        "A" => Ok(RecordType::A),
        "AAAA" => Ok(RecordType::AAAA),
        "CNAME" => Ok(RecordType::CNAME),
        "MX" => Ok(RecordType::MX),
        "TXT" => Ok(RecordType::TXT),
        "NS" => Ok(RecordType::NS),
        _ => Err(AppError::validation("Tipo de registro DNS inválido")),
    }
}

pub fn record_type_name(record_type: RecordType) -> &'static str {
    match record_type {
        RecordType::A => "A",
        RecordType::AAAA => "AAAA",
        RecordType::CNAME => "CNAME",
        RecordType::MX => "MX",
        RecordType::TXT => "TXT",
        RecordType::NS => "NS",
        _ => "A",
    }
}

pub fn encode_query(hostname: &str, record_type: RecordType) -> AppResult<Vec<u8>> {
    let name = Name::from_str(hostname.trim())
        .map_err(|_| AppError::validation("Hostname DNS inválido"))?;
    let mut query = Query::query(name, record_type);
    query.set_query_class(DNSClass::IN);
    let mut message = Message::new();
    message
        .set_id(rand::random())
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true)
        .add_query(query);
    message.to_vec().map_err(|error| {
        AppError::Internal(anyhow::anyhow!(
            "Não foi possível codificar consulta DNS: {error}"
        ))
    })
}

pub fn decode_message(bytes: &[u8]) -> AppResult<Message> {
    Message::from_bytes(bytes)
        .map_err(|error| AppError::Internal(anyhow::anyhow!("Resposta DNS inválida: {error}")))
}

#[must_use]
pub fn response_message(message: &Message) -> String {
    let code = format!("{:?}", message.response_code());
    DNS_RCODE_MESSAGES
        .iter()
        .find_map(|(name, description)| (*name == code).then_some((*description).to_string()))
        .unwrap_or_else(|| format!("Resposta DNS: {code}"))
}

#[must_use]
pub fn answers(message: &Message) -> Vec<DnsAnswer> {
    message
        .answers()
        .iter()
        .filter_map(|record| {
            let data = record.data()?;
            Some(DnsAnswer {
                name: record.name().to_utf8(),
                record_type: record.record_type().to_string(),
                ttl: record.ttl(),
                // Hickory já formata MX como "prioridade exchange" e concatena
                // fragments TXT, preservando o contrato legado sem parser manual.
                value: data.to_string(),
            })
        })
        .collect()
}
