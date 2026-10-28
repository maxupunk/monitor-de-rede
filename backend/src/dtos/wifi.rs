//! Telemetria Wi-Fi: o que a tela recebe (`docs/monitoramento_wifi_sinais.md`).
//!
//! Cada fabricante entrega as estações no contrato do plugin
//! (`services::wifi::contract`); daqui para frente o formato é um só, e a
//! classificação (faixas e saúde) já vem pronta — a tela só escolhe a cor.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Banda do rádio onde a estação está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum WifiBand {
    #[serde(rename = "2g")]
    Band2g,
    #[serde(rename = "5g")]
    Band5g,
    #[serde(rename = "6g")]
    Band6g,
}

/// Faixa de uma medida, da tabela de classificação (§3 do documento).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum WifiGrade {
    Excellent,
    Good,
    Fair,
    Poor,
    Critical,
}

/// Como a estação está, considerando todas as medidas juntas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum WifiHealth {
    Healthy,
    /// Sinal, SNR ou retransmissão na faixa de atenção.
    Fair,
    /// Retransmissão alta com sinal bom: interferência.
    HighRetries,
    /// Sinal ou SNR ruim.
    Critical,
    /// Parada e com sinal bom: economia de energia, não problema da rede.
    PowerSave,
}

/// Uma estação conectada a um equipamento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiStation {
    /// `aa:bb:cc:dd:ee:ff`.
    pub mac: String,
    /// MAC aleatório (privado), como celulares usam: sem fabricante.
    pub randomized_mac: bool,
    /// Fabricante pelo OUI do MAC.
    pub vendor: Option<String>,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub radio: Option<String>,
    pub band: Option<WifiBand>,
    pub channel: Option<u16>,
    pub channel_width_mhz: Option<u16>,
    pub signal_dbm: i32,
    pub signal_avg_dbm: Option<i32>,
    pub noise_dbm: Option<i32>,
    /// Sinal menos ruído. Sem o ruído do rádio, não há SNR.
    pub snr_db: Option<i32>,
    #[ts(type = "number | null")]
    pub rx_rate_bps: Option<u64>,
    #[ts(type = "number | null")]
    pub tx_rate_bps: Option<u64>,
    pub mcs: Option<u8>,
    /// Retransmissões sobre o total enviado desde a associação (0–100).
    pub tx_retries_pct: Option<f64>,
    #[ts(type = "number | null")]
    pub inactive_ms: Option<u64>,
    #[ts(type = "number | null")]
    pub connected_s: Option<u64>,
    pub signal_grade: WifiGrade,
    pub snr_grade: Option<WifiGrade>,
    pub retries_grade: Option<WifiGrade>,
    pub health: WifiHealth,
    /// Por que a estação não está saudável, em português.
    pub health_reason: Option<String>,
}

/// Estado da leitura de um equipamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum WifiReadStatus {
    /// Lendo agora; as estações são as da leitura anterior.
    Reading,
    Ok,
    Failed,
}

/// O que um equipamento tem conectado, como na última leitura. Viaja pelo
/// SSE (`wifi:snapshot`), um por equipamento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiDeviceSnapshot {
    #[ts(type = "number")]
    pub device_id: i64,
    pub device_name: String,
    pub status: WifiReadStatus,
    /// Quando a última leitura terminou (RFC 3339).
    pub read_at: Option<String>,
    /// Por que a última leitura falhou.
    pub error: Option<String>,
    /// Da pior para a melhor.
    pub stations: Vec<WifiStation>,
    /// O plugin do equipamento mede latência até as estações.
    pub can_ping: bool,
}

/// Pedido de leitura: um equipamento, ou todos sem `deviceId`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiRefreshInput {
    #[serde(default)]
    #[ts(optional, type = "number")]
    pub device_id: Option<i64>,
}

/// A leitura começou; o resultado chega pelo SSE.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiRefreshStarted {
    /// Os equipamentos lidos — numa leitura de todos, quem ficou de fora não
    /// tem mais Wi-Fi e sai da tela.
    #[ts(type = "Array<number>")]
    pub device_ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiPingInput {
    #[ts(type = "number")]
    pub device_id: i64,
    /// IP da estação.
    pub target: String,
}

/// Latência do equipamento até uma estação.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiPingResult {
    #[ts(type = "number")]
    pub device_id: i64,
    pub target: String,
    pub sent: u32,
    pub received: u32,
    pub loss_pct: f64,
    /// Média das respostas; sem resposta, não há média.
    pub avg_ms: Option<f64>,
    pub grade: WifiGrade,
}

/// Onde está um aparelho vigiado, pela última leitura completa da frota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum WifiWatchState {
    /// Ainda sem leitura completa desde que passou a ser vigiado.
    Pending,
    Connected,
    /// Nenhum roteador o enxerga.
    Offline,
}

/// Um aparelho que deveria estar sempre conectado (câmera, IoT). Só eles
/// geram alerta de Wi-Fi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiWatchedClient {
    #[ts(type = "number")]
    pub id: i64,
    /// `aa:bb:cc:dd:ee:ff`.
    pub mac: String,
    pub name: String,
    /// MAC privado: pode mudar e o aparelho "sumir" sem ter saído.
    pub randomized_mac: bool,
    pub state: WifiWatchState,
    /// O roteador onde apareceu por último.
    #[ts(type = "number | null")]
    pub device_id: Option<i64>,
    pub device_name: Option<String>,
    /// Quando apareceu por último (RFC 3339).
    pub last_seen_at: Option<String>,
}

/// A lista dos vigiados, pelo SSE (`wifi:watched`) — inteira a cada mudança.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiWatchedSnapshot {
    pub clients: Vec<WifiWatchedClient>,
}

/// Passar a vigiar um aparelho.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiWatchCreate {
    pub mac: String,
    pub name: String,
}

/// Renomear um aparelho vigiado.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct WifiWatchUpdate {
    pub name: String,
}
