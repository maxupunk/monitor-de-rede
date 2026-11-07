//! Medição comum aos motores: contadores de bytes, amostrador de 1 s e
//! cadência de envio (limite de banda).
//!
//! Os motores só somam bytes em contadores atômicos; quem transforma isso em
//! amostras é o [`Sampler`], um único laço por execução. Assim nenhum laço de
//! E/S precisa saber de relógio, e o mesmo código serve iperf3 e btest.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{interval_at, Instant, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;

use super::{bits_per_second, elapsed_ms, Flow, ThroughputSample};

/// Destino das amostras de 1 s. Enviar para um receptor já descartado não é
/// erro: o teste segue e o relatório final continua trazendo as amostras.
pub type SampleSink = mpsc::UnboundedSender<ThroughputSample>;

/// Intervalo entre amostras.
pub(crate) const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// Contador de bytes compartilhado entre o laço de E/S e o amostrador.
#[derive(Debug, Clone, Default)]
pub(crate) struct ByteCounter(Arc<AtomicU64>);

impl ByteCounter {
    pub(crate) fn add(&self, bytes: usize) {
        self.0.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub(crate) fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Laço que lê os contadores a cada segundo e emite uma amostra por fluxo.
pub(crate) struct Sampler {
    stop: CancellationToken,
    handle: Option<JoinHandle<Vec<ThroughputSample>>>,
}

impl Sampler {
    /// Começa a amostrar agora. `offset_ms` desloca o `t_ms` (segunda fase de
    /// um teste nos dois sentidos).
    pub(crate) fn spawn(flows: Vec<(Flow, ByteCounter)>, sink: SampleSink, offset_ms: u64) -> Self {
        let stop = CancellationToken::new();
        let token = stop.clone();
        // Início e linha de base fotografados **agora**, não quando a tarefa
        // rodar pela primeira vez: o que for somado depois desta chamada
        // pertence à primeira amostra.
        let start = Instant::now();
        let baseline: Vec<u64> = flows.iter().map(|(_, c)| c.get()).collect();
        let handle = tokio::spawn(async move {
            let mut window = Window {
                flows,
                last: baseline,
                last_at: start,
                start,
                offset_ms,
                sink,
                samples: Vec::new(),
            };
            let mut ticker = interval_at(start + SAMPLE_INTERVAL, SAMPLE_INTERVAL);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    () = token.cancelled() => break,
                    now = ticker.tick() => window.emit(now),
                }
            }
            // O intervalo parcial do fim vira amostra quando é longo o
            // bastante para significar algo — senão um teste de 1 s, que
            // termina junto com o primeiro tique, poderia sair sem nenhuma.
            let now = Instant::now();
            if now.saturating_duration_since(window.last_at) >= MIN_FINAL_INTERVAL {
                window.emit(now);
            }
            window.samples
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }

    /// Para de amostrar e devolve as amostras emitidas, inclusive a do
    /// intervalo parcial final quando ele passa de 200 ms.
    pub(crate) async fn finish(mut self) -> Vec<ThroughputSample> {
        self.stop.cancel();
        match self.handle.take() {
            Some(handle) => handle.await.unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

/// Menor intervalo final que ainda vira amostra.
const MIN_FINAL_INTERVAL: Duration = Duration::from_millis(200);

/// Estado do amostrador entre um tique e outro.
struct Window {
    flows: Vec<(Flow, ByteCounter)>,
    last: Vec<u64>,
    last_at: Instant,
    start: Instant,
    offset_ms: u64,
    sink: SampleSink,
    samples: Vec<ThroughputSample>,
}

impl Window {
    /// Uma amostra por fluxo com o que passou desde a anterior.
    fn emit(&mut self, now: Instant) {
        let interval = now.saturating_duration_since(self.last_at);
        self.last_at = now;
        let t_ms = self.offset_ms.saturating_add(elapsed_ms(now - self.start));
        for ((flow, counter), previous) in self.flows.iter().zip(self.last.iter_mut()) {
            let current = counter.get();
            let delta = current.saturating_sub(*previous);
            *previous = current;
            let sample = ThroughputSample {
                t_ms,
                flow: *flow,
                bps: bits_per_second(delta, interval).unwrap_or(0.0),
            };
            // Receptor fechado não interrompe o teste.
            let _ = self.sink.send(sample);
            self.samples.push(sample);
        }
    }
}

/// Quanto esperar para que `sent_bytes` caibam em `bps` desde o início.
/// `None` quando já se está dentro (ou atrás) do ritmo.
pub(crate) fn pace_delay(sent_bytes: u64, bps: u64, elapsed: Duration) -> Option<Duration> {
    if bps == 0 {
        return None;
    }
    let target = Duration::from_secs_f64(sent_bytes as f64 * 8.0 / bps as f64);
    target.checked_sub(elapsed).filter(|wait| !wait.is_zero())
}

/// Cadência de envio para um teto de banda. Sem teto, não espera nunca.
pub(crate) struct Pacer {
    bps: Option<u64>,
    start: Instant,
    sent: u64,
}

impl Pacer {
    pub(crate) fn new(bps: Option<u64>) -> Self {
        Self {
            bps: bps.filter(|b| *b > 0),
            start: Instant::now(),
            sent: 0,
        }
    }

    /// Contabiliza `bytes` enviados e espera o necessário para o teto.
    pub(crate) async fn pace(&mut self, bytes: usize) {
        let Some(bps) = self.bps else {
            return;
        };
        self.sent = self.sent.saturating_add(bytes as u64);
        if let Some(wait) = pace_delay(self.sent, bps, self.start.elapsed()) {
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pace_delay_respeita_o_teto() {
        // 1 Mbit/s: 125 000 bytes levam 1 s.
        assert_eq!(
            pace_delay(125_000, 1_000_000, Duration::from_millis(400)),
            Some(Duration::from_millis(600))
        );
        assert_eq!(pace_delay(125_000, 1_000_000, Duration::from_secs(2)), None);
        assert_eq!(pace_delay(125_000, 0, Duration::ZERO), None);
    }

    #[test]
    fn contador_soma() {
        let c = ByteCounter::default();
        c.add(10);
        c.clone().add(5);
        assert_eq!(c.get(), 15);
    }

    #[tokio::test]
    async fn amostrador_emite_um_por_segundo_por_fluxo() {
        let tx = ByteCounter::default();
        let rx = ByteCounter::default();
        let (sink, mut recebidas) = super::super::sample_channel();
        let sampler = Sampler::spawn(
            vec![(Flow::Tx, tx.clone()), (Flow::Rx, rx.clone())],
            sink,
            500,
        );
        tx.add(125_000);
        tokio::time::sleep(Duration::from_millis(1_300)).await;
        rx.add(250_000);
        tokio::time::sleep(Duration::from_millis(1_000)).await;
        let amostras = sampler.finish().await;
        // Dois tiques e o intervalo final de ~300 ms: 3 rodadas de 2 fluxos.
        assert_eq!(amostras.len(), 6, "{amostras:?}");
        assert!(amostras[4].t_ms > amostras[2].t_ms);
        assert_eq!((amostras[0].flow, amostras[1].flow), (Flow::Tx, Flow::Rx));
        // O tique real chega com alguns ms de atraso: compara com folga.
        assert!((1_500..1_700).contains(&amostras[0].t_ms), "{amostras:?}");
        assert!(
            (amostras[0].bps - 1_000_000.0).abs() < 50_000.0,
            "{amostras:?}"
        );
        assert_eq!(amostras[1].bps, 0.0);
        assert_eq!(amostras[2].bps, 0.0);
        assert!(
            (amostras[3].bps - 2_000_000.0).abs() < 100_000.0,
            "{amostras:?}"
        );
        let mut emitidas = Vec::new();
        while let Ok(s) = recebidas.try_recv() {
            emitidas.push(s);
        }
        assert_eq!(emitidas, amostras);
    }

    #[tokio::test]
    async fn pacer_segura_o_ritmo() {
        let mut pacer = Pacer::new(Some(8_000));
        let inicio = Instant::now();
        pacer.pace(100).await; // 100 bytes a 8 kbit/s = 100 ms
        assert!(inicio.elapsed() >= Duration::from_millis(95));
        let mut livre = Pacer::new(None);
        let inicio = Instant::now();
        livre.pace(1_000_000).await;
        assert!(inicio.elapsed() < Duration::from_millis(50));
    }
}
