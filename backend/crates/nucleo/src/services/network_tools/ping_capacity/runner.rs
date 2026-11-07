//! Coleta das amostras: ecos pequenos e grandes intercalados por um único
//! pinger, sobre o socket ICMP DGRAM do processo (ADR 003).

use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

use surge_ping::{Client, PingIdentifier, PingSequence};
use tokio_util::sync::CancellationToken;

use super::estimator::{PingSamples, LARGE_PAYLOAD, SMALL_PAYLOAD};
use crate::services::network_tools::icmp_probe::duration_to_ms;

/// Parâmetros da rodada. O padrão é o do spike: 80 pares a cada 20 ms
/// (~100 pacotes/s, ~70 kbit/s, ~3,5 s) — leve para qualquer equipamento.
#[derive(Debug, Clone)]
pub struct QuickPingPlan {
    pub pairs: u16,
    pub gap: Duration,
    /// Espera máxima por um eco.
    pub timeout: Duration,
    /// Teto da rodada inteira: com perda alta, cada eco perdido custa `timeout`.
    pub deadline: Duration,
    /// Desiste se nenhum destes primeiros pares tiver resposta.
    pub give_up_after_pairs: u16,
}

impl Default for QuickPingPlan {
    fn default() -> Self {
        Self {
            pairs: 80,
            gap: Duration::from_millis(20),
            timeout: Duration::from_secs(1),
            deadline: Duration::from_secs(15),
            give_up_after_pairs: 5,
        }
    }
}

/// Envia os pares e devolve os RTTs em ordem de envio. Um pinger só (um
/// identificador) para a rodada inteira; o socket é o do cliente recebido.
pub async fn collect_samples(
    client: &Client,
    target: IpAddr,
    plan: &QuickPingPlan,
    cancel: &CancellationToken,
) -> PingSamples {
    let mut pinger = client.pinger(target, PingIdentifier(rand::random())).await;
    pinger.timeout(plan.timeout);
    let (small_payload, large_payload) = (vec![0_u8; SMALL_PAYLOAD], vec![0_u8; LARGE_PAYLOAD]);
    let capacity = usize::from(plan.pairs);
    let mut samples = PingSamples::new(Vec::with_capacity(capacity), Vec::with_capacity(capacity));
    let started = Instant::now();
    let mut answered = false;

    'pairs: for pair in 0..plan.pairs {
        if cancel.is_cancelled() || started.elapsed() >= plan.deadline {
            break;
        }
        if !answered && pair >= plan.give_up_after_pairs {
            break;
        }
        // Intercalados: a mesma condição de fila vale para os dois tamanhos.
        for (offset, payload) in [(0_u16, &small_payload), (1, &large_payload)] {
            let sequence = PingSequence(pair.wrapping_mul(2).wrapping_add(offset));
            let rtt = pinger
                .ping(sequence, payload)
                .await
                .ok()
                .map(|(_, rtt)| duration_to_ms(rtt));
            answered |= rtt.is_some();
            if offset == 0 {
                samples.small.push(rtt);
            } else {
                samples.large.push(rtt);
            }
            tokio::select! {
                () = cancel.cancelled() => break 'pairs,
                () = tokio::time::sleep(plan.gap) => {}
            }
        }
    }
    // Um par interrompido no meio não entra na conta.
    let pairs = samples.small.len().min(samples.large.len());
    samples.small.truncate(pairs);
    samples.large.truncate(pairs);
    samples
}
