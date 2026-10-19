//! Estimador puro do diagnóstico rápido: das amostras de RTT ao relatório.
//!
//! Sem socket nem relógio — recebe os RTTs em ordem de envio e decide o que
//! dá para afirmar. Os limiares saíram do spike `examples/spikes/ping_capacity.rs`
//! em equipamento real (ADR 015 §9):
//!
//! | equipamento | btest | Δmin (3 rodadas) | estimativa |
//! |---|---|---|---|
//! | MikroTik | 135–175 Mbit/s | 0,16–0,24 ms | 94–142 Mbit/s |
//! | OpenWrt BPI-R3 | — | 0,04–0,09 ms | 243–588 (ruído) |
//! | OpenWrt RB922 | — | 0,15–0,20 ms | 116–156 Mbit/s |
//!
//! Abaixo de ~0,12 ms o Δmin é do tamanho do ruído de relógio/agendamento do
//! sistema: a conta explode (o BPI-R3 "deu" 588 Mbit/s). Por isso a
//! capacidade só sai quando o sinal passa desse piso **e** os três lotes da
//! mesma rodada concordam entre si — e mesmo assim como faixa larga.

use super::model::{
    CapacityConfidence, QueueingLevel, QuickPingCapacity, QuickPingQueueing, QuickPingReport,
    QuickPingRtt,
};
use crate::services::network_tools::icmp_probe::{calculate_jitter, round_two};

/// Payload do eco pequeno (o padrão do `ping`).
pub const SMALL_PAYLOAD: usize = 56;
/// Payload do eco grande: 1472 + 28 de cabeçalho = 1500, o MTU Ethernet.
pub const LARGE_PAYLOAD: usize = 1_472;

/// Δmin abaixo disto é ruído (spike: BPI-R3 com 0,04–0,09 ms deu 243–588
/// Mbit/s; MikroTik e RB922 com 0,15–0,24 ms ficaram na ordem certa).
pub const NOISE_FLOOR_MS: f64 = 0.12;
/// Lotes contíguos em que a rodada é dividida para medir a consistência.
pub const BATCHES: usize = 3;
/// Maior/menor estimativa entre lotes acima disto: instável demais, sem número.
pub const MAX_BATCH_RATIO: f64 = 2.0;
/// Acima disto (mas abaixo do máximo) a confiança cai para "muito baixa".
pub const LOW_CONFIDENCE_RATIO: f64 = 1.5;
/// A faixa é alargada em ±20% além das estimativas dos lotes.
pub const RANGE_MARGIN: f64 = 0.2;
/// Respostas mínimas de cada tamanho para tentar a estimativa.
pub const MIN_REPLIES_PER_SIZE: usize = 15;
/// Respostas mínimas de cada tamanho dentro de um lote para ele contar.
pub const MIN_REPLIES_PER_BATCH: usize = 3;

/// Fila: p95 − mínimo a partir do qual há sinal moderado (ms, ou fração do mínimo
/// em caminhos longos, o que for maior). No spike, 4 ms (p95 ~5 ms sobre ~1 ms
/// de mínimo) foi a fila visível em enlace carregado.
pub const QUEUE_MODERATE_MS: f64 = 2.0;
pub const QUEUE_MODERATE_RELATIVE: f64 = 0.5;
/// Fila alta: ≥ 10 ms ou ≥ 2× o mínimo, o que for maior.
pub const QUEUE_HIGH_MS: f64 = 10.0;
pub const QUEUE_HIGH_RELATIVE: f64 = 2.0;
/// Perda: a partir de 1% já é sinal; de 5% em diante é problema.
pub const LOSS_MODERATE_PCT: f64 = 1.0;
pub const LOSS_HIGH_PCT: f64 = 5.0;

/// RTTs (ms) em ordem de envio; `None` é eco sem resposta.
#[derive(Debug, Clone, Default)]
pub struct PingSamples {
    pub small_bytes: usize,
    pub large_bytes: usize,
    pub small: Vec<Option<f64>>,
    pub large: Vec<Option<f64>>,
}

impl PingSamples {
    #[must_use]
    pub fn new(small: Vec<Option<f64>>, large: Vec<Option<f64>>) -> Self {
        Self {
            small_bytes: SMALL_PAYLOAD,
            large_bytes: LARGE_PAYLOAD,
            small,
            large,
        }
    }
}

/// Capacidade (Mbit/s) que um Δmin explica: a diferença de bytes serializada
/// nos dois sentidos, `2 · Δbytes · 8 / Δt`.
#[must_use]
pub fn capacity_mbps(small_bytes: usize, large_bytes: usize, delta_ms: f64) -> f64 {
    let bits = 2.0 * large_bytes.saturating_sub(small_bytes) as f64 * 8.0;
    bits / (delta_ms / 1_000.0) / 1e6
}

/// O teto do método: a capacidade em que o Δmin cai no piso de ruído.
#[must_use]
pub fn ceiling_mbps(small_bytes: usize, large_bytes: usize) -> f64 {
    round_significant(capacity_mbps(small_bytes, large_bytes, NOISE_FLOOR_MS))
}

/// Percentil por posição mais próxima (o mesmo do spike).
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let index = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn received(samples: &[Option<f64>]) -> Vec<f64> {
    samples.iter().flatten().copied().collect()
}

fn min_of(values: &[f64]) -> Option<f64> {
    values.iter().copied().reduce(f64::min)
}

fn rtt_stats(bytes: usize, samples: &[Option<f64>]) -> Option<QuickPingRtt> {
    let mut values = received(samples);
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(QuickPingRtt {
        bytes: u32::try_from(bytes).unwrap_or(u32::MAX),
        sent: count(samples.len()),
        received: count(values.len()),
        min_ms: round_two(values[0]),
        p50_ms: round_two(percentile(&values, 0.5)),
        p95_ms: round_two(percentile(&values, 0.95)),
    })
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Duas casas significativas: a precisão que o método tem (75, 170, 1,2).
fn round_significant(value: f64) -> f64 {
    if value <= 0.0 || !value.is_finite() {
        return 0.0;
    }
    let magnitude = 10_f64.powf(value.log10().floor() - 1.0);
    round_two((value / magnitude).round() * magnitude)
}

/// "190", "1,2" — número de Mbit/s para as frases.
fn mbps_text(value: f64) -> String {
    if value >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}").replace('.', ",")
    }
}

fn pct(lost: usize, sent: usize) -> f64 {
    if sent == 0 {
        return 0.0;
    }
    round_two(lost as f64 * 100.0 / sent as f64)
}

/// Perda que vale para o diagnóstico. Se **nenhum** eco grande voltou, o
/// problema é MTU/filtro (dito na nota da capacidade), não perda no caminho:
/// conta só a dos pequenos.
fn effective_loss(samples: &PingSamples, small_rcv: usize, large_rcv: usize) -> f64 {
    let (small_sent, large_sent) = (samples.small.len(), samples.large.len());
    if large_rcv == 0 && small_rcv > 0 {
        return pct(small_sent - small_rcv, small_sent);
    }
    pct(
        small_sent + large_sent - small_rcv - large_rcv,
        small_sent + large_sent,
    )
}

/// Nível de fila a partir da inflação (p95 − mínimo) e da perda.
#[must_use]
pub fn queueing_level(inflation_ms: f64, min_ms: f64, loss_pct: f64) -> QueueingLevel {
    let high = QUEUE_HIGH_MS.max(min_ms * QUEUE_HIGH_RELATIVE);
    let moderate = QUEUE_MODERATE_MS.max(min_ms * QUEUE_MODERATE_RELATIVE);
    if inflation_ms >= high || loss_pct >= LOSS_HIGH_PCT {
        QueueingLevel::Alto
    } else if inflation_ms >= moderate || loss_pct >= LOSS_MODERATE_PCT {
        QueueingLevel::Moderado
    } else {
        QueueingLevel::Ok
    }
}

/// Δmin por lote contíguo (só os lotes com respostas suficientes).
fn batch_deltas(samples: &PingSamples) -> Vec<f64> {
    (0..BATCHES)
        .filter_map(|batch| {
            let slice = |values: &[Option<f64>]| {
                let n = values.len();
                let (start, end) = (batch * n / BATCHES, (batch + 1) * n / BATCHES);
                received(&values[start..end])
            };
            let (small, large) = (slice(&samples.small), slice(&samples.large));
            if small.len() < MIN_REPLIES_PER_BATCH || large.len() < MIN_REPLIES_PER_BATCH {
                return None;
            }
            Some(min_of(&large)? - min_of(&small)?)
        })
        .collect()
}

enum CapacityOutcome {
    Range(QuickPingCapacity),
    NoReply,
    LargeBlocked,
    FewReplies { small: usize, large: usize },
    TooFast,
    Inconsistent,
}

fn capacity_outcome(
    samples: &PingSamples,
    delta_min: Option<f64>,
    loss_pct: f64,
    level: Option<QueueingLevel>,
) -> CapacityOutcome {
    let small_rcv = received(&samples.small).len();
    let large_rcv = received(&samples.large).len();
    if small_rcv == 0 && large_rcv == 0 {
        return CapacityOutcome::NoReply;
    }
    if large_rcv == 0 {
        return CapacityOutcome::LargeBlocked;
    }
    let few = CapacityOutcome::FewReplies {
        small: small_rcv,
        large: large_rcv,
    };
    if small_rcv < MIN_REPLIES_PER_SIZE || large_rcv < MIN_REPLIES_PER_SIZE {
        return few;
    }
    let Some(delta) = delta_min else {
        return few;
    };
    if delta < NOISE_FLOOR_MS {
        return CapacityOutcome::TooFast;
    }
    let batches = batch_deltas(samples);
    if batches.len() < 2 {
        return few;
    }
    if batches.iter().any(|d| *d <= 0.0) {
        return CapacityOutcome::Inconsistent;
    }
    let to_mbps = |d: f64| capacity_mbps(samples.small_bytes, samples.large_bytes, d);
    let estimates: Vec<f64> = batches
        .iter()
        .copied()
        .chain(std::iter::once(delta))
        .map(to_mbps)
        .collect();
    let low = estimates.iter().copied().fold(f64::INFINITY, f64::min);
    let high = estimates.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let ratio = high / low;
    if ratio > MAX_BATCH_RATIO {
        return CapacityOutcome::Inconsistent;
    }
    let confidence = if ratio > LOW_CONFIDENCE_RATIO
        || loss_pct >= LOSS_HIGH_PCT
        || level == Some(QueueingLevel::Alto)
    {
        CapacityConfidence::MuitoBaixa
    } else {
        CapacityConfidence::Baixa
    };
    CapacityOutcome::Range(QuickPingCapacity {
        low_mbps: round_significant(low * (1.0 - RANGE_MARGIN)),
        high_mbps: round_significant(high * (1.0 + RANGE_MARGIN)),
        confidence,
    })
}

const PATH_NOTE: &str = "Ordem de grandeza do caminho todo, da central até o equipamento — \
inclui a CPU dele respondendo ao ping. Não é a banda de um enlace.";

fn capacity_note(outcome: &CapacityOutcome, samples: &PingSamples) -> String {
    match outcome {
        CapacityOutcome::Range(_) => PATH_NOTE.to_owned(),
        CapacityOutcome::NoReply => "Sem resposta ao ping — nada a estimar.".to_owned(),
        CapacityOutcome::LargeBlocked => format!(
            "O equipamento não respondeu aos pacotes grandes ({} bytes): MTU menor no caminho \
             ou filtro de ICMP. Sem eles não dá para estimar a capacidade.",
            samples.large_bytes
        ),
        CapacityOutcome::FewReplies { small, large } => format!(
            "Poucas respostas para estimar a capacidade ({small} de {} pequenos e {large} de {} \
             grandes).",
            samples.small.len(),
            samples.large.len()
        ),
        CapacityOutcome::TooFast => format!(
            "Acima de ~{} Mbit/s — rápido demais para estimar por ping.",
            mbps_text(ceiling_mbps(samples.small_bytes, samples.large_bytes))
        ),
        CapacityOutcome::Inconsistent => "As medidas variaram demais entre os lotes — caminho \
instável ou rápido demais para estimar por ping."
            .to_owned(),
    }
}

fn verdict(level: Option<QueueingLevel>, outcome: &CapacityOutcome) -> String {
    let Some(level) = level else {
        return "O equipamento não respondeu a nenhum ping. Confira se ele está ligado, se o IP \
                está certo e se responde a ICMP."
            .to_owned();
    };
    let health = match level {
        QueueingLevel::Ok => "Caminho estável: sem perda relevante e sem sinal de fila.",
        QueueingLevel::Moderado => {
            "Sinais moderados de fila ou perda — algum trecho do caminho pode estar perto do \
             limite."
        }
        QueueingLevel::Alto => {
            "Fila alta ou perda relevante — o caminho parece saturado ou instável agora."
        }
    };
    let capacity = match outcome {
        CapacityOutcome::Range(range) => format!(
            " Capacidade na ordem de {}–{} Mbit/s (estimativa de baixa precisão).",
            mbps_text(range.low_mbps),
            mbps_text(range.high_mbps)
        ),
        CapacityOutcome::TooFast => {
            " O caminho é rápido demais para o ping estimar a capacidade.".to_owned()
        }
        _ => " A capacidade não pôde ser estimada por ping.".to_owned(),
    };
    format!("{health}{capacity}")
}

/// O relatório inteiro a partir das amostras.
#[must_use]
pub fn estimate(target: &str, samples: &PingSamples) -> QuickPingReport {
    let small_values = received(&samples.small);
    let large_values = received(&samples.large);
    let (small_rcv, large_rcv) = (small_values.len(), large_values.len());
    let small = rtt_stats(samples.small_bytes, &samples.small);
    let large = rtt_stats(samples.large_bytes, &samples.large);
    let loss_pct = effective_loss(samples, small_rcv, large_rcv);

    let jitter_ms = calculate_jitter(if small_values.len() >= 2 {
        &small_values
    } else {
        &large_values
    });
    // A fila aparece mais nos pacotes grandes (mais tempo na serialização).
    let queueing = large.as_ref().or(small.as_ref()).map(|stats| {
        let inflation_ms = round_two(stats.p95_ms - stats.min_ms);
        QuickPingQueueing {
            inflation_ms,
            level: queueing_level(inflation_ms, stats.min_ms, loss_pct),
        }
    });
    let level = queueing.as_ref().map(|q| q.level);
    let delta_min = match (min_of(&small_values), min_of(&large_values)) {
        (Some(s), Some(l)) => Some(l - s),
        _ => None,
    };
    let outcome = capacity_outcome(samples, delta_min, loss_pct, level);

    QuickPingReport {
        target: target.to_owned(),
        sent: count(samples.small.len() + samples.large.len()),
        samples: count(small_rcv + large_rcv),
        loss_pct,
        small,
        large,
        jitter_ms,
        queueing,
        delta_min_ms: delta_min.map(round_two),
        capacity_note: capacity_note(&outcome, samples),
        verdict: verdict(level, &outcome),
        capacity: match outcome {
            CapacityOutcome::Range(range) => Some(range),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAIRS: usize = 80;

    /// Rodada sintética: mínimo `base` nos pequenos, `base + delta` nos
    /// grandes, com um pouco de fila (`spread`) por cima.
    fn round_with(base: f64, delta_per_batch: [f64; 3], spread: f64) -> PingSamples {
        let mut small = Vec::with_capacity(PAIRS);
        let mut large = Vec::with_capacity(PAIRS);
        for i in 0..PAIRS {
            let batch = i * BATCHES / PAIRS;
            let noise = spread * ((i % 7) as f64 / 6.0);
            small.push(Some(base + noise));
            large.push(Some(base + delta_per_batch[batch] + noise));
        }
        PingSamples::new(small, large)
    }

    #[test]
    fn formula_bate_com_o_spike() {
        // MikroTik: Δmin 0,16 ms → ~142 Mbit/s no spike.
        let mbps = capacity_mbps(SMALL_PAYLOAD, LARGE_PAYLOAD, 0.16);
        assert!((mbps - 141.6).abs() < 0.1, "{mbps}");
        assert_eq!(ceiling_mbps(SMALL_PAYLOAD, LARGE_PAYLOAD), 190.0);
    }

    #[test]
    fn enlace_rapido_nao_da_numero() {
        // BPI-R3: Δmin 0,05 ms — abaixo do piso de ruído.
        let report = estimate("10.0.0.2", &round_with(0.6, [0.05, 0.05, 0.05], 0.3));
        assert!(report.capacity.is_none());
        assert!(
            report.capacity_note.contains("Acima de ~190 Mbit/s"),
            "{}",
            report.capacity_note
        );
        assert!(report.verdict.contains("rápido demais"));
        assert_eq!(report.loss_pct, 0.0);
    }

    #[test]
    fn enlace_lento_da_faixa_alargada() {
        // MikroTik: lotes com 0,16 / 0,20 / 0,22 ms → 103–142 Mbit/s, ±20%.
        let report = estimate("10.0.0.242", &round_with(0.8, [0.16, 0.20, 0.22], 0.4));
        let capacity = report.capacity.expect("faixa");
        assert_eq!(capacity.low_mbps, 82.0);
        assert_eq!(capacity.high_mbps, 170.0);
        assert_eq!(capacity.confidence, CapacityConfidence::Baixa);
        assert_eq!(report.delta_min_ms, Some(0.16));
        assert!(
            report.verdict.contains("82–170 Mbit/s"),
            "{}",
            report.verdict
        );
    }

    #[test]
    fn lotes_discordantes_nao_dao_numero() {
        // 0,15 ms num lote e 0,40 no outro: 2,7× de diferença.
        let report = estimate("10.0.0.3", &round_with(0.8, [0.15, 0.40, 0.25], 0.2));
        assert!(report.capacity.is_none());
        assert!(report.capacity_note.contains("variaram demais"));
    }

    #[test]
    fn lotes_um_pouco_discordantes_baixam_a_confianca() {
        let report = estimate("10.0.0.3", &round_with(0.8, [0.15, 0.25, 0.20], 0.2));
        let capacity = report.capacity.expect("faixa");
        assert_eq!(capacity.confidence, CapacityConfidence::MuitoBaixa);
    }

    #[test]
    fn sem_resposta_nao_inventa_nada() {
        let samples = PingSamples::new(vec![None; PAIRS], vec![None; PAIRS]);
        let report = estimate("10.0.0.9", &samples);
        assert_eq!(report.samples, 0);
        assert_eq!(report.sent, 160);
        assert_eq!(report.loss_pct, 100.0);
        assert!(report.small.is_none() && report.large.is_none());
        assert!(report.queueing.is_none());
        assert!(report.capacity.is_none());
        assert!(report.verdict.contains("não respondeu"));
    }

    #[test]
    fn pacote_grande_bloqueado_vira_nota_de_mtu_sem_contar_como_perda() {
        let small = (0..PAIRS).map(|_| Some(0.9)).collect();
        let samples = PingSamples::new(small, vec![None; PAIRS]);
        let report = estimate("10.0.0.9", &samples);
        assert!(report.capacity.is_none());
        assert!(report.large.is_none());
        assert!(report.capacity_note.contains("MTU"));
        assert_eq!(report.loss_pct, 0.0);
        assert_eq!(report.queueing.unwrap().level, QueueingLevel::Ok);
    }

    #[test]
    fn poucas_respostas_nao_estimam() {
        let mut samples = round_with(0.8, [0.2, 0.2, 0.2], 0.1);
        for value in samples.large.iter_mut().skip(10) {
            *value = None;
        }
        let report = estimate("10.0.0.9", &samples);
        assert!(report.capacity.is_none());
        assert!(report.capacity_note.contains("Poucas respostas"));
        assert_eq!(report.queueing.unwrap().level, QueueingLevel::Alto);
    }

    #[test]
    fn niveis_de_fila_e_perda() {
        assert_eq!(queueing_level(0.5, 1.0, 0.0), QueueingLevel::Ok);
        // Spike: p95 ~5 ms sobre ~1 ms de mínimo.
        assert_eq!(queueing_level(4.0, 1.0, 0.0), QueueingLevel::Moderado);
        assert_eq!(queueing_level(12.0, 1.0, 0.0), QueueingLevel::Alto);
        // Caminho longo: 8 ms acima de 40 ms de mínimo ainda é normal.
        assert_eq!(queueing_level(8.0, 40.0, 0.0), QueueingLevel::Ok);
        assert_eq!(queueing_level(0.5, 1.0, 1.25), QueueingLevel::Moderado);
        assert_eq!(queueing_level(0.5, 1.0, 6.0), QueueingLevel::Alto);
    }

    #[test]
    fn fila_alta_derruba_a_confianca() {
        let mut samples = round_with(0.8, [0.18, 0.2, 0.22], 0.1);
        // Algumas rajadas de 15 ms nos grandes (fila), sem mexer no mínimo.
        for value in samples.large.iter_mut().step_by(8) {
            *value = Some(15.0);
        }
        let report = estimate("10.0.0.9", &samples);
        assert_eq!(report.queueing.as_ref().unwrap().level, QueueingLevel::Alto);
        assert_eq!(
            report.capacity.expect("faixa").confidence,
            CapacityConfidence::MuitoBaixa
        );
        assert!(report.verdict.contains("saturado"));
    }

    #[test]
    fn estatisticas_e_jitter() {
        let small = vec![Some(1.0), Some(2.0), Some(1.0), None];
        let large = vec![Some(3.0), Some(4.0), None, None];
        let report = estimate("127.0.0.1", &PingSamples::new(small, large));
        let s = report.small.unwrap();
        assert_eq!((s.sent, s.received, s.min_ms, s.p50_ms), (4, 3, 1.0, 1.0));
        assert_eq!(report.jitter_ms, Some(1.0));
        assert_eq!(report.loss_pct, 37.5);
        assert_eq!(report.samples, 5);
    }

    #[test]
    fn arredonda_em_duas_casas_significativas() {
        assert_eq!(round_significant(75.2), 75.0);
        assert_eq!(round_significant(169.9), 170.0);
        assert_eq!(round_significant(1.234), 1.2);
        assert_eq!(round_significant(0.0), 0.0);
        assert_eq!(mbps_text(1.2), "1,2");
    }
}
