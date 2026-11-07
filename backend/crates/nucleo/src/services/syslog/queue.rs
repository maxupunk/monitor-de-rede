//! A fila entre os listeners e o escritor em lote, com admissão e contadores.
//!
//! **A fila cheia descarta o mais novo, não o mais antigo.** O rascunho do
//! roadmap pedia o contrário, mas `mpsc::Sender::try_send` devolve
//! `TrySendError::Full(msg)` — o que se tem em mãos para descartar é a
//! mensagem nova. Descartar a mais antiga exigiria um ring buffer próprio, com
//! `Mutex<VecDeque>` e `Notify`, pagando complexidade justamente no caminho
//! quente. A diferença semântica é nula na prática: a fila só enche durante uma
//! rajada, e nessa rajada se perde do mesmo lote de qualquer jeito.
//!
//! **O que não muda é que perda nunca é silenciosa**: todo descarte incrementa
//! um contador exposto.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use super::parser::ParsedLog;

/// De onde a linha veio.
///
/// A mesma fila serve às duas origens — é essa a decisão da Fase 4 do roadmap
/// do servidor como dispositivo. Um segundo pipeline dentro do mesmo processo
/// seria fila, escritor, barramento, retenção e busca duplicados para gravar na
/// mesma tabela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogSource {
    /// Recebida pela rede, pelo listener de syslog.
    #[default]
    Syslog,
    /// Emitida por este processo, pela camada de `tracing`.
    Application,
}

impl LogSource {
    /// Forma persistida em `device_logs.source`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syslog => "syslog",
            Self::Application => "application",
        }
    }
}

/// Uma linha resolvida, a caminho do escritor.
#[derive(Debug, Clone)]
pub struct PendingLog {
    pub device_id: Option<i64>,
    pub source_ip: String,
    pub received_at: DateTime<Utc>,
    pub parsed: ParsedLog,
    pub source: LogSource,
}

/// Contadores de ingestão. Todos monotônicos desde o boot.
#[derive(Debug, Default)]
pub struct IngestMetrics {
    received: AtomicU64,
    queued: AtomicU64,
    dropped_queue_full: AtomicU64,
    dropped_rate_limit: AtomicU64,
    dropped_unknown_source: AtomicU64,
    dropped_oversized: AtomicU64,
}

/// Leitura instantânea dos contadores, para log e para a API.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestSnapshot {
    pub received: u64,
    pub queued: u64,
    pub dropped_queue_full: u64,
    pub dropped_rate_limit: u64,
    pub dropped_unknown_source: u64,
    pub dropped_oversized: u64,
}

impl IngestSnapshot {
    #[must_use]
    pub const fn dropped_total(&self) -> u64 {
        self.dropped_queue_full
            + self.dropped_rate_limit
            + self.dropped_unknown_source
            + self.dropped_oversized
    }
}

impl IngestMetrics {
    pub fn record_received(&self) {
        self.received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_queued(&self) {
        self.queued.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_queue_full(&self) {
        self.dropped_queue_full.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_rate_limited(&self) {
        self.dropped_rate_limit.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_unknown_source(&self) {
        self.dropped_unknown_source.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_oversized(&self) {
        self.dropped_oversized.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn snapshot(&self) -> IngestSnapshot {
        IngestSnapshot {
            received: self.received.load(Ordering::Relaxed),
            queued: self.queued.load(Ordering::Relaxed),
            dropped_queue_full: self.dropped_queue_full.load(Ordering::Relaxed),
            dropped_rate_limit: self.dropped_rate_limit.load(Ordering::Relaxed),
            dropped_unknown_source: self.dropped_unknown_source.load(Ordering::Relaxed),
            dropped_oversized: self.dropped_oversized.load(Ordering::Relaxed),
        }
    }
}

/// A ponta de escrita da fila.
#[derive(Clone)]
pub struct LogQueue {
    sender: mpsc::Sender<PendingLog>,
    metrics: Arc<IngestMetrics>,
}

impl LogQueue {
    /// Cria a fila e devolve a ponta de leitura, que vai para o escritor.
    #[must_use]
    pub fn create(
        capacity: usize,
        metrics: Arc<IngestMetrics>,
    ) -> (Self, mpsc::Receiver<PendingLog>) {
        let (sender, receiver) = mpsc::channel(capacity);
        (Self { sender, metrics }, receiver)
    }

    /// Enfileira sem bloquear. Devolve `false` quando descartou.
    ///
    /// Nunca `await`: o listener UDP não pode parar de ler o socket para
    /// esperar o escritor — a fila do kernel encheria e o descarte passaria a
    /// ser invisível, fora dos nossos contadores.
    pub fn try_enqueue(&self, log: PendingLog) -> bool {
        match self.sender.try_send(log) {
            Ok(()) => {
                self.metrics.record_queued();
                true
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.metrics.record_queue_full();
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    #[must_use]
    pub fn metrics(&self) -> &Arc<IngestMetrics> {
        &self.metrics
    }
}

/// Quantas vezes a rajada excede o limite sustentado.
///
/// Roteador que reiniciou despeja o buffer de uma vez; sem folga, esse despejo
/// — que é justamente o que interessa ler — seria o primeiro a ser cortado.
const BURST_FACTOR: f64 = 4.0;

/// Teto de fontes rastreadas ao mesmo tempo.
///
/// Sem teto, um remetente que forja o IP de origem faria o mapa crescer sem
/// limite: o limitador viraria o vazamento de memória que deveria impedir.
const MAX_TRACKED_SOURCES: usize = 4096;

struct Bucket {
    tokens: f64,
    last: Instant,
}

/// Balde de fichas por fonte.
///
/// O limite é **por origem**, não global: a 200/s globais, uma única fonte
/// defeituosa consumiria sozinha todo o orçamento e calaria o parque inteiro.
pub struct RateLimiter {
    per_second: f64,
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl RateLimiter {
    #[must_use]
    pub fn new(per_second: u32) -> Self {
        Self {
            per_second: f64::from(per_second.max(1)),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// `true` se a mensagem pode passar.
    pub fn allow(&self, source: &str) -> bool {
        self.allow_at(source, Instant::now())
    }

    /// A mesma decisão com o relógio injetado, para o teste não dormir.
    fn allow_at(&self, source: &str, agora: Instant) -> bool {
        let capacidade = self.per_second * BURST_FACTOR;
        // Lock envenenado só acontece se outra thread entrou em pânico dentro
        // desta seção crítica, que é aritmética pura. Recuperar é mais correto
        // do que derrubar a ingestão junto.
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if buckets.len() >= MAX_TRACKED_SOURCES && !buckets.contains_key(source) {
            // Descarta o que já está cheio de fichas: são as fontes que não
            // estão consumindo nada, logo as que menos perdem com o despejo.
            buckets.retain(|_, bucket| bucket.tokens < capacidade);
            if buckets.len() >= MAX_TRACKED_SOURCES {
                buckets.clear();
            }
        }

        let bucket = buckets.entry(source.to_owned()).or_insert(Bucket {
            tokens: capacidade,
            last: agora,
        });
        let decorrido = agora.saturating_duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + decorrido * self.per_second).min(capacidade);
        bucket.last = agora;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn log() -> PendingLog {
        PendingLog {
            device_id: Some(1),
            source_ip: "192.168.88.1".into(),
            received_at: Utc::now(),
            parsed: ParsedLog {
                message: "teste".into(),
                ..ParsedLog::default()
            },
            source: LogSource::Syslog,
        }
    }

    #[tokio::test]
    async fn a_fila_cheia_descarta_e_conta() {
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, _receiver) = LogQueue::create(2, Arc::clone(&metrics));
        assert!(queue.try_enqueue(log()));
        assert!(queue.try_enqueue(log()));
        // Terceira não cabe.
        assert!(!queue.try_enqueue(log()));

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.queued, 2);
        assert_eq!(snapshot.dropped_queue_full, 1, "perda tem de ser visível");
        assert_eq!(snapshot.dropped_total(), 1);
    }

    #[tokio::test]
    async fn fila_fechada_nao_conta_como_estouro() {
        // Receptor caído é desligamento, não pressão: contar aqui inflaria a
        // métrica que o operador usa para dimensionar a fila.
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, receiver) = LogQueue::create(4, Arc::clone(&metrics));
        drop(receiver);
        assert!(!queue.try_enqueue(log()));
        assert_eq!(metrics.snapshot().dropped_queue_full, 0);
    }

    #[test]
    fn o_limitador_permite_a_rajada_e_depois_segura() {
        let limitador = RateLimiter::new(10);
        let inicio = Instant::now();
        // Capacidade = 10 × 4 = 40 fichas.
        for indice in 0..40 {
            assert!(limitador.allow_at("10.0.0.1", inicio), "ficha {indice}");
        }
        assert!(!limitador.allow_at("10.0.0.1", inicio), "rajada esgotada");
    }

    #[test]
    fn as_fichas_voltam_com_o_tempo() {
        let limitador = RateLimiter::new(10);
        let inicio = Instant::now();
        for _ in 0..40 {
            limitador.allow_at("10.0.0.1", inicio);
        }
        assert!(!limitador.allow_at("10.0.0.1", inicio));
        // Meio segundo a 10/s devolve 5 fichas.
        let depois = inicio + Duration::from_millis(500);
        for indice in 0..5 {
            assert!(limitador.allow_at("10.0.0.1", depois), "recarga {indice}");
        }
        assert!(!limitador.allow_at("10.0.0.1", depois));
    }

    #[test]
    fn uma_fonte_barulhenta_nao_cala_as_outras() {
        let limitador = RateLimiter::new(1);
        let inicio = Instant::now();
        while limitador.allow_at("10.0.0.1", inicio) {}
        assert!(
            limitador.allow_at("10.0.0.2", inicio),
            "o limite é por origem"
        );
    }

    #[test]
    fn o_mapa_de_fontes_tem_teto() {
        // IP forjado não pode transformar o limitador num vazamento de memória.
        let limitador = RateLimiter::new(10);
        let inicio = Instant::now();
        for indice in 0..(MAX_TRACKED_SOURCES + 500) {
            limitador.allow_at(&format!("10.1.{}.{}", indice / 256, indice % 256), inicio);
        }
        let total = limitador
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        assert!(total <= MAX_TRACKED_SOURCES, "mapa cresceu para {total}");
    }
}
