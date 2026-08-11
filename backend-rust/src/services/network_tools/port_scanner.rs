//! Scanner TCP/UDP inspirado na estratégia do RustScan, sem depender da crate
//! GPL do binário. O algoritmo é um serviço reutilizável e cancelável.

use std::{
    io,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{stream, StreamExt};
use serde::Serialize;
use tokio::{
    net::{TcpStream, UdpSocket},
    sync::{mpsc, Mutex},
};
use tokio_util::sync::CancellationToken;

use super::udp_probes::probe_for;

pub const MAX_PORTS_PER_SCAN: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortProtocol {
    Tcp,
    Udp,
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
}

impl ScanStrategy {
    #[must_use]
    pub fn with_timeout(timeout_ms: u64) -> Self {
        // A estratégia tem limite seguro em todas as plataformas. Em Unix o
        // limite de arquivos pode ser injetado depois sem mudar a API pública.
        let fallback = if cfg!(windows) { 512 } else { 256 };
        Self {
            batch_size: fallback,
            timeout: Duration::from_millis(timeout_ms.clamp(100, 5_000)),
            adaptive: true,
        }
    }
}

#[derive(Debug, Default)]
struct AdaptiveTimeout {
    samples_ms: Vec<f64>,
}
impl AdaptiveTimeout {
    fn next(&self, fallback: Duration, adaptive: bool) -> Duration {
        if !adaptive || self.samples_ms.is_empty() {
            return fallback;
        }
        let average = self.samples_ms.iter().sum::<f64>() / self.samples_ms.len() as f64;
        Duration::from_secs_f64((average * 3.0 / 1_000.0).clamp(0.1, fallback.as_secs_f64()))
    }
    fn observe(&mut self, rtt: Duration) {
        self.samples_ms.push(rtt.as_secs_f64() * 1_000.0);
        // Janela móvel curta: uma porta anômala não contamina uma varredura inteira.
        if self.samples_ms.len() > 32 {
            self.samples_ms.remove(0);
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
    let concurrency = strategy.batch_size.clamp(16, 4_096).min(ports.len().max(1));

    for batch in ports.chunks(concurrency) {
        if cancel.is_cancelled() {
            break;
        }
        let base_timeout = strategy.timeout;
        let adaptive_enabled = strategy.adaptive;
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
                    let timeout = adaptive.lock().await.next(base_timeout, adaptive_enabled);
                    let item = match protocol {
                        PortProtocol::Tcp => scan_tcp(host, port, timeout).await,
                        PortProtocol::Udp => scan_udp(host, port, timeout).await,
                    };
                    if item.status == "open" && adaptive_enabled {
                        adaptive
                            .lock()
                            .await
                            .observe(Duration::from_secs_f64(item.latency_ms / 1_000.0));
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
    results
}

async fn scan_tcp(host: IpAddr, port: u16, timeout: Duration) -> PortScanItem {
    let started = Instant::now();
    let outcome =
        tokio::time::timeout(timeout, TcpStream::connect(SocketAddr::new(host, port))).await;
    let is_open = matches!(outcome, Ok(Ok(_)));
    PortScanItem {
        port,
        protocol: "tcp".into(),
        status: if is_open { "open" } else { "closed" }.into(),
        service: tcp_service(port),
        latency_ms: millis(started.elapsed()),
    }
}

async fn scan_udp(host: IpAddr, port: u16, timeout: Duration) -> PortScanItem {
    let started = Instant::now();
    let status = match tokio::time::timeout(timeout, async {
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
    .await
    {
        Err(_) => "open|filtered",
        Ok(Ok(())) => "open",
        Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => "closed",
        Ok(Err(_)) => "open|filtered",
    };
    PortScanItem {
        port,
        protocol: "udp".into(),
        status: status.into(),
        service: udp_service(port),
        latency_ms: millis(started.elapsed()),
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
    async fn varre_1024_portas_tcp_locais_em_menos_de_tres_segundos() {
        let ports: Vec<_> = (1..=MAX_PORTS_PER_SCAN as u16).collect();
        let (sender, _receiver) = mpsc::channel(MAX_PORTS_PER_SCAN);
        let started = Instant::now();
        let results = scan(
            "127.0.0.1".parse().unwrap(),
            &ports,
            PortProtocol::Tcp,
            ScanStrategy::with_timeout(500),
            sender,
            CancellationToken::new(),
        )
        .await;

        assert_eq!(results.len(), MAX_PORTS_PER_SCAN);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "o scan local levou {:?}",
            started.elapsed()
        );
    }
}
