use std::{collections::BTreeMap, time::Duration};

use rasn::types::ObjectIdentifier;
use rasn_snmp::{
    v2::{GetNextRequest, GetRequest, Pdu, Pdus, VarBind, VarBindValue},
    v2c::Message,
};
use thiserror::Error;
use tokio::net::UdpSocket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnmpVersion {
    V1,
    V2c,
    V3,
}
impl SnmpVersion {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "v1" | "1" => Some(Self::V1),
            "v2c" | "2c" | "2" => Some(Self::V2c),
            "v3" | "3" => Some(Self::V3),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnmpConfig {
    pub host: String,
    pub version: SnmpVersion,
    pub community: String,
    pub username: Option<String>,
    pub auth_protocol: Option<String>,
    pub auth_key: Option<String>,
    pub priv_protocol: Option<String>,
    pub priv_key: Option<String>,
    pub port: u16,
    pub timeout_ms: u64,
}
impl SnmpConfig {
    #[must_use]
    pub fn v2c(host: impl Into<String>, community: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            version: SnmpVersion::V2c,
            community: community.into(),
            username: None,
            auth_protocol: None,
            auth_key: None,
            priv_protocol: None,
            priv_key: None,
            port,
            timeout_ms: 4_000,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum SnmpValue {
    Number(u64),
    Text(String),
}
impl SnmpValue {
    #[must_use]
    pub fn number(&self) -> Option<u64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(value) => first_number(value),
        }
    }
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
        }
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnmpWalkEntry {
    pub oid: String,
    pub value: SnmpValue,
}

#[derive(Debug, Error)]
pub enum SnmpError {
    #[error("OID SNMP inválido: {0}")]
    Oid(String),
    #[error("Falha ao codificar mensagem SNMP: {0}")]
    Encode(String),
    #[error("Falha ao decodificar resposta SNMP: {0}")]
    Decode(String),
    #[error("Tempo esgotado na consulta SNMP")]
    Timeout,
    #[error("Erro de rede SNMP: {0}")]
    Network(String),
    #[error("Versão SNMP {0} ainda não possui segurança USM configurada")]
    UnsupportedVersion(&'static str),
    #[error("Agente SNMP devolveu erro {0}")]
    Agent(u32),
}

pub struct SnmpClient {
    config: SnmpConfig,
}
impl SnmpClient {
    #[must_use]
    pub fn new(config: SnmpConfig) -> Self {
        Self { config }
    }
    pub async fn get(
        &self,
        oids: &[&str],
    ) -> Result<BTreeMap<String, Option<SnmpValue>>, SnmpError> {
        let request = pdu(i32::from(rand::random::<u16>()), oids)?;
        let response = self.exchange_get(request).await?;
        let mut values = oids
            .iter()
            .map(|oid| ((*oid).to_string(), None))
            .collect::<BTreeMap<_, _>>();
        if response.error_status != Pdu::ERROR_STATUS_NO_ERROR {
            return Err(SnmpError::Agent(response.error_status));
        }
        for bind in response.variable_bindings {
            values.insert(oid_string(&bind.name), value(&bind.value));
        }
        Ok(values)
    }
    pub async fn walk(&self, base_oid: &str) -> Result<Vec<SnmpWalkEntry>, SnmpError> {
        let base = oid_parts(base_oid)?;
        let mut current = base.clone();
        let mut entries = Vec::new();
        for _ in 0..1_024 {
            let response = self
                .exchange_next(pdu_from_parts(i32::from(rand::random::<u16>()), &current))
                .await?;
            if response.error_status != Pdu::ERROR_STATUS_NO_ERROR {
                break;
            }
            let Some(bind) = response.variable_bindings.first() else {
                break;
            };
            let next: Vec<u32> = bind.name.iter().copied().collect();
            if !next.starts_with(&base) {
                break;
            }
            let Some(value) = value(&bind.value) else {
                break;
            };
            entries.push(SnmpWalkEntry {
                oid: oid_string(&bind.name),
                value,
            });
            current = next;
        }
        Ok(entries)
    }
    async fn exchange_get(&self, request: Pdu) -> Result<Pdu, SnmpError> {
        let bytes = rasn::ber::encode(&Message {
            version: 1.into(),
            community: self.config.community.as_bytes().to_vec().into(),
            data: GetRequest(request),
        })
        .map_err(|error| SnmpError::Encode(error.to_string()))?;
        self.exchange(bytes).await
    }
    async fn exchange_next(&self, request: Pdu) -> Result<Pdu, SnmpError> {
        let bytes = rasn::ber::encode(&Message {
            version: 1.into(),
            community: self.config.community.as_bytes().to_vec().into(),
            data: GetNextRequest(request),
        })
        .map_err(|error| SnmpError::Encode(error.to_string()))?;
        self.exchange(bytes).await
    }
    async fn exchange(&self, request: Vec<u8>) -> Result<Pdu, SnmpError> {
        match self.config.version {
            SnmpVersion::V2c => {}
            SnmpVersion::V1 => return Err(SnmpError::UnsupportedVersion("v1")),
            SnmpVersion::V3 => return Err(SnmpError::UnsupportedVersion("v3")),
        }
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|error| SnmpError::Network(error.to_string()))?;
        socket
            .connect((self.config.host.as_str(), self.config.port))
            .await
            .map_err(|error| SnmpError::Network(error.to_string()))?;
        // UDP pode perder pacotes: duas retentativas além da tentativa inicial.
        for _ in 0..=2 {
            socket
                .send(&request)
                .await
                .map_err(|error| SnmpError::Network(error.to_string()))?;
            let mut buffer = vec![0_u8; 65_535];
            match tokio::time::timeout(
                Duration::from_millis(self.config.timeout_ms.max(1)),
                socket.recv(&mut buffer),
            )
            .await
            {
                Ok(Ok(read)) => {
                    let message: Message<Pdus> = rasn::ber::decode(&buffer[..read])
                        .map_err(|error| SnmpError::Decode(error.to_string()))?;
                    if let Pdus::Response(response) = message.data {
                        return Ok(response.0);
                    }
                    return Err(SnmpError::Decode("PDU não é uma resposta".into()));
                }
                Ok(Err(error)) => return Err(SnmpError::Network(error.to_string())),
                Err(_) => continue,
            }
        }
        Err(SnmpError::Timeout)
    }
}
fn oid_parts(raw: &str) -> Result<Vec<u32>, SnmpError> {
    let parts: Result<Vec<_>, _> = raw
        .trim_matches('.')
        .split('.')
        .map(str::parse::<u32>)
        .collect();
    let parts = parts.map_err(|_| SnmpError::Oid(raw.into()))?;
    if parts.len() < 2 {
        Err(SnmpError::Oid(raw.into()))
    } else {
        Ok(parts)
    }
}
fn pdu(request_id: i32, oids: &[&str]) -> Result<Pdu, SnmpError> {
    Ok(Pdu {
        request_id,
        error_status: Pdu::ERROR_STATUS_NO_ERROR,
        error_index: 0,
        variable_bindings: oids
            .iter()
            .map(|oid| {
                oid_parts(oid).map(|parts| VarBind {
                    name: ObjectIdentifier::new_unchecked(parts.into()),
                    value: VarBindValue::Unspecified,
                })
            })
            .collect::<Result<_, _>>()?,
    })
}
fn pdu_from_parts(request_id: i32, oid: &[u32]) -> Pdu {
    Pdu {
        request_id,
        error_status: Pdu::ERROR_STATUS_NO_ERROR,
        error_index: 0,
        variable_bindings: vec![VarBind {
            name: ObjectIdentifier::new_unchecked(oid.to_vec().into()),
            value: VarBindValue::Unspecified,
        }],
    }
}
fn oid_string(oid: &ObjectIdentifier) -> String {
    oid.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(".")
}
fn value(value: &VarBindValue) -> Option<SnmpValue> {
    match value {
        VarBindValue::Value(value) => {
            let raw = format!("{value:?}");
            first_number(&raw)
                .filter(|_number| {
                    raw.chars()
                        .all(|char| char.is_ascii_digit() || !char.is_ascii_alphabetic())
                })
                .map(SnmpValue::Number)
                .or_else(|| Some(SnmpValue::Text(normalize_text(&raw))))
        }
        _ => None,
    }
}
fn first_number(raw: &str) -> Option<u64> {
    raw.split(|char: char| !char.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}
fn normalize_text(raw: &str) -> String {
    raw.trim().trim_matches('"').to_string()
}
