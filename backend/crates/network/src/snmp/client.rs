use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
    str::FromStr,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use async_snmp::{
    Auth, AuthProtocol, Client, ClientBuilder, EngineCache, Error as AsyncSnmpError, MasterKeys,
    Oid, OidOrdering, PrivProtocol, Retry, UdpTransport, Value, WalkMode,
};
use futures::TryStreamExt;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::OnceCell;

thread_local! {
    /// Um transporte UDP por thread do runtime (IPv4 e IPv6), não um por
    /// processo. O `UdpTransport` guarda a tarefa de recepção no runtime que o
    /// criou e morre com ele: num `static`, o primeiro runtime a encerrar —
    /// cada `#[tokio::test]` tem o seu — derrubava o SNMP de todos os outros
    /// com "Tokio context … being shutdown". No servidor isso dá um socket por
    /// *worker* do runtime: poucos, e reaproveitados por todas as consultas.
    static UDP: std::cell::RefCell<[Option<UdpTransport>; 2]> =
        const { std::cell::RefCell::new([None, None]) };
}
static ENGINE_CACHE: OnceLock<Arc<EngineCache>> = OnceLock::new();
static MASTER_KEYS: OnceLock<Mutex<HashMap<[u8; 32], MasterKeys>>> = OnceLock::new();

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

use netmonitor_core::devices::snmp_credentials::SnmpCredentials;
/// A versão é a das credenciais do núcleo: o perfil grava, o cliente usa.
pub use netmonitor_core::devices::snmp_credentials::SnmpVersion;

#[derive(Clone)]
pub struct SnmpConfig {
    pub host: String,
    pub version: SnmpVersion,
    pub community: String,
    pub username: Option<String>,
    pub auth_protocol: Option<String>,
    pub auth_key: Option<String>,
    pub priv_protocol: Option<String>,
    pub priv_key: Option<String>,
    /// `contextName` do SNMPv3 (vazio é o contexto padrão do agente).
    pub context_name: Option<String>,
    pub port: u16,
    pub timeout_ms: u64,
}

/// Sem segredo no `Debug`: a configuração aparece em `tracing` de erro.
impl std::fmt::Debug for SnmpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnmpConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("version", &self.version)
            .field("username", &self.username)
            .field("auth_protocol", &self.auth_protocol)
            .field("priv_protocol", &self.priv_protocol)
            .field("timeout_ms", &self.timeout_ms)
            .finish_non_exhaustive()
    }
}

impl SnmpConfig {
    #[must_use]
    pub fn v2c(host: impl Into<String>, community: impl Into<String>, port: u16) -> Self {
        Self::from_credentials(
            host,
            port,
            &SnmpCredentials::community(SnmpVersion::V2c, community),
        )
    }

    /// A consulta a `host:port` com uma credencial resolvida
    /// (`devices::snmp_credentials`) — o único ponto em que perfil vira
    /// configuração do cliente.
    #[must_use]
    pub fn from_credentials(
        host: impl Into<String>,
        port: u16,
        credentials: &SnmpCredentials,
    ) -> Self {
        Self {
            host: host.into(),
            version: credentials.version,
            community: credentials.community.clone(),
            username: credentials.username.clone(),
            auth_protocol: credentials.auth_protocol.clone(),
            auth_key: credentials.auth_key.clone(),
            priv_protocol: credentials.priv_protocol.clone(),
            priv_key: credentials.priv_key.clone(),
            context_name: credentials.context_name.clone(),
            port,
            timeout_ms: 4_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SnmpValue {
    Number(u64),
    Text(String),
    /// `OCTET STRING` cru. A interpretação depende da coluna consultada.
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

    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Bytes(bytes) => decode_text(bytes),
        }
    }

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
    #[error("Versão SNMP {0} não suportada")]
    UnsupportedVersion(&'static str),
    #[error("Agente SNMPv3 recusou a consulta: {0}")]
    Usm(&'static str),
    #[error("Configuração SNMP inválida: {0}")]
    InvalidConfig(String),
    #[error("Agente SNMP devolveu erro {0}")]
    Agent(u32),
}

/// Cliente de domínio. Transporte, retries e USM ficam encapsulados; os
/// coletores não dependem de detalhes de rede ou ASN.1.
pub struct SnmpClient {
    config: SnmpConfig,
    client: OnceCell<Client>,
    compatible_client: OnceCell<Client>,
}

impl SnmpClient {
    #[must_use]
    pub fn new(config: SnmpConfig) -> Self {
        Self {
            config,
            client: OnceCell::new(),
            compatible_client: OnceCell::new(),
        }
    }

    pub async fn get(
        &self,
        oids: &[&str],
    ) -> Result<BTreeMap<String, Option<SnmpValue>>, SnmpError> {
        let parsed = oids
            .iter()
            .map(|oid| parse_oid(oid))
            .collect::<Result<Vec<_>, _>>()?;
        let client = self.client().await?;
        let response = client.get_many(&parsed).await.map_err(map_error)?;
        let mut values = oids
            .iter()
            .map(|oid| ((*oid).to_string(), None))
            .collect::<BTreeMap<_, _>>();

        for var_bind in response {
            values.insert(var_bind.oid.to_string(), map_value(var_bind.value));
        }
        Ok(values)
    }

    pub async fn walk(&self, base_oid: &str) -> Result<Vec<SnmpWalkEntry>, SnmpError> {
        let oid = parse_oid(base_oid)?;
        let bulk_key = bulk_cache_key(&self.config.host, self.config.port);
        if self.config.version != SnmpVersion::V1 && bulk_known_broken(&bulk_key) {
            return Ok(walk_entries(self.compatible_walk(oid).await?));
        }
        let client = self.client().await?;
        let primary = match client.walk(oid.clone()) {
            Ok(stream) => stream.try_collect::<Vec<_>>().await,
            Err(error) => Err(error),
        };

        let bindings = match primary {
            Ok(bindings) if bindings.is_empty() && self.config.version != SnmpVersion::V1 => {
                tracing::debug!(
                    target = %self.config.host,
                    oid = base_oid,
                    "SNMP GETBULK vazio; confirmando com GETNEXT compatível"
                );
                self.compatible_walk(oid).await?
            }
            Ok(bindings) => bindings,
            // GETBULK descartado pelo equipamento chega aqui como Timeout sem
            // nenhum binding: o GETNEXT é a única forma de coletá-lo. Se ele
            // responder, o host fica memorizado como "sem GETBULK" para os
            // próximos walks não pagarem o timeout de novo.
            Err(primary_error)
                if self.config.version != SnmpVersion::V1
                    && matches!(primary_error.as_ref(), AsyncSnmpError::Timeout { .. }) =>
            {
                tracing::warn!(
                    target = %self.config.host,
                    oid = base_oid,
                    "SNMP GETBULK sem resposta; repetindo walk com GETNEXT compatível"
                );
                let bindings = self.compatible_walk(oid).await?;
                mark_bulk_broken(&bulk_key);
                bindings
            }
            Err(primary_error)
                if self.config.version != SnmpVersion::V1
                    && !matches!(primary_error.as_ref(), AsyncSnmpError::Auth { .. }) =>
            {
                tracing::warn!(
                    target = %self.config.host,
                    oid = base_oid,
                    error = %primary_error,
                    "SNMP GETBULK falhou; repetindo walk com GETNEXT compatível"
                );
                self.compatible_walk(oid).await?
            }
            Err(error) => return Err(map_error(error)),
        };

        Ok(walk_entries(bindings))
    }

    async fn client(&self) -> Result<&Client, SnmpError> {
        self.client
            .get_or_try_init(|| self.build_client(WalkMode::Auto, OidOrdering::Strict))
            .await
    }

    async fn compatible_walk(&self, oid: Oid) -> Result<Vec<async_snmp::VarBind>, SnmpError> {
        let compatible = self
            .compatible_client
            .get_or_try_init(|| {
                self.build_client(WalkMode::GetNext, OidOrdering::AllowNonIncreasing)
            })
            .await?;
        compatible
            .walk_getnext(oid)
            .try_collect::<Vec<_>>()
            .await
            .map_err(map_error)
    }

    async fn build_client(
        &self,
        walk_mode: WalkMode,
        oid_ordering: OidOrdering,
    ) -> Result<Client, SnmpError> {
        let auth = auth(&self.config)?;
        let target = resolve_target(&self.config.host, self.config.port).await?;
        let timeout = Duration::from_millis(self.config.timeout_ms.max(100));
        let retry = Retry::exponential(2)
            .initial_delay(Duration::from_millis(100))
            .max_delay(Duration::from_secs(1))
            .jitter(0.25)
            .build();
        let builder = ClientBuilder::new(target, auth)
            .timeout(timeout)
            .retry(retry)
            .max_oids_per_request(10)
            .max_repetitions(20)
            .walk_mode(walk_mode)
            .oid_ordering(oid_ordering)
            .max_walk_results(20_000)
            .engine_cache(engine_cache());
        let transport = shared_transport(target.ip()).await?;
        builder.build_with(&transport).await.map_err(map_error)
    }
}

fn walk_entries(bindings: Vec<async_snmp::VarBind>) -> Vec<SnmpWalkEntry> {
    bindings
        .into_iter()
        .filter_map(|var_bind| {
            map_value(var_bind.value).map(|value| SnmpWalkEntry {
                oid: var_bind.oid.to_string(),
                value,
            })
        })
        .collect()
}

/// Hosts cujo GETBULK não responde (mas o GETNEXT sim), com o instante da
/// constatação. Limitado e com validade: o firmware pode ser atualizado.
static BULK_BROKEN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
const BULK_BROKEN_MAX: usize = 1_024;
const BULK_BROKEN_TTL: Duration = Duration::from_secs(60 * 60);

fn bulk_cache_key(host: &str, port: u16) -> String {
    format!("{host}:{port}")
}

fn bulk_known_broken(key: &str) -> bool {
    let Some(cache) = BULK_BROKEN.get() else {
        return false;
    };
    let Ok(mut guard) = cache.lock() else {
        return false;
    };
    match guard.get(key) {
        Some(since) if since.elapsed() < BULK_BROKEN_TTL => true,
        Some(_) => {
            guard.remove(key);
            false
        }
        None => false,
    }
}

fn mark_bulk_broken(key: &str) {
    let cache = BULK_BROKEN.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut guard) = cache.lock() else {
        return;
    };
    if guard.len() >= BULK_BROKEN_MAX && !guard.contains_key(key) {
        guard.retain(|_, since| since.elapsed() < BULK_BROKEN_TTL);
        if guard.len() >= BULK_BROKEN_MAX {
            if let Some(oldest) = guard
                .iter()
                .min_by_key(|(_, since)| **since)
                .map(|(host, _)| host.clone())
            {
                guard.remove(&oldest);
            }
        }
    }
    guard.insert(key.to_string(), Instant::now());
}

fn auth(config: &SnmpConfig) -> Result<Auth, SnmpError> {
    match config.version {
        SnmpVersion::V1 => Ok(Auth::v1(config.community.clone())),
        SnmpVersion::V2c => Ok(Auth::v2c(config.community.clone())),
        SnmpVersion::V3 => {
            let username = config
                .username
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    SnmpError::InvalidConfig("usuário SNMPv3 obrigatório".to_string())
                })?;
            let mut usm = Auth::usm(username);
            if let Some(protocol) = config.auth_protocol.as_deref() {
                let auth_protocol = AuthProtocol::from_str(protocol)
                    .map_err(|error| SnmpError::InvalidConfig(error.to_string()))?;
                let auth_key = config.auth_key.as_deref().ok_or_else(|| {
                    SnmpError::InvalidConfig("senha de autenticação SNMPv3 obrigatória".to_string())
                })?;
                let priv_protocol = config
                    .priv_protocol
                    .as_deref()
                    .map(PrivProtocol::from_str)
                    .transpose()
                    .map_err(|error| SnmpError::InvalidConfig(error.to_string()))?;
                let keys = master_keys(config, auth_protocol, auth_key, priv_protocol)?;
                usm = usm.with_master_keys(keys);
            } else if config.priv_protocol.is_some() || config.priv_key.is_some() {
                return Err(SnmpError::InvalidConfig(
                    "privacidade SNMPv3 exige autenticação".to_string(),
                ));
            }
            if let Some(context) = config
                .context_name
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                usm = usm.context_name(context);
            }
            Ok(usm.into())
        }
    }
}

const MASTER_KEYS_CACHE_MAX: usize = 512;

fn master_keys(
    config: &SnmpConfig,
    auth_protocol: AuthProtocol,
    auth_key: &str,
    priv_protocol: Option<PrivProtocol>,
) -> Result<MasterKeys, SnmpError> {
    let mut hasher = Sha256::new();
    hasher.update(config.auth_protocol.as_deref().unwrap_or_default());
    hasher.update([0]);
    hasher.update(auth_key);
    hasher.update([0]);
    hasher.update(config.priv_protocol.as_deref().unwrap_or_default());
    hasher.update([0]);
    hasher.update(config.priv_key.as_deref().unwrap_or_default());
    let cache_key: [u8; 32] = hasher.finalize().into();
    let cache = MASTER_KEYS.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let guard = cache.lock().map_err(|_| {
            SnmpError::InvalidConfig("cache de chaves SNMP indisponível".to_string())
        })?;
        if let Some(keys) = guard.get(&cache_key) {
            return Ok(keys.clone());
        }
    }

    // Deriva chaves (1 MB PBKDF) fora do lock para não serializar outros threads.
    let mut keys = MasterKeys::new(auth_protocol, auth_key.as_bytes())
        .map_err(|error| SnmpError::InvalidConfig(error.to_string()))?;
    if let Some(protocol) = priv_protocol {
        let priv_key = config.priv_key.as_deref().ok_or_else(|| {
            SnmpError::InvalidConfig("senha de privacidade SNMPv3 obrigatória".to_string())
        })?;
        keys = if priv_key == auth_key {
            keys.with_privacy_same_password(protocol)
        } else {
            keys.with_privacy(protocol, priv_key.as_bytes())
                .map_err(|error| SnmpError::InvalidConfig(error.to_string()))?
        };
    }
    let mut guard = cache
        .lock()
        .map_err(|_| SnmpError::InvalidConfig("cache de chaves SNMP indisponível".to_string()))?;
    if guard.len() >= MASTER_KEYS_CACHE_MAX {
        guard.clear();
    }
    guard.insert(cache_key, keys.clone());
    Ok(keys)
}

async fn shared_transport(ip: IpAddr) -> Result<UdpTransport, SnmpError> {
    let (slot, bind) = if ip.is_ipv6() {
        (1, "[::]:0")
    } else {
        (0, "0.0.0.0:0")
    };
    if let Some(transport) = UDP.with(|cell| cell.borrow()[slot].clone()) {
        return Ok(transport);
    }
    let transport = UdpTransport::bind(bind).await.map_err(map_error)?;
    // Entre o `bind` e aqui a tarefa pode ter mudado de thread: o transporte
    // fica na thread em que ela retomou, e é tão válido quanto — o runtime é
    // o mesmo.
    UDP.with(|cell| cell.borrow_mut()[slot] = Some(transport.clone()));
    Ok(transport)
}

async fn resolve_target(host: &str, port: u16) -> Result<std::net::SocketAddr, SnmpError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(std::net::SocketAddr::new(ip, port));
    }
    match tokio::time::timeout(LOOKUP_TIMEOUT, tokio::net::lookup_host((host, port))).await {
        Ok(Ok(mut addresses)) => addresses
            .next()
            .ok_or_else(|| SnmpError::Network(format!("host SNMP não resolvido: {host}"))),
        Ok(Err(error)) => Err(SnmpError::Network(error.to_string())),
        Err(_) => Err(SnmpError::Network(format!(
            "tempo esgotado ao resolver host SNMP {host} (timeout de {LOOKUP_TIMEOUT:?})"
        ))),
    }
}

fn engine_cache() -> Arc<EngineCache> {
    ENGINE_CACHE
        .get_or_init(|| Arc::new(EngineCache::new().with_max_capacity(4_096)))
        .clone()
}

#[cfg(test)]
fn target(host: &str, port: u16) -> String {
    if host.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn parse_oid(value: &str) -> Result<Oid, SnmpError> {
    Oid::from_str(value.trim_start_matches('.')).map_err(|error| SnmpError::Oid(error.to_string()))
}

fn map_error(error: impl AsRef<AsyncSnmpError>) -> SnmpError {
    match error.as_ref() {
        AsyncSnmpError::Timeout { .. } => SnmpError::Timeout,
        AsyncSnmpError::InvalidOid(value) => SnmpError::Oid(value.to_string()),
        AsyncSnmpError::Config(value) => SnmpError::InvalidConfig(value.to_string()),
        AsyncSnmpError::Auth { .. } => SnmpError::Usm("autenticação ou autorização inválida"),
        other => SnmpError::Network(other.to_string()),
    }
}

fn map_value(value: Value) -> Option<SnmpValue> {
    match value {
        Value::Integer(value) if value >= 0 => Some(SnmpValue::Number(value as u64)),
        Value::Integer(value) => Some(SnmpValue::Text(value.to_string())),
        Value::OctetString(value) | Value::Opaque(value) | Value::Nsap(value) => {
            Some(SnmpValue::Bytes(value.to_vec()))
        }
        Value::ObjectIdentifier(value) => Some(SnmpValue::Text(value.to_string())),
        Value::IpAddress(value) => Some(SnmpValue::Text(IpAddr::from(value).to_string())),
        Value::Counter32(value)
        | Value::Gauge32(value)
        | Value::UInteger32(value)
        | Value::TimeTicks(value) => Some(SnmpValue::Number(u64::from(value))),
        Value::Counter64(value) => Some(SnmpValue::Number(value)),
        Value::Unknown { data, .. } => Some(SnmpValue::Bytes(data.to_vec())),
        Value::Null | Value::NoSuchObject | Value::NoSuchInstance | Value::EndOfMibView => None,
        _ => None,
    }
}

fn first_number(value: &str) -> Option<u64> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse()
        .ok()
}

fn decode_text(bytes: &[u8]) -> String {
    if bytes
        .iter()
        .all(|byte| byte.is_ascii_graphic() || byte.is_ascii_whitespace())
    {
        String::from_utf8_lossy(bytes).trim().to_string()
    } else {
        hex(bytes)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_snmp::{
        oid, BoxFuture, GetNextResult, GetResult, HandlerResult, MibHandler, RequestContext,
        VarBind,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio_util::sync::CancellationToken;

    struct TestMib;

    impl MibHandler for TestMib {
        fn get<'a>(
            &'a self,
            _context: &'a RequestContext,
            requested: &'a Oid,
        ) -> BoxFuture<'a, HandlerResult<GetResult>> {
            Box::pin(async move {
                if requested == &oid!(1, 3, 6, 1, 2, 1, 1, 1, 0) {
                    Ok(GetResult::Value(Value::OctetString("fixture".into())))
                } else {
                    Ok(GetResult::NoSuchObject)
                }
            })
        }

        fn get_next<'a>(
            &'a self,
            _context: &'a RequestContext,
            requested: &'a Oid,
        ) -> BoxFuture<'a, HandlerResult<GetNextResult>> {
            Box::pin(async move {
                let sys_descr = oid!(1, 3, 6, 1, 2, 1, 1, 1, 0);
                if requested < &sys_descr {
                    Ok(GetNextResult::Value(VarBind::new(
                        sys_descr,
                        Value::OctetString("fixture".into()),
                    )))
                } else {
                    Ok(GetNextResult::EndOfMibView)
                }
            })
        }
    }

    struct FaultyOnceMib(AtomicBool);

    impl MibHandler for FaultyOnceMib {
        fn get<'a>(
            &'a self,
            _context: &'a RequestContext,
            _requested: &'a Oid,
        ) -> BoxFuture<'a, HandlerResult<GetResult>> {
            Box::pin(async { Ok(GetResult::NoSuchObject) })
        }

        fn get_next<'a>(
            &'a self,
            _context: &'a RequestContext,
            requested: &'a Oid,
        ) -> BoxFuture<'a, HandlerResult<GetNextResult>> {
            Box::pin(async move {
                if !self.0.swap(true, Ordering::SeqCst) {
                    return Ok(GetNextResult::Value(VarBind::new(
                        requested.clone(),
                        Value::Integer(1),
                    )));
                }
                let sys_descr = oid!(1, 3, 6, 1, 2, 1, 1, 1, 0);
                if requested < &sys_descr {
                    Ok(GetNextResult::Value(VarBind::new(
                        sys_descr,
                        Value::OctetString("fallback".into()),
                    )))
                } else {
                    Ok(GetNextResult::EndOfMibView)
                }
            })
        }
    }

    struct DelayedOnceMib(AtomicBool);

    impl MibHandler for DelayedOnceMib {
        fn get<'a>(
            &'a self,
            _context: &'a RequestContext,
            requested: &'a Oid,
        ) -> BoxFuture<'a, HandlerResult<GetResult>> {
            Box::pin(async move {
                if !self.0.swap(true, Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
                if requested == &oid!(1, 3, 6, 1, 2, 1, 1, 1, 0) {
                    Ok(GetResult::Value(Value::OctetString("retry".into())))
                } else {
                    Ok(GetResult::NoSuchObject)
                }
            })
        }

        fn get_next<'a>(
            &'a self,
            _context: &'a RequestContext,
            _requested: &'a Oid,
        ) -> BoxFuture<'a, HandlerResult<GetNextResult>> {
            Box::pin(async { Ok(GetNextResult::EndOfMibView) })
        }
    }

    #[test]
    fn memoriza_host_sem_getbulk_com_limite() {
        let key = bulk_cache_key("192.0.2.77", 1161);
        assert!(!bulk_known_broken(&key));
        mark_bulk_broken(&key);
        assert!(bulk_known_broken(&key));
        assert!(!bulk_known_broken(&bulk_cache_key("192.0.2.77", 161)));
        for index in 0..(BULK_BROKEN_MAX + 10) {
            mark_bulk_broken(&bulk_cache_key("198.51.100.1", index as u16));
        }
        let len = BULK_BROKEN.get().unwrap().lock().unwrap().len();
        assert!(len <= BULK_BROKEN_MAX);
    }

    #[test]
    fn parses_versions() {
        assert_eq!(SnmpVersion::parse("1"), Some(SnmpVersion::V1));
        assert_eq!(SnmpVersion::parse("v2c"), Some(SnmpVersion::V2c));
        assert_eq!(SnmpVersion::parse("V3"), Some(SnmpVersion::V3));
        assert_eq!(SnmpVersion::parse("4"), None);
    }

    #[test]
    fn credencial_vira_configuracao_sem_vazar_segredo() {
        let credencial = SnmpCredentials {
            version: SnmpVersion::V3,
            community: String::new(),
            username: Some("monitor".into()),
            auth_protocol: Some("sha".into()),
            auth_key: Some("senha-auth".into()),
            priv_protocol: Some("aes128".into()),
            priv_key: Some("senha-priv".into()),
            context_name: Some("vlan10".into()),
        };
        let config = SnmpConfig::from_credentials("10.0.0.1", 1161, &credencial);
        assert_eq!(config.version, SnmpVersion::V3);
        assert_eq!(config.port, 1161);
        assert_eq!(config.username.as_deref(), Some("monitor"));
        assert_eq!(config.priv_key.as_deref(), Some("senha-priv"));
        assert_eq!(config.context_name.as_deref(), Some("vlan10"));
        assert!(auth(&config).is_ok());
        let texto = format!("{config:?}");
        assert!(!texto.contains("senha-auth") && !texto.contains("senha-priv"));

        let v1 = SnmpConfig::from_credentials(
            "10.0.0.1",
            161,
            &SnmpCredentials::community(SnmpVersion::V1, "segredo"),
        );
        assert_eq!(v1.version, SnmpVersion::V1);
        assert_eq!(v1.community, "segredo");
        assert!(!format!("{v1:?}").contains("segredo"));
    }

    #[test]
    fn preserves_binary_and_text_octets() {
        let mac = SnmpValue::Bytes(vec![0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]);
        assert_eq!(mac.mac().as_deref(), Some("00:1a:2b:3c:4d:5e"));
        assert_eq!(
            SnmpValue::Bytes(b" router-01 ".to_vec()).text(),
            "router-01"
        );
        assert_eq!(SnmpValue::Bytes(vec![0x00, 0xff]).text(), "00:ff");
    }

    #[test]
    fn maps_library_values_without_losing_types() {
        assert_eq!(map_value(Value::Counter64(9)), Some(SnmpValue::Number(9)));
        assert_eq!(
            map_value(Value::IpAddress([10, 8, 0, 1])),
            Some(SnmpValue::Text("10.8.0.1".to_string()))
        );
        assert_eq!(map_value(Value::NoSuchObject), None);
    }

    #[test]
    fn formats_ipv6_target_with_brackets() {
        assert_eq!(target("2001:db8::1", 161), "[2001:db8::1]:161");
        assert_eq!(target("router.local", 161), "router.local:161");
    }

    #[tokio::test]
    async fn resolve_target_ip_literal_nao_consulta_dns() {
        assert_eq!(
            resolve_target("192.0.2.10", 161).await.unwrap(),
            std::net::SocketAddr::new("192.0.2.10".parse().unwrap(), 161)
        );
        assert_eq!(
            resolve_target("2001:db8::1", 161).await.unwrap(),
            std::net::SocketAddr::new("2001:db8::1".parse().unwrap(), 161)
        );
    }

    #[tokio::test]
    async fn consulta_simulador_real_em_v1_v2c_e_v3() {
        let cancel = CancellationToken::new();
        let agent = async_snmp::Agent::builder()
            .bind("127.0.0.1:0")
            .community(b"public")
            .usm_user("scanner", |user| {
                user.auth(AuthProtocol::Sha256, "auth-password")
                    .privacy(PrivProtocol::Aes128, "priv-password")
            })
            .usm_user("leitor", |user| {
                user.auth(AuthProtocol::Sha256, "auth-password")
            })
            .handler(oid!(1, 3, 6, 1, 2, 1, 1), Arc::new(TestMib))
            .cancel(cancel.clone())
            .build()
            .await
            .expect("simulador SNMP");
        let port = agent.local_addr().port();
        let worker_agent = agent.clone();
        let worker = tokio::spawn(async move { worker_agent.run().await });
        tokio::task::yield_now().await;

        for version in [SnmpVersion::V1, SnmpVersion::V2c] {
            let mut config = SnmpConfig::v2c("127.0.0.1", "public", port);
            config.version = version;
            config.timeout_ms = 500;
            let client = SnmpClient::new(config);
            let values = client
                .get(&["1.3.6.1.2.1.1.1.0"])
                .await
                .expect("GET de sistema");
            assert!(values["1.3.6.1.2.1.1.1.0"].is_some());
        }

        let v3 = |user: &str, auth_key: &str, privacy: Option<&str>| {
            let mut config = SnmpConfig::from_credentials(
                "127.0.0.1",
                port,
                &SnmpCredentials {
                    version: SnmpVersion::V3,
                    community: String::new(),
                    username: Some(user.into()),
                    auth_protocol: Some("sha256".into()),
                    auth_key: Some(auth_key.into()),
                    priv_protocol: privacy.map(|_| "aes128".into()),
                    priv_key: privacy.map(Into::into),
                    context_name: None,
                },
            );
            config.timeout_ms = 500;
            SnmpClient::new(config)
        };
        // authPriv e authNoPriv pelo mesmo caminho do perfil.
        assert!(!v3("scanner", "auth-password", Some("priv-password"))
            .walk("1.3.6.1.2.1.1")
            .await
            .expect("GETBULK SNMPv3 authPriv")
            .is_empty());
        assert!(v3("leitor", "auth-password", None)
            .get(&["1.3.6.1.2.1.1.1.0"])
            .await
            .expect("GET SNMPv3 authNoPriv")["1.3.6.1.2.1.1.1.0"]
            .is_some());
        // Senha errada não vira "sem dados": é erro.
        assert!(v3("scanner", "senha-errada", Some("priv-password"))
            .get(&["1.3.6.1.2.1.1.1.0"])
            .await
            .is_err());

        cancel.cancel();
        let _ = worker.await;

        let cancel = CancellationToken::new();
        let agent = async_snmp::Agent::builder()
            .bind("127.0.0.1:0")
            .community(b"public")
            .handler(
                oid!(1, 3, 6, 1, 2, 1, 1),
                Arc::new(FaultyOnceMib(AtomicBool::new(false))),
            )
            .cancel(cancel.clone())
            .build()
            .await
            .expect("agente com OID fora de ordem");
        let port = agent.local_addr().port();
        let worker_agent = agent.clone();
        let worker = tokio::spawn(async move { worker_agent.run().await });
        let client = SnmpClient::new(SnmpConfig::v2c("127.0.0.1", "public", port));
        assert_eq!(client.walk("1.3.6.1.2.1.1").await.unwrap().len(), 1);
        cancel.cancel();
        let _ = worker.await;

        let cancel = CancellationToken::new();
        let agent = async_snmp::Agent::builder()
            .bind("127.0.0.1:0")
            .community(b"public")
            .handler(
                oid!(1, 3, 6, 1, 2, 1, 1),
                Arc::new(DelayedOnceMib(AtomicBool::new(false))),
            )
            .cancel(cancel.clone())
            .build()
            .await
            .expect("agente atrasado");
        let port = agent.local_addr().port();
        let worker_agent = agent.clone();
        let worker = tokio::spawn(async move { worker_agent.run().await });
        let mut config = SnmpConfig::v2c("127.0.0.1", "public", port);
        config.timeout_ms = 100;
        let values = SnmpClient::new(config)
            .get(&["1.3.6.1.2.1.1.1.0"])
            .await
            .expect("retry após resposta atrasada");
        assert!(values["1.3.6.1.2.1.1.1.0"].is_some());
        cancel.cancel();
        let _ = worker.await;
    }
}
