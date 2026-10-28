//! Da medida crua do plugin à estação que a tela mostra: MAC normalizado,
//! fabricante, SNR, retransmissão e a classificação de [`super::quality`].

use super::{
    contract::StationReading,
    quality::{self, Reading},
};
use crate::{
    dtos::wifi::{WifiBand, WifiHealth, WifiStation},
    services::{network_tools::neighbor_cache::normalize_mac, vendors},
};

/// A estação pronta, ou `None` quando falta o que a identifica (MAC válido)
/// ou o que a classifica (sinal).
#[must_use]
pub fn build(reading: StationReading) -> Option<WifiStation> {
    let mac = normalize_mac(&reading.mac)?;
    let signal_dbm = reading.signal_dbm?;
    let vendor = vendors::lookup(&mac);
    let snr_db = reading.noise_dbm.map(|noise| signal_dbm - noise);
    let tx_retries_pct = retries_pct(reading.tx_packets, reading.tx_retries);
    let (health, health_reason) = quality::assess(Reading {
        signal_dbm,
        snr_db,
        retries_pct: tx_retries_pct,
        inactive_ms: reading.inactive_ms,
    });
    Some(WifiStation {
        mac,
        randomized_mac: vendor.locally_administered,
        vendor: vendor.vendor,
        ip: non_empty(reading.ip),
        hostname: non_empty(reading.hostname),
        ssid: non_empty(reading.ssid),
        bssid: non_empty(reading.bssid).and_then(|bssid| normalize_mac(&bssid)),
        radio: non_empty(reading.radio),
        band: reading.band.as_deref().and_then(band),
        channel: reading.channel.filter(|channel| *channel > 0),
        channel_width_mhz: reading.channel_width_mhz.filter(|width| *width > 0),
        signal_dbm,
        signal_avg_dbm: reading.signal_avg_dbm,
        noise_dbm: reading.noise_dbm,
        snr_db,
        rx_rate_bps: reading.rx_rate_bps,
        tx_rate_bps: reading.tx_rate_bps,
        mcs: reading.mcs,
        tx_retries_pct,
        inactive_ms: reading.inactive_ms,
        connected_s: reading.connected_s,
        signal_grade: quality::signal(signal_dbm),
        snr_grade: snr_db.map(quality::snr),
        retries_grade: tx_retries_pct.map(quality::retries),
        health,
        health_reason,
    })
}

/// Da pior para a melhor: saúde mais grave primeiro, depois o sinal mais fraco.
pub fn sort_worst_first(stations: &mut [WifiStation]) {
    stations.sort_by(|a, b| {
        severity(a.health)
            .cmp(&severity(b.health))
            .then_with(|| a.signal_dbm.cmp(&b.signal_dbm))
            .then_with(|| a.mac.cmp(&b.mac))
    });
}

const fn severity(health: WifiHealth) -> u8 {
    match health {
        WifiHealth::Critical => 0,
        WifiHealth::HighRetries => 1,
        WifiHealth::Fair => 2,
        WifiHealth::Healthy => 3,
        WifiHealth::PowerSave => 4,
    }
}

/// Retransmissões sobre tudo o que foi transmitido (entregues + repetidas),
/// em %. Sem envio, não há taxa.
#[allow(clippy::cast_precision_loss)]
fn retries_pct(packets: Option<u64>, retries: Option<u64>) -> Option<f64> {
    let (packets, retries) = (packets?, retries?);
    let total = packets.checked_add(retries).filter(|total| *total > 0)?;
    let pct = retries as f64 / total as f64 * 100.0;
    Some((pct * 10.0).round() / 10.0)
}

fn band(value: &str) -> Option<WifiBand> {
    match value {
        "2g" => Some(WifiBand::Band2g),
        "5g" => Some(WifiBand::Band5g),
        "6g" => Some(WifiBand::Band6g),
        _ => None,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty() && text != "*")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::wifi::WifiGrade;

    fn leitura() -> StationReading {
        StationReading {
            mac: "AA-BB-CC-DD-EE-01".into(),
            ip: Some("192.168.1.150".into()),
            hostname: Some("*".into()),
            band: Some("5g".into()),
            channel: Some(36),
            signal_dbm: Some(-58),
            noise_dbm: Some(-92),
            tx_packets: Some(900),
            tx_retries: Some(100),
            inactive_ms: Some(40),
            ..StationReading::default()
        }
    }

    #[test]
    fn calcula_snr_retransmissao_e_classifica() {
        let estacao = build(leitura()).unwrap();
        assert_eq!(estacao.mac, "aa:bb:cc:dd:ee:01");
        assert_eq!(estacao.hostname, None, "`*` do lease é sem nome");
        assert_eq!(estacao.band, Some(WifiBand::Band5g));
        assert_eq!(estacao.snr_db, Some(34));
        assert_eq!(estacao.tx_retries_pct, Some(10.0));
        assert_eq!(estacao.signal_grade, WifiGrade::Excellent);
        assert_eq!(estacao.retries_grade, Some(WifiGrade::Good));
        assert_eq!(estacao.health, WifiHealth::Healthy);
    }

    #[test]
    fn sem_mac_valido_ou_sinal_nao_vira_estacao() {
        assert!(build(StationReading {
            mac: "lixo".into(),
            ..leitura()
        })
        .is_none());
        assert!(build(StationReading {
            signal_dbm: None,
            ..leitura()
        })
        .is_none());
    }

    #[test]
    fn sem_contadores_nao_inventa_taxa() {
        assert_eq!(retries_pct(None, Some(3)), None);
        assert_eq!(retries_pct(Some(0), Some(0)), None);
        let sem_ruido = build(StationReading {
            noise_dbm: None,
            tx_packets: None,
            ..leitura()
        })
        .unwrap();
        assert_eq!(sem_ruido.snr_db, None);
        assert_eq!(sem_ruido.snr_grade, None);
        assert_eq!(sem_ruido.tx_retries_pct, None);
    }

    #[test]
    fn ordena_da_pior_para_a_melhor() {
        let mut estacoes: Vec<_> = [-50, -88, -74]
            .into_iter()
            .enumerate()
            .map(|(i, signal)| {
                build(StationReading {
                    mac: format!("aa:bb:cc:dd:ee:0{i}"),
                    signal_dbm: Some(signal),
                    ..leitura()
                })
                .unwrap()
            })
            .collect();
        sort_worst_first(&mut estacoes);
        let sinais: Vec<_> = estacoes.iter().map(|s| s.signal_dbm).collect();
        assert_eq!(sinais, [-88, -74, -50]);
    }
}
