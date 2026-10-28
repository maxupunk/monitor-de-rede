//! O contrato da saída dos plugins Wi-Fi (bloco `wifi` do manifesto).
//!
//! Cada fabricante lê o equipamento do seu jeito e entrega **medidas cruas**
//! neste formato — chaves em `snake_case`, valor desconhecido como `null`.
//! Nada de classificação no plugin: SNR, retransmissão e saúde são
//! calculados uma vez só, em [`super::station`].

use serde::Deserialize;
use serde_json::Value;

/// Saída do papel `clients`.
#[derive(Debug, Deserialize)]
struct ClientsOutput {
    clients: Vec<StationReading>,
}

/// Uma estação como o equipamento a vê.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct StationReading {
    pub mac: String,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub radio: Option<String>,
    /// `2g`, `5g` ou `6g`.
    pub band: Option<String>,
    pub channel: Option<u16>,
    pub channel_width_mhz: Option<u16>,
    pub signal_dbm: Option<i32>,
    pub signal_avg_dbm: Option<i32>,
    /// Ruído do rádio onde a estação está.
    pub noise_dbm: Option<i32>,
    pub rx_rate_bps: Option<u64>,
    pub tx_rate_bps: Option<u64>,
    pub mcs: Option<u8>,
    /// Contadores desde a associação.
    pub tx_packets: Option<u64>,
    pub tx_retries: Option<u64>,
    pub inactive_ms: Option<u64>,
    pub connected_s: Option<u64>,
}

/// Saída do papel `ping`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct PingReply {
    pub sent: u32,
    pub received: u32,
    #[serde(default)]
    pub avg_ms: Option<f64>,
}

/// As estações da saída de `clients`.
///
/// # Errors
///
/// Saída fora do contrato, com o motivo.
pub fn parse_clients(output: Value) -> Result<Vec<StationReading>, String> {
    serde_json::from_value::<ClientsOutput>(output)
        .map(|output| output.clients)
        .map_err(|error| format!("a lista de clientes veio fora do contrato ({error})"))
}

/// A resposta de `ping`.
///
/// # Errors
///
/// Saída fora do contrato, com o motivo.
pub fn parse_ping(output: Value) -> Result<PingReply, String> {
    serde_json::from_value(output)
        .map_err(|error| format!("o resultado do ping veio fora do contrato ({error})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_estacoes_com_valores_ausentes() {
        let estacoes = parse_clients(json!({
            "summary": "1 cliente",
            "clients": [{ "mac": "AA:BB:CC:DD:EE:01", "signal_dbm": -45, "noise_dbm": null,
                          "tx_packets": 100, "band": "5g" }]
        }))
        .unwrap();
        assert_eq!(estacoes.len(), 1);
        assert_eq!(estacoes[0].signal_dbm, Some(-45));
        assert_eq!(estacoes[0].noise_dbm, None);
        assert_eq!(estacoes[0].tx_packets, Some(100));
    }

    #[test]
    fn saida_fora_do_contrato_e_recusada() {
        assert!(parse_clients(json!({ "stations": [] })).is_err());
        assert!(parse_clients(json!({ "clients": [{ "signal_dbm": "forte" }] })).is_err());
        assert!(parse_ping(json!({ "avg_ms": 3.2 })).is_err());
        assert_eq!(
            parse_ping(json!({ "sent": 3, "received": 0, "avg_ms": null })).unwrap(),
            PingReply {
                sent: 3,
                received: 0,
                avg_ms: None
            }
        );
    }
}
