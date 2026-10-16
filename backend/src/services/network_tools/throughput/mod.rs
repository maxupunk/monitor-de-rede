//! Motores nativos de teste de enlace (ADR 015 §1).
//!
//! Rust puro, sem processo externo e testável em loopback:
//!
//! * [`iperf3`] — **cliente** do protocolo iperf3 (TCP), compatível com o
//!   `iperf3 -s` 3.x;
//! * [`btest`] — protocolo MikroTik Bandwidth Test (TCP, porta 2000):
//!   **servidor** armado (central) e **cliente** (agente), com o mesmo codec.
//!
//! Este módulo guarda só o que os dois motores compartilham: o contrato
//! (`ThroughputSpec` → `ThroughputReport`), as amostras de 1 s e os erros
//! comuns. **Sentido sempre do ponto de vista de quem DIRIGE o teste**:
//! `tx` = quem dirige envia, `rx` = quem dirige recebe. O orquestrador traduz
//! para o sentido do enlace.

pub mod btest;
pub mod iperf3;
mod meter;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use ts_rs::TS;

pub use meter::SampleSink;

use super::icmp_probe::round_two;
use crate::services::shared::errors::{AppError, AppResult};

/// Duração mínima de um sentido do teste, em segundos.
pub const MIN_DURATION_S: u8 = 1;
/// Duração máxima de um sentido do teste, em segundos.
pub const MAX_DURATION_S: u8 = 60;
/// Mínimo de fluxos TCP paralelos.
pub const MIN_STREAMS: u8 = 1;
/// Máximo de fluxos TCP paralelos.
pub const MAX_STREAMS: u8 = 8;
/// Prazo de cada passo de conexão e de handshake.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Folga somada à duração do teste para o prazo total de uma execução.
pub const DEADLINE_MARGIN: Duration = Duration::from_secs(15);
/// Mensagem do erro devolvido quando o teste é cancelado.
pub const CANCELLED_MESSAGE: &str = "teste de enlace cancelado";

/// Motor que mede o enlace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ThroughputEngine {
    Iperf3,
    Btest,
}

impl ThroughputEngine {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Iperf3 => "iperf3",
            Self::Btest => "btest",
        }
    }
}

/// Sentido do teste, do ponto de vista de quem **dirige**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum TestDirection {
    /// Quem dirige envia.
    Send,
    /// Quem dirige recebe.
    Receive,
    /// Os dois sentidos.
    Both,
}

impl TestDirection {
    /// Quem dirige transmite neste sentido.
    #[must_use]
    pub const fn sends(self) -> bool {
        matches!(self, Self::Send | Self::Both)
    }

    /// Quem dirige recebe neste sentido.
    #[must_use]
    pub const fn receives(self) -> bool {
        matches!(self, Self::Receive | Self::Both)
    }
}

/// Protocolo de transporte. UDP fica para a segunda etapa (ADR 015).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum TestProtocol {
    #[default]
    Tcp,
}

/// O que medir e como. Valide com [`ThroughputSpec::validate`] antes de usar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ThroughputSpec {
    pub engine: ThroughputEngine,
    pub direction: TestDirection,
    #[serde(default)]
    pub protocol: TestProtocol,
    /// Duração de **cada** sentido, 1–60 s.
    pub duration_s: u8,
    /// Fluxos TCP paralelos, 1–8.
    pub streams: u8,
    /// Teto de banda total, em Mbit/s. `None` = sem limite.
    #[serde(default)]
    pub limit_mbps: Option<u32>,
}

impl ThroughputSpec {
    /// Confere os limites do contrato.
    ///
    /// # Errors
    ///
    /// [`AppError::Validation`] com o primeiro campo fora da faixa.
    pub fn validate(&self) -> AppResult<()> {
        if !(MIN_DURATION_S..=MAX_DURATION_S).contains(&self.duration_s) {
            return Err(AppError::validation(format!(
                "A duração do teste deve ficar entre {MIN_DURATION_S} e {MAX_DURATION_S} segundos"
            )));
        }
        if !(MIN_STREAMS..=MAX_STREAMS).contains(&self.streams) {
            return Err(AppError::validation(format!(
                "O número de fluxos deve ficar entre {MIN_STREAMS} e {MAX_STREAMS}"
            )));
        }
        if self.limit_mbps == Some(0) {
            return Err(AppError::validation(
                "O limite de banda deve ser maior que zero (omita para não limitar)",
            ));
        }
        Ok(())
    }

    /// Duração de um sentido do teste.
    #[must_use]
    pub fn duration(&self) -> Duration {
        Duration::from_secs(u64::from(self.duration_s))
    }

    /// Teto de banda total em bit/s, quando houver.
    #[must_use]
    pub fn limit_bps(&self) -> Option<u64> {
        self.limit_mbps.map(|mbps| u64::from(mbps) * 1_000_000)
    }

    /// Confere o motor esperado pelo executor e os limites do contrato.
    pub(crate) fn validate_for(&self, engine: ThroughputEngine) -> AppResult<()> {
        if self.engine != engine {
            return Err(AppError::validation(format!(
                "Especificação do motor {} entregue ao motor {}",
                self.engine.as_str(),
                engine.as_str()
            )));
        }
        self.validate()
    }
}

/// Fluxo de uma amostra, do ponto de vista de quem dirige.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum Flow {
    /// Quem dirige envia.
    Tx,
    /// Quem dirige recebe.
    Rx,
}

/// Vazão medida num intervalo de ~1 s.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ThroughputSample {
    /// Fim do intervalo, em ms desde o início do teste.
    #[ts(type = "number")]
    pub t_ms: u64,
    pub flow: Flow,
    pub bps: f64,
}

/// Resultado de um teste, do ponto de vista de quem dirige.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ThroughputReport {
    pub engine: ThroughputEngine,
    /// Vazão média do sentido em que quem dirige envia; `None` se não medido.
    pub tx_bps: Option<f64>,
    /// Vazão média do sentido em que quem dirige recebe; `None` se não medido.
    pub rx_bps: Option<f64>,
    #[ts(type = "number")]
    pub tx_bytes: u64,
    #[ts(type = "number")]
    pub rx_bytes: u64,
    /// Retransmissões TCP informadas pelo lado que enviou, quando houver.
    #[ts(type = "number | null")]
    pub retransmits: Option<u64>,
    pub samples: Vec<ThroughputSample>,
}

impl ThroughputReport {
    /// Relatório vazio do motor, para ser preenchido sentido a sentido.
    #[must_use]
    pub fn empty(engine: ThroughputEngine) -> Self {
        Self {
            engine,
            tx_bps: None,
            rx_bps: None,
            tx_bytes: 0,
            rx_bytes: 0,
            retransmits: None,
            samples: Vec::new(),
        }
    }

    /// Registra o sentido `tx` medido em `elapsed`.
    pub(crate) fn set_tx(&mut self, bytes: u64, elapsed: Duration) {
        self.tx_bytes = bytes;
        self.tx_bps = bits_per_second(bytes, elapsed);
    }

    /// Registra o sentido `rx` medido em `elapsed`.
    pub(crate) fn set_rx(&mut self, bytes: u64, elapsed: Duration) {
        self.rx_bytes = bytes;
        self.rx_bps = bits_per_second(bytes, elapsed);
    }
}

/// Cria o canal de amostras: o emissor vai ao motor, o receptor a quem exibe.
#[must_use]
pub fn sample_channel() -> (SampleSink, mpsc::UnboundedReceiver<ThroughputSample>) {
    mpsc::unbounded_channel()
}

/// Vazão em bit/s, com duas casas. `None` quando o intervalo é nulo.
#[must_use]
pub fn bits_per_second(bytes: u64, elapsed: Duration) -> Option<f64> {
    let secs = elapsed.as_secs_f64();
    (secs > 0.0).then(|| round_two(bytes as f64 * 8.0 / secs))
}

/// Milissegundos inteiros de um intervalo, saturando em `u64::MAX`.
#[must_use]
pub fn elapsed_ms(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

/// Erro padrão de teste cancelado (ver [`CANCELLED_MESSAGE`]).
pub(crate) fn cancelled() -> AppError {
    AppError::business_rule(CANCELLED_MESSAGE)
}

/// O erro é o de teste cancelado.
#[must_use]
pub fn is_cancelled(error: &AppError) -> bool {
    matches!(error, AppError::BusinessRule(msg) if msg == CANCELLED_MESSAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ThroughputSpec {
        ThroughputSpec {
            engine: ThroughputEngine::Iperf3,
            direction: TestDirection::Both,
            protocol: TestProtocol::Tcp,
            duration_s: 5,
            streams: 1,
            limit_mbps: None,
        }
    }

    #[test]
    fn spec_valida_limites() {
        assert!(spec().validate().is_ok());
        for duration_s in [0, 61] {
            let s = ThroughputSpec {
                duration_s,
                ..spec()
            };
            assert!(matches!(s.validate(), Err(AppError::Validation(_))));
        }
        for streams in [0, 9] {
            let s = ThroughputSpec { streams, ..spec() };
            assert!(matches!(s.validate(), Err(AppError::Validation(_))));
        }
        let s = ThroughputSpec {
            limit_mbps: Some(0),
            ..spec()
        };
        assert!(s.validate().is_err());
        let s = ThroughputSpec {
            duration_s: 60,
            streams: 8,
            limit_mbps: Some(1),
            ..spec()
        };
        assert!(s.validate().is_ok());
    }

    #[test]
    fn spec_recusa_motor_trocado() {
        let s = spec();
        assert!(s.validate_for(ThroughputEngine::Iperf3).is_ok());
        assert!(s.validate_for(ThroughputEngine::Btest).is_err());
    }

    #[test]
    fn spec_converte_duracao_e_limite() {
        let s = ThroughputSpec {
            limit_mbps: Some(250),
            ..spec()
        };
        assert_eq!(s.duration(), Duration::from_secs(5));
        assert_eq!(s.limit_bps(), Some(250_000_000));
        assert_eq!(spec().limit_bps(), None);
    }

    #[test]
    fn spec_serializa_em_camel_case() {
        let json = serde_json::to_value(ThroughputSpec {
            limit_mbps: Some(10),
            ..spec()
        })
        .expect("serializa");
        assert_eq!(
            json,
            serde_json::json!({
                "engine": "iperf3",
                "direction": "both",
                "protocol": "tcp",
                "durationS": 5,
                "streams": 1,
                "limitMbps": 10
            })
        );
        let lido: ThroughputSpec = serde_json::from_value(serde_json::json!({
            "engine": "btest", "direction": "send", "durationS": 3, "streams": 1
        }))
        .expect("desserializa");
        assert_eq!(lido.protocol, TestProtocol::Tcp);
        assert_eq!(lido.limit_mbps, None);
        assert_eq!(lido.engine, ThroughputEngine::Btest);
    }

    #[test]
    fn direcao_diz_quem_envia_e_quem_recebe() {
        assert!(TestDirection::Send.sends() && !TestDirection::Send.receives());
        assert!(!TestDirection::Receive.sends() && TestDirection::Receive.receives());
        assert!(TestDirection::Both.sends() && TestDirection::Both.receives());
    }

    #[test]
    fn calcula_bits_por_segundo() {
        assert_eq!(
            bits_per_second(1_000, Duration::from_secs(1)),
            Some(8_000.0)
        );
        assert_eq!(
            bits_per_second(125_000, Duration::from_millis(500)),
            Some(2_000_000.0)
        );
        assert_eq!(bits_per_second(10, Duration::ZERO), None);
    }

    #[test]
    fn relatorio_registra_sentidos() {
        let mut r = ThroughputReport::empty(ThroughputEngine::Btest);
        r.set_tx(2_000, Duration::from_secs(2));
        assert_eq!(
            (r.tx_bytes, r.tx_bps, r.rx_bps),
            (2_000, Some(8_000.0), None)
        );
        r.set_rx(0, Duration::ZERO);
        assert_eq!(r.rx_bps, None);
        let json = serde_json::to_value(&r).expect("serializa");
        assert_eq!(json["txBps"], 8_000.0);
        assert!(json["rxBps"].is_null());
        assert_eq!(json["engine"], "btest");
    }

    #[test]
    fn identifica_cancelamento() {
        assert!(is_cancelled(&cancelled()));
        assert!(!is_cancelled(&AppError::business_rule("outro")));
        assert_eq!(elapsed_ms(Duration::from_millis(1_234)), 1_234);
    }
}
