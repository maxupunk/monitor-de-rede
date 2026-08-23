//! Scanner TCP/UDP inspirado na estratégia do RustScan, sem depender da crate
//! GPL do binário. O algoritmo é um serviço reutilizável e cancelável.

use std::{
    io,
    net::{IpAddr, SocketAddr},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use futures::{stream, StreamExt};
use serde::Serialize;
use tokio::{
    net::UdpSocket,
    sync::{mpsc, Mutex, Semaphore},
};
use tokio_util::sync::CancellationToken;

use super::{tcp_probe::probe_tcp, udp_probes::probe_for};

pub const MAX_PORTS_PER_SCAN: usize = u16::MAX as usize;
pub const PORTS_PER_BATCH: usize = 1_024;
const DEFAULT_GLOBAL_CONCURRENCY: usize = 512;

fn global_concurrency() -> &'static Semaphore {
    static LIMIT: OnceLock<Semaphore> = OnceLock::new();
    LIMIT.get_or_init(|| {
        let configured = std::env::var("PORT_SCAN_MAX_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(DEFAULT_GLOBAL_CONCURRENCY)
            .clamp(16, 4_096);
        Semaphore::new(configured)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanProfile {
    Fast,
    Reliable,
    Complete,
}

impl ScanProfile {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "fast" | "rapido" | "rápido" => Some(Self::Fast),
            "reliable" | "confiavel" | "confiável" => Some(Self::Reliable),
            "complete" | "completo" => Some(Self::Complete),
            _ => None,
        }
    }
}

impl PortProtocol {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortScanItem {
    pub port: u16,
    pub protocol: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<&'static str>,
    pub latency_ms: f64,
    pub attempts: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Eventos internos do stream NDJSON. Mantê-los tipados evita que detalhes do
/// transporte HTTP contaminem o scanner.
#[derive(Debug, Clone)]
pub enum PortScanEvent {
    Result(PortScanItem),
    Done,
}

#[derive(Debug, Clone)]
pub struct ScanStrategy {
    pub batch_size: usize,
    pub timeout: Duration,
    pub adaptive: bool,
    pub max_retries: u8,
    pub min_timeout: Duration,
    pub retry_delay: Duration,
}

impl ScanStrategy {
    #[must_use]
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self::for_profile(ScanProfile::Reliable, timeout_ms)
    }

    #[must_use]
    pub fn for_profile(profile: ScanProfile, timeout_ms: u64) -> Self {
        let timeout = Duration::from_millis(timeout_ms.clamp(100, 5_000));
        let (batch_size, max_retries, minimum_ms, retry_delay_ms) = match profile {
            ScanProfile::Fast => (512, 0, 150, 50),
            ScanProfile::Reliable => (256, 1, 750, 100),
            ScanProfile::Complete => (64, 3, 1_200, 200),
        };
        Self {
            batch_size,
            timeout,
            adaptive: true,
            max_retries,
            min_timeout: Duration::from_millis(minimum_ms).min(timeout),
            retry_delay: Duration::from_millis(retry_delay_ms),
        }
    }
}

#[derive(Debug, Default)]
struct AdaptiveTimeout {
    average_ms: Option<f64>,
    deviation_ms: f64,
    samples: u64,
    failures: u64,
}
impl AdaptiveTimeout {
    fn next(&self, fallback: Duration, minimum: Duration, adaptive: bool) -> Duration {
        if !adaptive {
            return fallback;
        }
        let Some(average_ms) = self.average_ms else {
            return fallback;
        };
        let loss = self.loss_rate();
        let estimated_ms = (average_ms + 4.0 * self.deviation_ms) * (1.0 + loss * 2.0);
        Duration::from_secs_f64(
            (estimated_ms / 1_000.0).clamp(minimum.as_secs_f64(), fallback.as_secs_f64()),
        )
    }
    fn observe(&mut self, rtt: Duration) {
        self.samples += 1;
        let sample_ms = rtt.as_secs_f64() * 1_000.0;
        match self.average_ms {
            None => {
                self.average_ms = Some(sample_ms);
                self.deviation_ms = sample_ms / 2.0;
            }
            Some(average_ms) => {
                self.deviation_ms =
                    0.75 * self.deviation_ms + 0.25 * (sample_ms - average_ms).abs();
                self.average_ms = Some(0.875 * average_ms + 0.125 * sample_ms);
            }
        }
    }

    fn observe_failure(&mut self) {
        self.samples += 1;
        self.failures += 1;
    }

    fn loss_rate(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.failures as f64 / self.samples as f64
        }
    }

    fn concurrency(&self, configured: usize) -> usize {
        match self.loss_rate() {
            loss if loss >= 0.5 => (configured / 4).max(16),
            loss if loss >= 0.2 => (configured / 2).max(16),
            _ => configured,
        }
    }
}

/// Faz scan em lotes. O canal é o contrato de streaming: se o consumidor some,
/// o envio falha, o token é cancelado e nenhuma porta nova é iniciada.
pub async fn scan(
    host: IpAddr,
    ports: &[u16],
    protocol: PortProtocol,
    strategy: ScanStrategy,
    on_result: mpsc::Sender<PortScanEvent>,
    cancel: CancellationToken,
) -> Vec<PortScanItem> {
    let adaptive = Arc::new(Mutex::new(AdaptiveTimeout::default()));
    let all = Arc::new(Mutex::new(Vec::with_capacity(ports.len())));
    let configured_concurrency = strategy.batch_size.clamp(16, 4_096).min(ports.len().max(1));

    let started = Instant::now();
    for batch in ports.chunks(PORTS_PER_BATCH) {
        if cancel.is_cancelled() {
            break;
        }
        let base_timeout = strategy.timeout;
        let adaptive_enabled = strategy.adaptive;
        let min_timeout = strategy.min_timeout;
        let max_retries = strategy.max_retries;
        let retry_delay = strategy.retry_delay;
        let concurrency = adaptive.lock().await.concurrency(configured_concurrency);
        stream::iter(batch.iter().copied())
            .for_each_concurrent(concurrency, |port| {
                let cancel = cancel.clone();
                let adaptive = Arc::clone(&adaptive);
                let all = Arc::clone(&all);
                let sender = on_result.clone();
                async move {
                    if cancel.is_cancelled() {
                        return;
                    }
                    let timeout =
                        adaptive
                            .lock()
                            .await
                            .next(base_timeout, min_timeout, adaptive_enabled);
                    let Ok(_permit) = global_concurrency().acquire().await else {
                        return;
                    };
                    let item = match protocol {
                        PortProtocol::Tcp => {
                            scan_tcp(host, port, timeout, max_retries, retry_delay).await
                        }
                        PortProtocol::Udp => {
                            scan_udp(host, port, timeout, max_retries, retry_delay).await
                        }
                    };
                    if adaptive_enabled {
                        let mut adaptive = adaptive.lock().await;
                        if matches!(item.status.as_str(), "open" | "closed") {
                            adaptive.observe(Duration::from_secs_f64(item.latency_ms / 1_000.0));
                        } else {
                            adaptive.observe_failure();
                        }
                    }
                    all.lock().await.push(item.clone());
                    if sender.send(PortScanEvent::Result(item)).await.is_err() {
                        cancel.cancel();
                    }
                }
            })
            .await;
    }
    let mut results = all.lock().await.clone();
    results.sort_by_key(|item| item.port);
    tracing::info!(
        target = %host,
        protocol = ?protocol,
        ports = results.len(),
        open = results.iter().filter(|item| item.status == "open").count(),
        filtered = results.iter().filter(|item| item.status.contains("filtered")).count(),
        errors = results.iter().filter(|item| item.status == "error").count(),
        attempts = results.iter().map(|item| u64::from(item.attempts)).sum::<u64>(),
        rate_per_second = results.len() as f64 / started.elapsed().as_secs_f64().max(0.001),
        duration_ms = started.elapsed().as_millis(),
        "varredura de portas concluída"
    );
    results
}

async fn scan_tcp(
    host: IpAddr,
    port: u16,
    timeout: Duration,
    max_retries: u8,
    retry_delay: Duration,
) -> PortScanItem {
    let started = Instant::now();
    let mut attempts = 0;
    let (status, error) = loop {
        attempts += 1;
        let observation = probe_tcp(SocketAddr::new(host, port), timeout).await;
        let status = observation.state;
        if status.proves_reachability() || !status.retryable() || attempts > max_retries {
            break (status, observation.error);
        }
        tokio::time::sleep(retry_backoff(retry_delay, attempts)).await;
    };
    PortScanItem {
        port,
        protocol: "tcp".into(),
        status: status.as_str().into(),
        service: tcp_service(port),
        latency_ms: millis(started.elapsed()),
        attempts,
        error,
    }
}

fn retry_backoff(base: Duration, attempt: u8) -> Duration {
    let factor = 1_u32 << u32::from(attempt.saturating_sub(1).min(5));
    let scaled = base.saturating_mul(factor);
    let jitter_limit = (scaled.as_millis() / 4).max(1) as u64;
    scaled + Duration::from_millis(rand::random::<u64>() % jitter_limit)
}

/// Traduz o desfecho de uma sondagem UDP no vocabulário do `nmap`.
///
/// UDP não tem handshake: silêncio é ambíguo por natureza — pode ser porta
/// aberta que não responde àquele payload, ou firewall descartando o pacote. Só
/// o `ICMP port unreachable`, que o SO entrega como `ECONNREFUSED`, prova que a
/// porta está fechada. Por isso o padrão é `open|filtered` e **não** `closed`:
/// afirmar "fechada" no escuro esconderia serviço ativo do operador.
#[must_use]
pub fn classify_udp_outcome(outcome: Result<io::Result<()>, ()>) -> &'static str {
    match outcome {
        // Estouro do timeout: ninguém respondeu, e isso não decide nada.
        Err(()) => "open|filtered",
        Ok(Ok(())) => "open",
        Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => "closed",
        Ok(Err(_)) => "open|filtered",
    }
}

async fn scan_udp(
    host: IpAddr,
    port: u16,
    timeout: Duration,
    max_retries: u8,
    retry_delay: Duration,
) -> PortScanItem {
    let started = Instant::now();
    let mut attempts = 0;
    let (status, error) = loop {
        attempts += 1;
        let outcome = tokio::time::timeout(timeout, async {
            let bind = if host.is_ipv4() {
                "0.0.0.0:0"
            } else {
                "[::]:0"
            };
            let socket = UdpSocket::bind(bind).await?;
            socket.connect(SocketAddr::new(host, port)).await?;
            socket.send(&probe_for(port)).await?;
            let mut reply = [0_u8; 4_096];
            socket.recv(&mut reply).await.map(|_| ())
        })
        .await;
        let (status, error) = match outcome {
            Err(error) => ("open|filtered", Some(error.to_string())),
            Ok(Ok(())) => ("open", None),
            Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => ("closed", None),
            Ok(Err(error)) => ("open|filtered", Some(error.to_string())),
        };
        let retryable = status == "open|filtered";
        if !retryable || attempts > max_retries {
            break (status, error);
        }
        tokio::time::sleep(retry_backoff(retry_delay, attempts)).await;
    };
    PortScanItem {
        port,
        protocol: "udp".into(),
        status: status.into(),
        service: udp_service(port),
        latency_ms: millis(started.elapsed()),
        attempts,
        error,
    }
}

fn millis(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1_000.0 * 1_000.0).round() / 1_000.0
}

fn tcp_service(port: u16) -> Option<&'static str> {
    Some(match port {
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        80 => "HTTP",
        110 => "POP3",
        111 => "RPCBind",
        135 => "MS-RPC",
        139 => "NetBIOS",
        143 => "IMAP",
        161 => "SNMP",
        389 => "LDAP",
        443 => "HTTPS",
        445 => "SMB",
        465 => "SMTPS",
        587 => "SMTP (Submission)",
        993 => "IMAPS",
        995 => "POP3S",
        1433 => "MSSQL",
        1521 => "Oracle DB",
        2049 => "NFS",
        3306 => "MySQL",
        3389 => "RDP",
        5060 => "SIP",
        5432 => "PostgreSQL",
        5900 => "VNC",
        6379 => "Redis",
        8000 => "HTTP-Alt",
        8080 => "HTTP-Proxy",
        8443 => "HTTPS-Alt",
        9000 => "HTTP-Alt",
        27017 => "MongoDB",
        _ => return None,
    })
}
fn udp_service(port: u16) -> Option<&'static str> {
    Some(match port {
        53 => "DNS",
        67 => "DHCP Server",
        68 => "DHCP Client",
        69 => "TFTP",
        123 => "NTP",
        137 => "NetBIOS-NS",
        138 => "NetBIOS-DGM",
        161 => "SNMP",
        162 => "SNMP Trap",
        500 => "IKE/IPSec",
        514 => "Syslog",
        520 => "RIP",
        1900 => "SSDP",
        4500 => "IPSec NAT-T",
        5353 => "mDNS",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn encontra_porta_tcp_local_aberta() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sender, mut receiver) = mpsc::channel(4);
        let scan = tokio::spawn(async move {
            scan(
                "127.0.0.1".parse().unwrap(),
                &[port],
                PortProtocol::Tcp,
                ScanStrategy::with_timeout(500),
                sender,
                CancellationToken::new(),
            )
            .await
        });
        let event = receiver.recv().await.unwrap();
        assert!(
            matches!(event, PortScanEvent::Result(PortScanItem { status, .. }) if status == "open")
        );
        assert_eq!(scan.await.unwrap()[0].status, "open");
    }

    #[tokio::test]
    async fn fixture_controlada_nao_tem_falso_negativo() {
        let mut listeners = Vec::new();
        for _ in 0..5 {
            listeners.push(tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap());
        }
        let ports = listeners
            .iter()
            .map(|listener| listener.local_addr().unwrap().port())
            .collect::<Vec<_>>();
        let (sender, _receiver) = mpsc::channel(ports.len());
        let results = scan(
            "127.0.0.1".parse().unwrap(),
            &ports,
            PortProtocol::Tcp,
            ScanStrategy::for_profile(ScanProfile::Complete, 500),
            sender,
            CancellationToken::new(),
        )
        .await;
        let open = results
            .iter()
            .filter(|item| item.status == "open")
            .map(|item| item.port)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(open, ports.into_iter().collect());
    }

    #[tokio::test]
    async fn varre_multiplos_lotes_tcp_locais() {
        let ports: Vec<_> = (40_000..=41_024).collect();
        let (sender, _receiver) = mpsc::channel(ports.len());
        let started = Instant::now();
        let results = scan(
            "127.0.0.1".parse().unwrap(),
            &ports,
            PortProtocol::Tcp,
            ScanStrategy::for_profile(ScanProfile::Fast, 100),
            sender,
            CancellationToken::new(),
        )
        .await;

        assert_eq!(results.len(), ports.len());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "o scan local levou {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn perfis_confiaveis_mantem_timeout_conservador() {
        let reliable = ScanStrategy::for_profile(ScanProfile::Reliable, 1_500);
        let complete = ScanStrategy::for_profile(ScanProfile::Complete, 1_500);
        assert_eq!(reliable.min_timeout, Duration::from_millis(750));
        assert_eq!(complete.min_timeout, Duration::from_millis(1_200));
        assert!(complete.max_retries > reliable.max_retries);
    }

    #[test]
    fn perda_reduz_concorrencia_e_aumenta_margem() {
        let mut adaptive = AdaptiveTimeout::default();
        adaptive.observe(Duration::from_millis(100));
        adaptive.observe_failure();
        assert_eq!(adaptive.concurrency(256), 64);
        assert!(
            adaptive.next(
                Duration::from_millis(1_500),
                Duration::from_millis(300),
                true
            ) >= Duration::from_millis(300)
        );
    }

    #[test]
    fn udp_so_afirma_fechada_com_icmp_port_unreachable() {
        // Matriz de paridade #50. O ECONNREFUSED é a tradução que o SO dá ao
        // ICMP port unreachable — a única prova de porta fechada em UDP.
        assert_eq!(
            classify_udp_outcome(Ok(Err(io::Error::from(io::ErrorKind::ConnectionRefused)))),
            "closed"
        );
        assert_eq!(classify_udp_outcome(Ok(Ok(()))), "open");
        // Timeout e qualquer outro erro de rede não decidem nada.
        assert_eq!(classify_udp_outcome(Err(())), "open|filtered");
        assert_eq!(
            classify_udp_outcome(Ok(Err(io::Error::from(io::ErrorKind::PermissionDenied)))),
            "open|filtered"
        );
        assert_eq!(
            classify_udp_outcome(Ok(Err(io::Error::from(io::ErrorKind::HostUnreachable)))),
            "open|filtered"
        );
    }
}
