//! Checker ICMP nativo usando `surge-ping` e socket ICMP DGRAM não privilegiado.

use std::{net::IpAddr, sync::Arc, time::Duration};

use chrono::Utc;
use loco_rs::app::AppContext;
use serde::Deserialize;
use socket2::Type;
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

use crate::services::{
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
    monitoring::ping_diagnostics::PingDiagnosticsConfig,
    shared::errors::{AppError, AppResult},
};

/// Cliente único por processo. O socket compartilhado evita abrir um raw/DGRAM
/// socket para cada monitor e o `PingIdentifier` faz a multiplexação segura.
#[derive(Clone)]
pub struct PingClient {
    ipv4: Arc<Client>,
    ipv6: Option<Arc<Client>>,
}

impl PingClient {
    /// Cria o cliente com DGRAM, a modalidade que funciona sem `CAP_NET_RAW`.
    pub fn create() -> AppResult<Self> {
        let ipv4_config = Config::builder()
            .kind(ICMP::V4)
            .sock_type_hint(Type::DGRAM)
            .build();
        let ipv6_config = Config::builder()
            .kind(ICMP::V6)
            .sock_type_hint(Type::DGRAM)
            .build();
        let ipv4 = Client::new(&ipv4_config).map_err(|err| AppError::Internal(err.into()))?;
        let ipv6 = Client::new(&ipv6_config)
            .map(Arc::new)
            .map_err(|error| {
                tracing::warn!(%error, "socket ICMPv6 DGRAM indisponível; ICMPv4 permanece ativo");
                error
            })
            .ok();
        Ok(Self {
            ipv4: Arc::new(ipv4),
            ipv6,
        })
    }

    /// Recupera o cliente inicializado pelo hook do Loco.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store
            .get::<Self>()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Cliente ICMP não inicializado")))
    }

    #[must_use]
    pub fn for_ip(&self, ip: IpAddr) -> Option<Arc<Client>> {
        match ip {
            IpAddr::V4(_) => Some(Arc::clone(&self.ipv4)),
            IpAddr::V6(_) => self.ipv6.clone(),
        }
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
    #[serde(default, rename = "_diagnostics")]
    pub diagnostics: PingDiagnosticsConfig,
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

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq)]
enum PingLookupError {
    Timeout,
    NoAddress,
    Dns(String),
}

async fn resolve_host(host: &str) -> Result<IpAddr, PingLookupError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }

    match tokio::time::timeout(LOOKUP_TIMEOUT, tokio::net::lookup_host((host, 0))).await {
        Ok(Ok(mut addresses)) => addresses
            .next()
            .map(|address| address.ip())
            .ok_or(PingLookupError::NoAddress),
        Ok(Err(error)) => Err(PingLookupError::Dns(error.to_string())),
        Err(_) => Err(PingLookupError::Timeout),
    }
}

fn check_result_from_lookup_error(
    started_at: chrono::DateTime<Utc>,
    host: &str,
    error: PingLookupError,
) -> CheckResult {
    let finished_at = Utc::now();
    let failure_kind = match &error {
        PingLookupError::Timeout => "dns_timeout",
        PingLookupError::NoAddress => "dns_no_address",
        PingLookupError::Dns(_) => "dns_error",
    };
    let (status, message) = match error {
        PingLookupError::Timeout => (
            MonitorStatus::Unknown,
            format!("Tempo esgotado ao resolver {host} para ping (timeout de {LOOKUP_TIMEOUT:?})"),
        ),
        PingLookupError::NoAddress => (
            MonitorStatus::Down,
            format!("nenhum endereço IP foi encontrado no DNS para {host}"),
        ),
        PingLookupError::Dns(error) => (
            MonitorStatus::Down,
            format!("falha na resolução DNS de {host}: {error}"),
        ),
    };
    CheckResult {
        success: false,
        status,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message: Some(message),
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
        data: serde_json::json!({ "failureKind": failure_kind }),
    }
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
        let ip: IpAddr = match resolve_host(&host).await {
            Ok(ip) => ip,
            Err(error) => return check_result_from_lookup_error(started_at, &host, error),
        };
        let count = config.packet_count.clamp(1, 20);
        let Some(client) = self.client.for_ip(ip) else {
            return failed_result(started_at, &host, "socket ICMPv6 indisponível".into());
        };
        let mut pinger = client.pinger(ip, PingIdentifier(rand::random())).await;
        let per_packet_timeout_ms =
            (config.timeout_ms / u64::from(count)).clamp(200, config.timeout_ms.max(200));
        pinger.timeout(Duration::from_millis(per_packet_timeout_ms));
        let payload = vec![0_u8; config.packet_size.clamp(1, 65_507)];
        let mut latencies = Vec::with_capacity(usize::from(count));

        for sequence in 0..count {
            if let Ok((_, rtt)) = pinger.ping(PingSequence(sequence), &payload).await {
                latencies.push(rtt);
            }
        }

        let mut result = summarize(started_at, Utc::now(), &host, count, &latencies);
        if let Some(data) = result.data.as_object_mut() {
            data.insert("resolvedIp".into(), serde_json::json!(ip.to_string()));
        }
        result
    }
}

/// Converte as respostas recebidas na observação que o resto do sistema lê.
///
/// Separado do `execute` porque é aqui que mora a regra da matriz de paridade
/// #1 — perda parcial é `warning`, perda total é `down` — e a única forma de
/// provar isso de forma determinística é sem socket ICMP no meio: um teste que
/// dependesse da rede do executor mediria o ambiente, não a regra.
#[must_use]
pub fn summarize(
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
    host: &str,
    sent: u16,
    latencies: &[Duration],
) -> CheckResult {
    let sent = sent.max(1);
    let received = latencies.len();
    let packet_loss = 100.0 * (f64::from(sent) - received as f64) / f64::from(sent);
    let latency_ms = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().map(Duration::as_secs_f64).sum::<f64>() * 1_000.0 / received as f64
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
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
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
        data: if status == MonitorStatus::Down {
            serde_json::json!({ "failureKind": "icmp_no_reply" })
        } else {
            serde_json::json!({})
        },
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
        data: serde_json::json!({ "failureKind": "icmp_socket_error" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medida(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    fn resumo(sent: u16, latencies: &[Duration]) -> CheckResult {
        let started_at = Utc::now();
        summarize(started_at, started_at, "192.0.2.10", sent, latencies)
    }

    fn metrica(result: &CheckResult, name: &str) -> f64 {
        result
            .metrics
            .iter()
            .find(|metric| metric.name == name)
            .map(|metric| metric.value)
            .expect("métrica presente")
    }

    #[test]
    fn resposta_completa_mede_a_media_e_fica_up() {
        let result = resumo(3, &[medida(10), medida(20), medida(30)]);
        assert_eq!(result.status, MonitorStatus::Up);
        assert!(result.success);
        assert!((metrica(&result, "latency") - 20.0).abs() < 0.001);
        assert!((metrica(&result, "packet_loss") - 0.0).abs() < 0.001);
    }

    #[test]
    fn perda_parcial_e_warning_e_nao_down() {
        // Matriz de paridade #1: o host respondeu, então ele está acessível —
        // rebaixar para `down` abriria alerta de queda num link só degradado.
        let result = resumo(3, &[medida(10), medida(20)]);
        assert_eq!(result.status, MonitorStatus::Warning);
        assert!(result.success);
        assert!((metrica(&result, "packet_loss") - 100.0 / 3.0).abs() < 0.001);
        assert!((metrica(&result, "latency") - 15.0).abs() < 0.001);
    }

    #[test]
    fn perda_total_e_down_com_mensagem_clara() {
        let result = resumo(3, &[]);
        assert_eq!(result.status, MonitorStatus::Down);
        assert!(!result.success);
        assert!((metrica(&result, "packet_loss") - 100.0).abs() < 0.001);
        // Sem resposta a latência é 0, e não uma média de conjunto vazio (NaN).
        assert!((metrica(&result, "latency") - 0.0).abs() < 0.001);
        assert_eq!(
            result.message.as_deref(),
            Some("Host 192.0.2.10 inacessível (100% perda de pacotes)")
        );
    }

    #[test]
    fn a_mensagem_de_sucesso_arredonda_para_duas_casas() {
        let result = resumo(2, &[medida(12), medida(13)]);
        assert_eq!(
            result.message.as_deref(),
            Some("Ping para 192.0.2.10 finalizado em 12.5ms (0% perda)")
        );
        // Perda parcial arredonda o percentual para inteiro, como o `toFixed(0)`.
        assert_eq!(
            resumo(3, &[medida(12), medida(13)]).message.as_deref(),
            Some("Ping para 192.0.2.10 finalizado em 12.5ms (33% perda)")
        );
    }

    #[tokio::test]
    async fn resolve_ip_literal_sem_consultar_dns() {
        assert_eq!(
            resolve_host("192.0.2.10").await,
            Ok("192.0.2.10".parse::<IpAddr>().unwrap())
        );
        assert_eq!(
            resolve_host("2001:db8::1").await,
            Ok("2001:db8::1".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn timeout_de_lookup_gera_resultado_unknown() {
        let started_at = Utc::now();
        let result =
            check_result_from_lookup_error(started_at, "router.local", PingLookupError::Timeout);
        assert_eq!(result.status, MonitorStatus::Unknown);
        assert!(!result.success);
        assert!(result
            .message
            .as_deref()
            .expect("mensagem presente")
            .contains("Tempo esgotado ao resolver router.local"));
        assert_eq!(metrica(&result, "packet_loss"), 100.0);
        assert_eq!(metrica(&result, "latency"), 0.0);
    }

    #[test]
    fn erro_de_dns_gera_resultado_down() {
        let started_at = Utc::now();
        let result = check_result_from_lookup_error(
            started_at,
            "router.local",
            PingLookupError::Dns("NXDOMAIN".into()),
        );
        assert_eq!(result.status, MonitorStatus::Down);
        assert!(!result.success);
        assert!(result
            .message
            .as_deref()
            .expect("mensagem presente")
            .contains("falha na resolução DNS de router.local"));
    }

    #[test]
    fn endereco_vazio_gera_resultado_down() {
        let started_at = Utc::now();
        let result =
            check_result_from_lookup_error(started_at, "router.local", PingLookupError::NoAddress);
        assert_eq!(result.status, MonitorStatus::Down);
        assert!(!result.success);
        assert!(result
            .message
            .as_deref()
            .expect("mensagem presente")
            .contains("nenhum endereço IP foi encontrado"));
    }
}
