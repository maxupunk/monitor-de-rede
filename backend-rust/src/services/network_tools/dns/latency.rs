//! Medições DNS. Falha de rede vira amostra `success: false`, nunca erro que
//! derrube um checker ou interrompa o benchmark inteiro.

use std::time::{Duration, Instant};

use hickory_proto::{op::Message, rr::RecordType};
use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

use crate::services::{
    network_tools::dns::wire,
    shared::errors::{AppError, AppResult},
};

pub const DEFAULT_DNS_PORT: u16 = 53;
pub const DEFAULT_DNS_TIMEOUT_MS: u64 = 3_000;
pub const DEFAULT_BENCHMARK_HOSTNAMES: [&str; 3] = ["google.com", "cloudflare.com", "globo.com"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsProtocol {
    Udp,
    Tcp,
    Doh,
    System,
}

impl DnsProtocol {
    pub fn parse(value: Option<&str>) -> AppResult<Self> {
        match value.unwrap_or("udp").trim().to_ascii_lowercase().as_str() {
            "udp" => Ok(Self::Udp),
            "tcp" => Ok(Self::Tcp),
            "doh" => Ok(Self::Doh),
            "system" => Ok(Self::System),
            _ => Err(AppError::validation("Protocolo DNS inválido")),
        }
    }
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
            Self::Doh => "doh",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DnsLookupOptions {
    pub hostname: String,
    pub record_type: RecordType,
    pub server: Option<String>,
    pub protocol: DnsProtocol,
    pub doh_url: Option<String>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsLookupSample {
    pub hostname: String,
    pub record_type: String,
    pub protocol: String,
    pub server: String,
    pub success: bool,
    pub lookup_time_ms: Option<f64>,
    pub answers: Vec<wire::DnsAnswer>,
    pub truncated: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DnsServerTarget {
    pub server: String,
    pub label: Option<String>,
    pub protocol: DnsProtocol,
}

#[derive(Debug, Clone)]
pub struct DnsBenchmarkOptions {
    pub servers: Vec<DnsServerTarget>,
    pub hostnames: Vec<String>,
    pub record_type: RecordType,
    pub timeout_ms: u64,
    pub rounds: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsServerRanking {
    pub server: String,
    pub label: String,
    pub protocol: String,
    pub avg_lookup_time_ms: Option<f64>,
    pub min_lookup_time_ms: Option<f64>,
    pub max_lookup_time_ms: Option<f64>,
    pub median_lookup_time_ms: Option<f64>,
    pub success_rate: f64,
    pub total_queries: usize,
    pub failed_queries: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub const DEFAULT_DNS_SERVERS: &[(&str, &str)] = &[
    ("Cloudflare", "1.1.1.1"),
    ("Google", "8.8.8.8"),
    ("Quad9", "9.9.9.9"),
    ("OpenDNS", "208.67.222.222"),
    ("AdGuard", "94.140.14.14"),
];

/// Separa host e porta sem destruir IPv6 entre colchetes.
pub fn parse_server_address(raw: &str) -> AppResult<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::validation("Informe o endereço do servidor DNS"));
    }
    if let Some(rest) = raw.strip_prefix('[') {
        let Some(end) = rest.find(']') else {
            return Err(AppError::validation("Endereço IPv6 DNS inválido"));
        };
        let host = &rest[..end];
        let suffix = &rest[end + 1..];
        let port = if suffix.is_empty() {
            DEFAULT_DNS_PORT
        } else if let Some(port) = suffix.strip_prefix(':') {
            parse_port(port)?
        } else {
            return Err(AppError::validation("Endereço IPv6 DNS inválido"));
        };
        return Ok((host.to_string(), port));
    }
    // Um único ':' é host:port; mais de um é IPv6 sem colchetes.
    if raw.matches(':').count() == 1 {
        let (host, port) = raw
            .rsplit_once(':')
            .expect("contagem de dois pontos já verificada");
        if let Ok(port) = parse_port(port) {
            return Ok((host.to_string(), port));
        }
    }
    Ok((raw.to_string(), DEFAULT_DNS_PORT))
}
fn parse_port(raw: &str) -> AppResult<u16> {
    raw.parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| AppError::validation("Porta DNS inválida"))
}

pub async fn measure_dns_lookup(options: DnsLookupOptions) -> DnsLookupSample {
    let server = effective_server(&options);
    let record_name = wire::record_type_name(options.record_type).to_string();
    let result = async {
        let query = wire::encode_query(&options.hostname, options.record_type)?;
        let timeout = Duration::from_millis(options.timeout_ms.clamp(200, 15_000));
        let (elapsed, message) = match options.protocol {
            DnsProtocol::Udp => udp(&server, &query, timeout).await?,
            DnsProtocol::Tcp => tcp(&server, &query, timeout).await?,
            DnsProtocol::Doh => doh(&server, &query, timeout).await?,
            DnsProtocol::System => system_lookup(&options.hostname, timeout).await?,
        };
        Ok::<_, AppError>((elapsed, message))
    }
    .await;
    match result {
        Ok((elapsed, message)) => DnsLookupSample {
            hostname: options.hostname,
            record_type: record_name,
            protocol: options.protocol.as_str().into(),
            server,
            success: true,
            lookup_time_ms: Some(round_ms(elapsed)),
            answers: wire::answers(&message),
            truncated: message.truncated(),
            message: wire::response_message(&message),
            error: None,
        },
        Err(error) => DnsLookupSample {
            hostname: options.hostname,
            record_type: record_name,
            protocol: options.protocol.as_str().into(),
            server,
            success: false,
            lookup_time_ms: None,
            answers: vec![],
            truncated: false,
            message: "Falha na resolução DNS".into(),
            error: Some(error.to_string()),
        },
    }
}

fn effective_server(options: &DnsLookupOptions) -> String {
    match options.protocol {
        DnsProtocol::Doh => options
            .doh_url
            .clone()
            .or_else(|| options.server.clone())
            .unwrap_or_else(|| "https://cloudflare-dns.com/dns-query".into()),
        DnsProtocol::System => "sistema".into(),
        _ => options.server.clone().unwrap_or_else(|| "1.1.1.1".into()),
    }
}

async fn udp(server: &str, query: &[u8], timeout: Duration) -> AppResult<(Duration, Message)> {
    let (host, port) = parse_server_address(server)?;
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(internal)?;
    socket
        .connect((host.as_str(), port))
        .await
        .map_err(internal)?;
    let started = Instant::now();
    tokio::time::timeout(timeout, async {
        socket.send(query).await.map_err(internal)?;
        let mut buffer = [0_u8; 4_096];
        let read = socket.recv(&mut buffer).await.map_err(internal)?;
        wire::decode_message(&buffer[..read])
    })
    .await
    .map_err(|_| AppError::BusinessRule("Tempo esgotado na consulta DNS".into()))?
    .map(|message| (started.elapsed(), message))
}
async fn tcp(server: &str, query: &[u8], timeout: Duration) -> AppResult<(Duration, Message)> {
    let (host, port) = parse_server_address(server)?;
    let mut stream = tokio::time::timeout(timeout, TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| AppError::BusinessRule("Tempo esgotado ao conectar no DNS".into()))?
        .map_err(internal)?;
    let length = u16::try_from(query.len())
        .map_err(|_| AppError::validation("Consulta DNS muito grande"))?;
    let started = Instant::now();
    tokio::time::timeout(timeout, async {
        stream
            .write_all(&length.to_be_bytes())
            .await
            .map_err(internal)?;
        stream.write_all(query).await.map_err(internal)?;
        let mut prefix = [0_u8; 2];
        stream.read_exact(&mut prefix).await.map_err(internal)?;
        let mut body = vec![0; usize::from(u16::from_be_bytes(prefix))];
        stream.read_exact(&mut body).await.map_err(internal)?;
        wire::decode_message(&body)
    })
    .await
    .map_err(|_| AppError::BusinessRule("Tempo esgotado na consulta DNS".into()))?
    .map(|message| (started.elapsed(), message))
}
async fn doh(url: &str, query: &[u8], timeout: Duration) -> AppResult<(Duration, Message)> {
    if !url.starts_with("https://") {
        return Err(AppError::validation(
            "O endpoint DoH precisa começar com https://",
        ));
    }
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(internal)?;
    let started = Instant::now();
    let response = client
        .post(url)
        .header("content-type", "application/dns-message")
        .header("accept", "application/dns-message")
        .body(query.to_vec())
        .send()
        .await
        .map_err(internal)?
        .error_for_status()
        .map_err(internal)?
        .bytes()
        .await
        .map_err(internal)?;
    wire::decode_message(&response).map(|message| (started.elapsed(), message))
}
async fn system_lookup(hostname: &str, timeout: Duration) -> AppResult<(Duration, Message)> {
    // Sem fornecedor específico, o sistema só é usado em lookup avulso. Criamos
    // resposta vazia válida: a métrica e o sucesso continuam precisos.
    let started = Instant::now();
    let mut addresses = tokio::time::timeout(timeout, tokio::net::lookup_host((hostname, 0)))
        .await
        .map_err(|_| AppError::BusinessRule("Tempo esgotado na consulta DNS".into()))?
        .map_err(internal)?;
    addresses
        .next()
        .ok_or_else(|| AppError::BusinessRule("Hostname DNS sem endereço".into()))?;
    Ok((started.elapsed(), Message::new()))
}
fn internal(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!(error.to_string()))
}
fn round_ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1_000_000.0).round() / 1_000.0
}

pub async fn benchmark_dns_servers(options: DnsBenchmarkOptions) -> Vec<DnsServerRanking> {
    let mut ranking = Vec::with_capacity(options.servers.len());
    // Serial por desenho: cada resolvedor recebe a mesma carga em sequência.
    for target in options.servers {
        let mut values = Vec::new();
        let mut errors = Vec::new();
        let mut total = 0;
        for _ in 0..options.rounds.clamp(1, 5) {
            for hostname in &options.hostnames {
                total += 1;
                let sample = measure_dns_lookup(DnsLookupOptions {
                    hostname: hostname.clone(),
                    record_type: options.record_type,
                    server: Some(target.server.clone()),
                    protocol: target.protocol,
                    doh_url: (target.protocol == DnsProtocol::Doh).then(|| target.server.clone()),
                    timeout_ms: options.timeout_ms,
                })
                .await;
                if let Some(value) = sample.lookup_time_ms.filter(|_| sample.success) {
                    values.push(value);
                } else if let Some(error) = sample.error {
                    errors.push(error);
                }
            }
        }
        values.sort_by(f64::total_cmp);
        let count = values.len();
        let median = count.checked_sub(1).map(|_| {
            if count % 2 == 0 {
                (values[count / 2 - 1] + values[count / 2]) / 2.0
            } else {
                values[count / 2]
            }
        });
        ranking.push(DnsServerRanking {
            server: target.server.clone(),
            label: target.label.unwrap_or(target.server),
            protocol: target.protocol.as_str().into(),
            avg_lookup_time_ms: (!values.is_empty())
                .then(|| round_value(values.iter().sum::<f64>() / count as f64)),
            min_lookup_time_ms: values.first().copied(),
            max_lookup_time_ms: values.last().copied(),
            median_lookup_time_ms: median.map(round_value),
            success_rate: if total == 0 {
                0.0
            } else {
                round_value(count as f64 / total as f64)
            },
            total_queries: total,
            failed_queries: total - count,
            error: errors.into_iter().next(),
        });
    }
    sort_by_latency(ranking)
}
fn round_value(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}
pub fn sort_by_latency(mut values: Vec<DnsServerRanking>) -> Vec<DnsServerRanking> {
    values.sort_by(|a, b| match (a.avg_lookup_time_ms, b.avg_lookup_time_ms) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.label.cmp(&b.label),
    });
    values
}
