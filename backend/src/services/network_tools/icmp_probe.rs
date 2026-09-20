//! Sondagem ICMP (Ping) unificada para diagnósticos e ferramentas de rede.
//!
//! Executa sondas ICMP Echo Request usando socket DGRAM não-privilegiado (ADR 003),
//! calculando métricas de latência média, mínima, máxima, jitter e perda de pacotes.
//! Segue os princípios SOLID (Single Responsibility e Interface Segregation).

use std::{net::IpAddr, time::Duration};

use socket2::Type;
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use tokio_util::sync::CancellationToken;

pub const DEFAULT_PROBE_COUNT: usize = 3;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1_500);
pub const DEFAULT_PAYLOAD_SIZE: usize = 56;

/// Opções de configuração para a sonda ICMP.
#[derive(Debug, Clone)]
pub struct IcmpProbeOptions {
    pub count: usize,
    pub timeout: Duration,
    pub payload_size: usize,
}

impl Default for IcmpProbeOptions {
    fn default() -> Self {
        Self {
            count: DEFAULT_PROBE_COUNT,
            timeout: DEFAULT_TIMEOUT,
            payload_size: DEFAULT_PAYLOAD_SIZE,
        }
    }
}

impl IcmpProbeOptions {
    pub fn new(count: usize, timeout: Duration) -> Self {
        Self {
            count: count.clamp(1, 20),
            timeout,
            payload_size: DEFAULT_PAYLOAD_SIZE,
        }
    }
}

/// Resultado detalhado da execução da sonda ICMP.
#[derive(Debug, Clone)]
pub struct IcmpProbeResult {
    pub success: bool,
    pub rtts: Vec<Option<f64>>,
    pub avg_rtt_ms: Option<f64>,
    pub min_rtt_ms: Option<f64>,
    pub max_rtt_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss_pct: f64,
}

/// Arredonda um valor de ponto flutuante para duas casas decimais.
pub fn round_two(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

/// Converte um `Duration` em milissegundos com arredondamento para duas casas decimais.
pub fn duration_to_ms(duration: Duration) -> f64 {
    round_two(duration.as_secs_f64() * 1_000.0)
}

/// Cria um cliente ICMP DGRAM não-privilegiado (ADR 003) para IPv4 ou IPv6,
/// opcionalmente definindo o TTL do socket.
pub fn create_icmp_client(is_ipv4: bool, ttl: Option<u32>) -> std::io::Result<Client> {
    let mut builder = Config::builder()
        .kind(if is_ipv4 { ICMP::V4 } else { ICMP::V6 })
        .sock_type_hint(Type::DGRAM);
    if let Some(t) = ttl {
        builder = builder.ttl(t);
    }
    Client::new(&builder.build())
}

/// Calcula o jitter (variação média de latência entre amostras consecutivas)
/// conforme a RFC 3550 / RFC 1889.
pub fn calculate_jitter(latencies: &[f64]) -> Option<f64> {
    if latencies.len() < 2 {
        return None;
    }
    let diff_sum: f64 = latencies.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
    Some(round_two(diff_sum / (latencies.len() - 1) as f64))
}

/// Executa múltiplas sondas ICMP contra o alvo, retornando métricas consolidadas.
pub async fn probe_icmp(
    target: IpAddr,
    options: &IcmpProbeOptions,
    cancel: &CancellationToken,
) -> IcmpProbeResult {
    let is_ipv4 = target.is_ipv4();
    let client = match create_icmp_client(is_ipv4, None) {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(%err, %target, "falha ao criar socket ICMP DGRAM");
            return IcmpProbeResult {
                success: false,
                rtts: Vec::new(),
                avg_rtt_ms: None,
                min_rtt_ms: None,
                max_rtt_ms: None,
                jitter_ms: None,
                packet_loss_pct: 100.0,
            };
        }
    };

    let mut pinger = client.pinger(target, PingIdentifier(rand::random())).await;
    pinger.timeout(options.timeout);
    let payload = vec![0u8; options.payload_size];
    let mut rtts = Vec::with_capacity(options.count);

    for seq in 0..options.count {
        if cancel.is_cancelled() {
            break;
        }
        match pinger.ping(PingSequence(seq as u16), &payload).await {
            Ok((_packet, rtt)) => {
                let ms = duration_to_ms(rtt);
                rtts.push(Some(ms));
            }
            Err(_) => {
                rtts.push(None);
            }
        }
    }

    calculate_probe_result(rtts, options.count)
}

/// Executa uma sonda ICMP simples retornando apenas o RTT em milissegundos se bem-sucedido.
pub async fn probe_icmp_single(
    target: IpAddr,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Option<f64> {
    let options = IcmpProbeOptions::new(1, timeout);
    let res = probe_icmp(target, &options, cancel).await;
    res.avg_rtt_ms
}

fn calculate_probe_result(rtts: Vec<Option<f64>>, total_sent: usize) -> IcmpProbeResult {
    let successful: Vec<f64> = rtts.iter().flatten().copied().collect();
    let received_count = successful.len();
    let total = total_sent.max(1);
    let packet_loss_pct = round_two(((total - received_count) as f64 / total as f64) * 100.0);

    if successful.is_empty() {
        return IcmpProbeResult {
            success: false,
            rtts,
            avg_rtt_ms: None,
            min_rtt_ms: None,
            max_rtt_ms: None,
            jitter_ms: None,
            packet_loss_pct: 100.0,
        };
    }

    let sum: f64 = successful.iter().sum();
    let avg = round_two(sum / received_count as f64);
    let min = round_two(successful.iter().cloned().fold(f64::INFINITY, f64::min));
    let max = round_two(successful.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let jitter = calculate_jitter(&successful);

    IcmpProbeResult {
        success: true,
        rtts,
        avg_rtt_ms: Some(avg),
        min_rtt_ms: Some(min),
        max_rtt_ms: Some(max),
        jitter_ms: jitter,
        packet_loss_pct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_two_casas_decimais() {
        assert_eq!(round_two(12.3456), 12.35);
        assert_eq!(round_two(12.341), 12.34);
        assert_eq!(round_two(0.0), 0.0);
    }

    #[test]
    fn duration_to_ms_converte_corretamente() {
        assert_eq!(duration_to_ms(Duration::from_millis(150)), 150.0);
        assert_eq!(duration_to_ms(Duration::from_micros(12_345)), 12.35);
    }

    #[test]
    fn options_respeita_clamping() {
        let opt = IcmpProbeOptions::new(0, Duration::from_secs(1));
        assert_eq!(opt.count, 1);
        let opt2 = IcmpProbeOptions::new(50, Duration::from_secs(1));
        assert_eq!(opt2.count, 20);
    }

    #[test]
    fn calcula_estatisticas_com_sucesso_total() {
        let rtts = vec![Some(10.0), Some(20.0), Some(15.0)];
        let res = calculate_probe_result(rtts, 3);
        assert!(res.success);
        assert_eq!(res.avg_rtt_ms, Some(15.0));
        assert_eq!(res.min_rtt_ms, Some(10.0));
        assert_eq!(res.max_rtt_ms, Some(20.0));
        assert_eq!(res.packet_loss_pct, 0.0);
        assert!(res.jitter_ms.is_some());
    }

    #[test]
    fn calcula_estatisticas_com_perda_parcial() {
        let rtts = vec![Some(10.0), None, Some(20.0)];
        let res = calculate_probe_result(rtts, 3);
        assert!(res.success);
        assert_eq!(res.avg_rtt_ms, Some(15.0));
        assert_eq!(res.packet_loss_pct, 33.33);
    }

    #[test]
    fn calcula_estatisticas_com_perda_total() {
        let rtts = vec![None, None, None];
        let res = calculate_probe_result(rtts, 3);
        assert!(!res.success);
        assert_eq!(res.packet_loss_pct, 100.0);
    }

    #[test]
    fn calcula_jitter_correto() {
        let latencies = vec![10.0, 12.0, 11.0, 15.0];
        // |12-10| = 2, |11-12| = 1, |15-11| = 4 -> soma = 7 / 3 = 2.33
        assert_eq!(calculate_jitter(&latencies), Some(2.33));
        assert_eq!(calculate_jitter(&[10.0]), None);
        assert_eq!(calculate_jitter(&[]), None);
    }
}
