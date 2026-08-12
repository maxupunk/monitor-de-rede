use std::{collections::BTreeMap, time::Duration};

use rasn::types::ObjectIdentifier;
use rasn_smi::{
    v1::{
        ApplicationSyntax as V1ApplicationSyntax, ObjectSyntax as V1ObjectSyntax,
        SimpleSyntax as V1SimpleSyntax,
    },
    v2::{
        ApplicationSyntax as V2ApplicationSyntax, ObjectSyntax as V2ObjectSyntax,
        SimpleSyntax as V2SimpleSyntax,
    },
};
use rasn_snmp::{
    v2::{GetNextRequest, GetRequest, Pdu, Pdus, VarBind, VarBindValue},
    v2c::Message,
    v3,
};
use thiserror::Error;
use tokio::{net::UdpSocket, sync::OnceCell};

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

#[derive(Debug, Clone, PartialEq)]
pub enum SnmpValue {
    Number(u64),
    Text(String),
    /// `OCTET STRING` cru, como veio do agente.
    ///
    /// Os bytes ficam intactos porque só o coletor sabe o que a coluna
    /// significa: os mesmos seis bytes são um MAC em `ifPhysAddress` e o texto
    /// "br-lan" em `ifName`. Decidir isso aqui, por heurística de tamanho,
    /// transformava nomes de seis caracteres em hexadecimal.
    Bytes(Vec<u8>),
}
impl SnmpValue {
    #[must_use]
    pub fn number(&self) -> Option<u64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(value) => first_number(value),
            Self::Bytes(bytes) => first_number(&String::from_utf8_lossy(bytes)),
        }
    }
    /// Leitura textual: imprimível vira texto, binário vira hexadecimal.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Bytes(bytes) => decode_text(bytes),
        }
    }
    /// Leitura de endereço físico: seis bytes sempre saem como MAC.
    #[must_use]
    pub fn mac(&self) -> Option<String> {
        match self {
            Self::Bytes(bytes) if bytes.len() == 6 => Some(hex(bytes)),
            other => Some(other.text()).filter(|text| !text.is_empty()),
        }
    }
}
impl serde::Serialize for SnmpValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Number(value) => serializer.serialize_u64(*value),
            Self::Text(_) | Self::Bytes(_) => serializer.serialize_str(&self.text()),
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
    #[error("Configuração SNMP inválida: {0}")]
    InvalidConfig(String),
    #[error("Agente SNMP devolveu erro {0}")]
    Agent(u32),
}

pub struct SnmpClient {
    config: SnmpConfig,
    v3_context: OnceCell<V3Context>,
}
#[derive(Debug, Clone)]
struct V3Context {
    engine_id: Vec<u8>,
    engine_boots: u64,
    engine_time: u64,
}
impl SnmpClient {
    #[must_use]
    pub fn new(config: SnmpConfig) -> Self {
        Self {
            config,
            v3_context: OnceCell::new(),
        }
    }
    pub async fn get(
        &self,
        oids: &[&str],
    ) -> Result<BTreeMap<String, Option<SnmpValue>>, SnmpError> {
        match self.config.version {
            SnmpVersion::V1 => self.get_v1(oids).await,
            SnmpVersion::V2c => self.get_v2c(oids).await,
            SnmpVersion::V3 => self.get_v3(oids).await,
        }
    }
    async fn get_v2c(
        &self,
        oids: &[&str],
    ) -> Result<BTreeMap<String, Option<SnmpValue>>, SnmpError> {
        let request = pdu(i32::from(rand::random::<u16>()), oids)?;
        let response = self.exchange_v2c_get(request).await?;
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
    async fn get_v1(
        &self,
        oids: &[&str],
    ) -> Result<BTreeMap<String, Option<SnmpValue>>, SnmpError> {
        let request = pdu_v1(i32::from(rand::random::<u16>()), oids)?;
        let response = self.exchange_v1_get(request).await?;
        let mut values = oids
            .iter()
            .map(|oid| ((*oid).to_string(), None))
            .collect::<BTreeMap<_, _>>();
        if response.error_status != 0.into() {
            return Err(SnmpError::Agent(
                response
                    .error_status
                    .to_string()
                    .parse()
                    .unwrap_or_default(),
            ));
        }
        for bind in response.variable_bindings {
            values.insert(oid_string(&bind.name), value_v1(&bind.value));
        }
        Ok(values)
    }
    async fn get_v3(
        &self,
        oids: &[&str],
    ) -> Result<BTreeMap<String, Option<SnmpValue>>, SnmpError> {
        let request = pdu(i32::from(rand::random::<u16>()), oids)?;
        let response = self.exchange_v3_get(request).await?;
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
        match self.config.version {
            SnmpVersion::V1 => self.walk_v1(base_oid).await,
            SnmpVersion::V2c => self.walk_v2c(base_oid).await,
            SnmpVersion::V3 => self.walk_v3(base_oid).await,
        }
    }
    async fn walk_v2c(&self, base_oid: &str) -> Result<Vec<SnmpWalkEntry>, SnmpError> {
        let base = oid_parts(base_oid)?;
        let mut current = base.clone();
        let mut entries = Vec::new();
        for _ in 0..1_024 {
            let response = self
                .exchange_v2c_next(pdu_from_parts(i32::from(rand::random::<u16>()), &current))
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
    async fn walk_v1(&self, base_oid: &str) -> Result<Vec<SnmpWalkEntry>, SnmpError> {
        let base = oid_parts(base_oid)?;
        let mut current = base.clone();
        let mut entries = Vec::new();
        for _ in 0..1_024 {
            let response = self
                .exchange_v1_next(pdu_v1_from_parts(
                    i32::from(rand::random::<u16>()),
                    &current,
                ))
                .await?;
            if response.error_status != 0.into() {
                break;
            }
            let Some(bind) = response.variable_bindings.first() else {
                break;
            };
            let next: Vec<u32> = bind.name.iter().copied().collect();
            if !next.starts_with(&base) {
                break;
            }
            let Some(value) = value_v1(&bind.value) else {
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
    async fn walk_v3(&self, base_oid: &str) -> Result<Vec<SnmpWalkEntry>, SnmpError> {
        let base = oid_parts(base_oid)?;
        let mut current = base.clone();
        let mut entries = Vec::new();
        for _ in 0..1_024 {
            let response = self
                .exchange_v3_next(pdu_from_parts(i32::from(rand::random::<u16>()), &current))
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
    async fn exchange_v2c_get(&self, request: Pdu) -> Result<Pdu, SnmpError> {
        let bytes = rasn::ber::encode(&Message {
            version: 1.into(),
            community: self.config.community.as_bytes().to_vec().into(),
            data: GetRequest(request),
        })
        .map_err(|error| SnmpError::Encode(error.to_string()))?;
        self.exchange_v2c(bytes).await
    }
    async fn exchange_v2c_next(&self, request: Pdu) -> Result<Pdu, SnmpError> {
        let bytes = rasn::ber::encode(&Message {
            version: 1.into(),
            community: self.config.community.as_bytes().to_vec().into(),
            data: GetNextRequest(request),
        })
        .map_err(|error| SnmpError::Encode(error.to_string()))?;
        self.exchange_v2c(bytes).await
    }
    async fn exchange_v2c(&self, request: Vec<u8>) -> Result<Pdu, SnmpError> {
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
    async fn exchange_v3_get(&self, request: Pdu) -> Result<Pdu, SnmpError> {
        self.exchange_v3(v3::Pdus::GetRequest(v3::GetRequest(request)))
            .await
    }
    async fn exchange_v3_next(&self, request: Pdu) -> Result<Pdu, SnmpError> {
        self.exchange_v3(v3::Pdus::GetNextRequest(v3::GetNextRequest(request)))
            .await
    }
    async fn exchange_v3(&self, data: v3::Pdus) -> Result<Pdu, SnmpError> {
        if self.config.auth_key.is_some()
            || self.config.priv_key.is_some()
            || self.config.auth_protocol.is_some()
            || self.config.priv_protocol.is_some()
        {
            return Err(SnmpError::UnsupportedVersion("v3 com authPriv"));
        }
        let username = self
            .config
            .username
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                (!self.config.community.trim().is_empty()).then_some(self.config.community.as_str())
            })
            .ok_or_else(|| SnmpError::InvalidConfig("usuario v3 e obrigatorio".into()))?;
        let context = self
            .v3_context
            .get_or_try_init(|| async { self.discover_v3().await })
            .await?;
        let message = v3_message(data, context, username, true)?;
        let bytes =
            rasn::ber::encode(&message).map_err(|error| SnmpError::Encode(error.to_string()))?;
        let response = self.exchange_v3_bytes(bytes).await?;
        let message: v3::Message =
            rasn::ber::decode(&response).map_err(|error| SnmpError::Decode(error.to_string()))?;
        let v3::ScopedPduData::CleartextPdu(scoped) = message.scoped_data else {
            return Err(SnmpError::UnsupportedVersion("v3 com privacidade"));
        };
        if let v3::Pdus::Response(response) = scoped.data {
            Ok(response.0)
        } else {
            Err(SnmpError::Decode("PDU v3 nao e uma resposta".into()))
        }
    }
    async fn discover_v3(&self) -> Result<V3Context, SnmpError> {
        let request = pdu(i32::from(rand::random::<u16>()), &["1.3.6.1.2.1.1.3.0"])?;
        let empty = V3Context {
            engine_id: vec![],
            engine_boots: 0,
            engine_time: 0,
        };
        let message = v3_message(
            v3::Pdus::GetRequest(v3::GetRequest(request)),
            &empty,
            "",
            true,
        )?;
        let bytes =
            rasn::ber::encode(&message).map_err(|error| SnmpError::Encode(error.to_string()))?;
        let response = self.exchange_v3_bytes(bytes).await?;
        let response: v3::Message =
            rasn::ber::decode(&response).map_err(|error| SnmpError::Decode(error.to_string()))?;
        let security = response
            .decode_security_parameters::<v3::USMSecurityParameters>(rasn::Codec::Ber)
            .map_err(|error| SnmpError::Decode(error.to_string()))?;
        let engine_id = security.authoritative_engine_id.as_ref().to_vec();
        if engine_id.is_empty() {
            return Err(SnmpError::Decode("resposta v3 sem snmpEngineID".into()));
        }
        Ok(V3Context {
            engine_id,
            engine_boots: security
                .authoritative_engine_boots
                .to_string()
                .parse()
                .unwrap_or_default(),
            engine_time: security
                .authoritative_engine_time
                .to_string()
                .parse()
                .unwrap_or_default(),
        })
    }
    async fn exchange_v3_bytes(&self, request: Vec<u8>) -> Result<Vec<u8>, SnmpError> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|error| SnmpError::Network(error.to_string()))?;
        socket
            .connect((self.config.host.as_str(), self.config.port))
            .await
            .map_err(|error| SnmpError::Network(error.to_string()))?;
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
                Ok(Ok(read)) => return Ok(buffer[..read].to_vec()),
                Ok(Err(error)) => return Err(SnmpError::Network(error.to_string())),
                Err(_) => continue,
            }
        }
        Err(SnmpError::Timeout)
    }
    async fn exchange_v1_get(
        &self,
        request: rasn_snmp::v1::Pdu,
    ) -> Result<rasn_snmp::v1::Pdu, SnmpError> {
        let bytes = rasn::ber::encode(&rasn_snmp::v1::Message {
            version: 0.into(),
            community: self.config.community.as_bytes().to_vec().into(),
            data: rasn_snmp::v1::GetRequest(request),
        })
        .map_err(|error| SnmpError::Encode(error.to_string()))?;
        self.exchange_v1(bytes).await
    }
    async fn exchange_v1_next(
        &self,
        request: rasn_snmp::v1::Pdu,
    ) -> Result<rasn_snmp::v1::Pdu, SnmpError> {
        let bytes = rasn::ber::encode(&rasn_snmp::v1::Message {
            version: 0.into(),
            community: self.config.community.as_bytes().to_vec().into(),
            data: rasn_snmp::v1::GetNextRequest(request),
        })
        .map_err(|error| SnmpError::Encode(error.to_string()))?;
        self.exchange_v1(bytes).await
    }
    async fn exchange_v1(&self, request: Vec<u8>) -> Result<rasn_snmp::v1::Pdu, SnmpError> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|error| SnmpError::Network(error.to_string()))?;
        socket
            .connect((self.config.host.as_str(), self.config.port))
            .await
            .map_err(|error| SnmpError::Network(error.to_string()))?;
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
                    let message: rasn_snmp::v1::Message<rasn_snmp::v1::Pdus> =
                        rasn::ber::decode(&buffer[..read])
                            .map_err(|error| SnmpError::Decode(error.to_string()))?;
                    if let rasn_snmp::v1::Pdus::GetResponse(response) = message.data {
                        return Ok(response.0);
                    }
                    return Err(SnmpError::Decode("PDU v1 nao e uma resposta".into()));
                }
                Ok(Err(error)) => return Err(SnmpError::Network(error.to_string())),
                Err(_) => continue,
            }
        }
        Err(SnmpError::Timeout)
    }
}
fn v3_message(
    data: v3::Pdus,
    context: &V3Context,
    username: &str,
    reportable: bool,
) -> Result<v3::Message, SnmpError> {
    let mut message = v3::Message {
        version: 3.into(),
        global_data: v3::HeaderData {
            message_id: i32::from(rand::random::<u16>()).into(),
            max_size: 65_507.into(),
            flags: vec![if reportable { 0b0000_0100 } else { 0 }].into(),
            security_model: 3.into(),
        },
        security_parameters: vec![].into(),
        scoped_data: v3::ScopedPduData::CleartextPdu(v3::ScopedPdu {
            engine_id: context.engine_id.clone().into(),
            name: vec![].into(),
            data,
        }),
    };
    message
        .encode_security_parameters(
            rasn::Codec::Ber,
            &v3::USMSecurityParameters {
                authoritative_engine_id: context.engine_id.clone().into(),
                authoritative_engine_boots: context.engine_boots.into(),
                authoritative_engine_time: context.engine_time.into(),
                user_name: username.as_bytes().to_vec().into(),
                authentication_parameters: vec![].into(),
                privacy_parameters: vec![].into(),
            },
        )
        .map_err(|error| SnmpError::Encode(error.to_string()))?;
    Ok(message)
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
fn pdu_v1(request_id: i32, oids: &[&str]) -> Result<rasn_snmp::v1::Pdu, SnmpError> {
    Ok(rasn_snmp::v1::Pdu {
        request_id: request_id.into(),
        error_status: 0.into(),
        error_index: 0.into(),
        variable_bindings: oids
            .iter()
            .map(|oid| {
                oid_parts(oid).map(|parts| rasn_snmp::v1::VarBind {
                    name: ObjectIdentifier::new_unchecked(parts.into()),
                    value: V1ObjectSyntax::Simple(V1SimpleSyntax::Empty),
                })
            })
            .collect::<Result<_, _>>()?,
    })
}
fn pdu_v1_from_parts(request_id: i32, oid: &[u32]) -> rasn_snmp::v1::Pdu {
    rasn_snmp::v1::Pdu {
        request_id: request_id.into(),
        error_status: 0.into(),
        error_index: 0.into(),
        variable_bindings: vec![rasn_snmp::v1::VarBind {
            name: ObjectIdentifier::new_unchecked(oid.to_vec().into()),
            value: V1ObjectSyntax::Simple(V1SimpleSyntax::Empty),
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
        VarBindValue::Value(value) => value_v2(value),
        _ => None,
    }
}
fn value_v1(value: &V1ObjectSyntax) -> Option<SnmpValue> {
    match value {
        V1ObjectSyntax::Simple(V1SimpleSyntax::Number(value)) => integer(value.to_string()),
        V1ObjectSyntax::Simple(V1SimpleSyntax::String(value)) => octet_string(value.as_ref()),
        V1ObjectSyntax::Simple(V1SimpleSyntax::Object(value)) => {
            Some(SnmpValue::Text(oid_string(value)))
        }
        V1ObjectSyntax::Simple(V1SimpleSyntax::Empty) => None,
        V1ObjectSyntax::ApplicationWide(V1ApplicationSyntax::Counter(value)) => {
            Some(SnmpValue::Number(u64::from(value.0)))
        }
        V1ObjectSyntax::ApplicationWide(V1ApplicationSyntax::Gauge(value)) => {
            Some(SnmpValue::Number(u64::from(value.0)))
        }
        V1ObjectSyntax::ApplicationWide(V1ApplicationSyntax::Ticks(value)) => {
            Some(SnmpValue::Number(u64::from(value.0)))
        }
        V1ObjectSyntax::ApplicationWide(V1ApplicationSyntax::Address(value)) => {
            let rasn_smi::v1::NetworkAddress::Internet(ip) = value;
            Some(SnmpValue::Text(ipv4(ip.0.as_ref())))
        }
        V1ObjectSyntax::ApplicationWide(V1ApplicationSyntax::Arbitrary(value)) => {
            octet_string(value.as_ref())
        }
    }
}
fn value_v2(value: &V2ObjectSyntax) -> Option<SnmpValue> {
    match value {
        V2ObjectSyntax::Simple(V2SimpleSyntax::Integer(value)) => integer(value.to_string()),
        V2ObjectSyntax::Simple(V2SimpleSyntax::String(value)) => octet_string(value.as_ref()),
        V2ObjectSyntax::Simple(V2SimpleSyntax::ObjectId(value)) => {
            Some(SnmpValue::Text(oid_string(value)))
        }
        V2ObjectSyntax::ApplicationWide(V2ApplicationSyntax::Counter(value)) => {
            Some(SnmpValue::Number(u64::from(value.0)))
        }
        V2ObjectSyntax::ApplicationWide(V2ApplicationSyntax::Ticks(value)) => {
            Some(SnmpValue::Number(u64::from(value.0)))
        }
        V2ObjectSyntax::ApplicationWide(V2ApplicationSyntax::BigCounter(value)) => {
            Some(SnmpValue::Number(value.0))
        }
        V2ObjectSyntax::ApplicationWide(V2ApplicationSyntax::Unsigned(value)) => {
            Some(SnmpValue::Number(u64::from(value.0)))
        }
        V2ObjectSyntax::ApplicationWide(V2ApplicationSyntax::Address(value)) => {
            Some(SnmpValue::Text(ipv4(value.0.as_ref())))
        }
        V2ObjectSyntax::ApplicationWide(V2ApplicationSyntax::Arbitrary(value)) => {
            octet_string(value.as_ref())
        }
    }
}
fn integer(value: String) -> Option<SnmpValue> {
    value.parse::<u64>().ok().map(SnmpValue::Number)
}
fn octet_string(bytes: &[u8]) -> Option<SnmpValue> {
    Some(SnmpValue::Bytes(bytes.to_vec()))
}
fn decode_text(bytes: &[u8]) -> String {
    if bytes
        .iter()
        .all(|byte| byte.is_ascii_graphic() || byte.is_ascii_whitespace())
    {
        return String::from_utf8_lossy(bytes).trim().to_string();
    }
    hex(bytes)
}
fn ipv4(bytes: &[u8]) -> String {
    if let [a, b, c, d] = bytes {
        return format!("{a}.{b}.{c}.{d}");
    }
    hex(bytes)
}
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}
fn first_number(raw: &str) -> Option<u64> {
    raw.split(|char: char| !char.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codifica_consulta_snmp_v1_com_versao_zero() {
        let message = rasn_snmp::v1::Message {
            version: 0.into(),
            community: b"public".to_vec().into(),
            data: rasn_snmp::v1::GetRequest(pdu_v1(42, &[OID_SYS_UPTIME]).unwrap()),
        };
        let encoded = rasn::ber::encode(&message).unwrap();
        let decoded: rasn_snmp::v1::Message<rasn_snmp::v1::Pdus> =
            rasn::ber::decode(&encoded).unwrap();

        assert_eq!(decoded.version, 0.into());
        assert!(matches!(decoded.data, rasn_snmp::v1::Pdus::GetRequest(_)));
    }

    #[test]
    fn endereco_fisico_sai_em_hexadecimal_e_texto_sai_limpo() {
        let mac = SnmpValue::Bytes(vec![0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]);
        assert_eq!(mac.mac(), Some("00:1a:2b:3c:4d:5e".into()));
        assert_eq!(
            SnmpValue::Bytes(b" router-01 ".to_vec()).text(),
            "router-01"
        );
        // Binário não imprimível continua legível como hexadecimal.
        assert_eq!(SnmpValue::Bytes(vec![0x00, 0xff]).text(), "00:ff");
    }

    /// Seis bytes imprimíveis são um nome de interface, não um MAC: a heurística
    /// antiga por tamanho devolvia `62:72:2d:6c:61:6e` no lugar de `br-lan`.
    #[test]
    fn nome_de_seis_caracteres_nao_vira_hexadecimal() {
        let value = SnmpValue::Bytes(b"br-lan".to_vec());
        assert_eq!(value.text(), "br-lan");
        assert_eq!(value.mac(), Some("62:72:2d:6c:61:6e".into()));
    }

    #[test]
    fn endereco_ip_sai_em_notacao_decimal() {
        assert_eq!(ipv4(&[192, 168, 1, 10]), "192.168.1.10");
    }

    #[test]
    fn codifica_consulta_snmp_v3_sem_autenticacao() {
        let context = V3Context {
            engine_id: vec![0x80, 0x00, 0x1f, 0x88, 0x80],
            engine_boots: 2,
            engine_time: 30,
        };
        let message = v3_message(
            v3::Pdus::GetRequest(v3::GetRequest(pdu(99, &[OID_SYS_UPTIME]).unwrap())),
            &context,
            "monitor",
            true,
        )
        .unwrap();
        let encoded = rasn::ber::encode(&message).unwrap();
        let decoded: v3::Message = rasn::ber::decode(&encoded).unwrap();
        let security = match decoded
            .decode_security_parameters::<v3::USMSecurityParameters>(rasn::Codec::Ber)
        {
            Ok(security) => security,
            Err(error) => panic!("falha ao decodificar USM: {error}"),
        };

        assert_eq!(decoded.version, 3.into());
        assert_eq!(security.user_name.as_ref(), b"monitor");
        assert_eq!(security.authoritative_engine_id.as_ref(), context.engine_id);
    }

    const OID_SYS_UPTIME: &str = "1.3.6.1.2.1.1.3.0";
}
