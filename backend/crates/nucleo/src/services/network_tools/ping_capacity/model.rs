//! Contrato do diagnóstico rápido por ping (`POST /api/diagnostics/quick-ping`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Pedido: um dispositivo cadastrado **ou** um IP de rede privada.
#[derive(Debug, Clone, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct QuickPingInput {
    #[ts(optional, type = "number")]
    pub device_id: Option<i64>,
    /// Endereço avulso; só faixas privadas, CGNAT, loopback e link-local.
    #[ts(optional)]
    pub ip: Option<String>,
}

/// Latência de um tamanho de pacote (só as respostas recebidas).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct QuickPingRtt {
    /// Tamanho do payload ICMP, em bytes.
    pub bytes: u32,
    pub sent: u32,
    pub received: u32,
    pub min_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
}

/// Quanto a latência sobe acima do mínimo — o sinal de fila no caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum QueueingLevel {
    Ok,
    Moderado,
    Alto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct QuickPingQueueing {
    /// p95 − mínimo, em ms (dos pacotes grandes, ou dos pequenos sem eles).
    pub inflation_ms: f64,
    pub level: QueueingLevel,
}

/// O ping nunca dá mais que uma ordem de grandeza — por isso não existe "alta".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum CapacityConfidence {
    Baixa,
    MuitoBaixa,
}

/// Faixa da capacidade do caminho, em Mbit/s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct QuickPingCapacity {
    pub low_mbps: f64,
    pub high_mbps: f64,
    pub confidence: CapacityConfidence,
}

/// Resultado do diagnóstico rápido. Mede o **caminho todo** da central até o
/// equipamento (inclusive a CPU dele respondendo ao ping), não um enlace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct QuickPingReport {
    pub target: String,
    /// Ecos enviados (pequenos + grandes).
    pub sent: u32,
    /// Ecos respondidos.
    pub samples: u32,
    pub loss_pct: f64,
    /// `null` quando nenhum pacote pequeno voltou.
    pub small: Option<QuickPingRtt>,
    /// `null` quando nenhum pacote grande voltou (MTU menor ou filtro de ICMP).
    pub large: Option<QuickPingRtt>,
    pub jitter_ms: Option<f64>,
    /// `null` quando nada respondeu.
    pub queueing: Option<QuickPingQueueing>,
    /// RTTmin(grande) − RTTmin(pequeno), em ms — a base da estimativa.
    pub delta_min_ms: Option<f64>,
    /// `null` quando o sinal não passa do ruído (ver `capacity_note`).
    pub capacity: Option<QuickPingCapacity>,
    pub capacity_note: String,
    pub verdict: String,
}
