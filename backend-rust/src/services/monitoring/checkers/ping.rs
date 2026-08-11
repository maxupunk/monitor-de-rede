//! Checker ICMP nativo usando `surge-ping` e socket ICMP DGRAM não privilegiado.

use std::{net::IpAddr, sync::Arc, time::Duration};

use chrono::Utc;
use loco_rs::app::AppContext;
use serde::Deserialize;
use socket2::Type;
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

use crate::services::{
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
    shared::errors::{AppError, AppResult},
};

/// Cliente único por processo. O socket compartilhado evita abrir um raw/DGRAM
/// socket para cada monitor e o `PingIdentifier` faz a multiplexação segura.
#[derive(Clone)]
pub struct PingClient(pub Arc<Client>);

impl PingClient {
    /// Cria o cliente com DGRAM, a modalidade que funciona sem `CAP_NET_RAW`.
    pub fn create() -> AppResult<Self> {
        let config = Config::builder()
            .kind(ICMP::V4)
            .sock_type_hint(Type::DGRAM)
            .build();
        Client::new(&config)
            .map(|client| Self(Arc::new(client)))
            .map_err(|err| AppError::Internal(err.into()))
    }

    /// Recupera o cliente inicializado pelo hook do Loco.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store
            .get::<Self>()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Cliente ICMP não inicializado")))
    }
}

/// Configuração compatível com o payload persistido pelo monitor de ping.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingConfig {
    pub host: String,
    #[serde(default = "default_packet_count")]
    pub packet_count: u16,
    #[serde(default = "default_packet_size")]
    pub packet_size: usize,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

const fn default_packet_count() -> u16 {
    3
}
const fn default_packet_size() -> usize {
    56
}
const fn default_timeout_ms() -> u64 {
    5_000
}

/// Checker ICMP associado ao cliente do processo atual.
pub struct PingChecker {
    client: PingClient,
}

impl PingChecker {
    /// Cria o checker usando a dependência injetada pelo `AppContext` do Loco.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        Ok(Self {
            client: PingClient::from_context(ctx)?,
        })
    }
}

#[async_trait::async_trait]
impl Checker for PingChecker {
    type Config = PingConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let host = config.host.clone();
        let ip: IpAddr = match host.parse::<IpAddr>() {
            Ok(ip) if ip.is_ipv4() => ip,
            Ok(_) => {
                return failed_result(
                    started_at,
                    &host,
                    "o checker ICMP atual suporta apenas IPv4".into(),
                )
            }
            Err(_) => match tokio::net::lookup_host((host.as_str(), 0)).await {
                Ok(mut addresses) => match addresses.find(|address| address.ip().is_ipv4()) {
                    Some(address) => address.ip(),
                    None => {
                        return failed_result(
                            started_at,
                            &host,
                            "nenhum endereÃ§o IPv4 foi encontrado no DNS".into(),
                        )
                    }
                },
                Err(error) => return failed_result(started_at, &host, error.to_string()),
            },
        };
        let count = config.packet_count.clamp(1, 20);
        let mut pinger = self
            .client
            .0
            .pinger(ip, PingIdentifier(rand::random()))
            .await;
        pinger.timeout(Duration::from_millis(config.timeout_ms.max(1)));
        let payload = vec![0_u8; config.packet_size.clamp(1, 65_507)];
        let mut latencies = Vec::with_capacity(usize::from(count));

        for sequence in 0..count {
            if let Ok((_, rtt)) = pinger.ping(PingSequence(sequence), &payload).await {
                latencies.push(rtt);
            }
        }

        let finished_at = Utc::now();
        let duration_ms = (finished_at - started_at).num_milliseconds().max(0);
        let packet_loss = 100.0 * (f64::from(count) - latencies.len() as f64) / f64::from(count);
        let latency_ms = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().map(Duration::as_secs_f64).sum::<f64>() * 1_000.0
                / latencies.len() as f64
        };
        let status = if latencies.is_empty() {
            MonitorStatus::Down
        } else if packet_loss > 0.0 {
            MonitorStatus::Warning
        } else {
            MonitorStatus::Up
        };
        let message = if status == MonitorStatus::Down {
            format!("Host {host} inacessível (100% perda de pacotes)")
        } else {
            format!("Ping para {host} finalizado em {latency_ms:.1}ms ({packet_loss:.0}% perda)")
        };
        CheckResult {
            success: status != MonitorStatus::Down,
            status,
            started_at,
            finished_at,
            duration_ms,
            message: Some(message),
            metrics: vec![
                CheckMetric {
                    name: "latency".into(),
                    value: latency_ms,
                    unit: "ms".into(),
                },
                CheckMetric {
                    name: "packet_loss".into(),
                    value: packet_loss,
                    unit: "%".into(),
                },
            ],
            data: serde_json::json!({}),
        }
    }
}

fn failed_result(started_at: chrono::DateTime<Utc>, host: &str, error: String) -> CheckResult {
    let finished_at = Utc::now();
    CheckResult {
        success: false,
        status: MonitorStatus::Down,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        started_at,
        finished_at,
        message: Some(format!("Falha ao executar ping em {host}: {error}")),
        metrics: vec![
            CheckMetric {
                name: "latency".into(),
                value: 0.0,
                unit: "ms".into(),
            },
            CheckMetric {
                name: "packet_loss".into(),
                value: 100.0,
                unit: "%".into(),
            },
        ],
        data: serde_json::json!({}),
    }
}
