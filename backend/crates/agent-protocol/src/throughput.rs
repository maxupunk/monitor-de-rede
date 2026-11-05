//! A especificação de um teste de vazão, como viaja no `Command::Throughput`
//! (ADR 015 §6). A validação com a mensagem ao operador fica no motor
//! (`network_tools::throughput`), que é quem a usa.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Duração mínima de um sentido do teste, em segundos.
pub const MIN_DURATION_S: u8 = 1;
/// Duração máxima de um sentido do teste, em segundos.
pub const MAX_DURATION_S: u8 = 60;
/// Mínimo de fluxos TCP paralelos.
pub const MIN_STREAMS: u8 = 1;
/// Máximo de fluxos TCP paralelos.
pub const MAX_STREAMS: u8 = 8;

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
}
