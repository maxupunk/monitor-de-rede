//! Distância aproximada entre o AP e o aparelho, pelo sinal (RSSI).
//!
//! Modelo de perda de percurso log-distância (`docs/monitoramento_wifi_sinais.md`
//! §4.3):
//!
//! ```text
//! perda medida  = potência do rádio (dBm) − sinal (dBm)
//! PL(d)         = PL(1 m) + 10 · n · log10(d / 1 m)
//! PL(1 m)       = FSPL(1 m, f) + perda extra de referência do ambiente
//! FSPL(1 m, f)  = 20 · log10(f em MHz) − 27,55
//! d             = 10 ^ ((perda medida − PL(1 m)) / (10 · n))
//! ```
//!
//! A faixa sai do sombreamento (σ, em dB) do ambiente: a mesma conta com a
//! perda medida −σ e +σ. O sinal do `station dump` é o que o **aparelho**
//! envia ao AP; a perda é simétrica, mas o aparelho transmite em geral menos
//! que o AP — essa diferença, mais antena e corpo, está na perda extra de
//! referência de cada ambiente. Por isso a resposta é sempre uma faixa
//! ("~3–6 m"), nunca um número exato.
//!
//! Função pura: o ambiente vem das Configurações de Wi-Fi (`settings`).

use crate::dtos::{WifiBand, WifiDistance, WifiEnvironment};

/// Nem o desenho nem a faixa passam disto: além, o sinal não diz mais nada.
const MIN_METERS: f64 = 0.5;
const MAX_METERS: f64 = 300.0;

/// Quanto o sinal cai com a distância num ambiente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathLoss {
    /// Expoente de perda `n` (2 = espaço livre).
    pub exponent: f64,
    /// Somado à perda do espaço livre a 1 m.
    pub reference_extra_db: f64,
    /// Sombreamento: o quanto o sinal varia no mesmo lugar (a largura da faixa).
    pub spread_db: f64,
}

/// Os parâmetros de cada ambiente (ITU-R P.1238 e medições típicas de
/// escritório; documentados no §4.3).
#[must_use]
pub const fn path_loss(environment: WifiEnvironment) -> PathLoss {
    match environment {
        WifiEnvironment::Open => PathLoss {
            exponent: 2.2,
            reference_extra_db: 4.0,
            spread_db: 4.0,
        },
        WifiEnvironment::Office => PathLoss {
            exponent: 3.0,
            reference_extra_db: 7.0,
            spread_db: 6.0,
        },
        WifiEnvironment::Walls => PathLoss {
            exponent: 3.8,
            reference_extra_db: 10.0,
            spread_db: 8.0,
        },
    }
}

/// O que a leitura diz do rádio e da estação.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioSignal {
    /// O sinal recebido (de preferência a média).
    pub signal_dbm: i32,
    pub band: Option<WifiBand>,
    pub channel: Option<u16>,
    pub frequency_mhz: Option<u32>,
    pub tx_power_dbm: Option<i32>,
}

/// Potência típica quando o rádio não informa: o teto comum de cada banda.
const fn default_tx_power(band: WifiBand) -> i32 {
    match band {
        WifiBand::Band2g => 20,
        WifiBand::Band5g | WifiBand::Band6g => 23,
    }
}

/// A frequência do canal, ou o meio da banda.
fn frequency(signal: &RadioSignal) -> Option<(f64, bool)> {
    if let Some(mhz) = signal.frequency_mhz.filter(|mhz| *mhz > 0) {
        return Some((f64::from(mhz), true));
    }
    let band = signal.band?;
    let by_channel = signal.channel.map(|channel| {
        let channel = f64::from(channel);
        match band {
            WifiBand::Band2g if channel >= 14.0 => 2484.0,
            WifiBand::Band2g => 2407.0 + 5.0 * channel,
            WifiBand::Band5g => 5000.0 + 5.0 * channel,
            WifiBand::Band6g => 5950.0 + 5.0 * channel,
        }
    });
    match by_channel {
        Some(mhz) => Some((mhz, true)),
        None => Some((
            match band {
                WifiBand::Band2g => 2437.0,
                WifiBand::Band5g => 5500.0,
                WifiBand::Band6g => 6175.0,
            },
            false,
        )),
    }
}

/// A distância (em metros) que explica uma perda, no ambiente.
fn meters_for(loss_db: f64, frequency_mhz: f64, model: PathLoss) -> f64 {
    let reference = 20.0 * frequency_mhz.log10() - 27.55 + model.reference_extra_db;
    let meters = 10f64.powf((loss_db - reference) / (10.0 * model.exponent));
    meters.clamp(MIN_METERS, MAX_METERS)
}

/// O passo da faixa: 1 m de perto, 5 m a média distância, 10 m longe.
fn step(meters: f64) -> f64 {
    if meters < 10.0 {
        1.0
    } else if meters < 50.0 {
        5.0
    } else {
        10.0
    }
}

/// A faixa "~3–6 m": arredondada para fora, sem número com vírgula.
fn rounded(min: f64, max: f64) -> (f64, f64) {
    let low = ((min / step(min)).floor() * step(min)).max(1.0);
    let high = ((max / step(max)).ceil() * step(max)).max(low + 1.0);
    (low, high)
}

/// A distância aproximada, ou `None` sem banda nem frequência.
#[must_use]
pub fn estimate(signal: &RadioSignal, environment: WifiEnvironment) -> Option<WifiDistance> {
    let (frequency_mhz, frequency_known) = frequency(signal)?;
    let band = signal.band.unwrap_or(if frequency_mhz < 3000.0 {
        WifiBand::Band2g
    } else {
        WifiBand::Band5g
    });
    let tx_power = signal
        .tx_power_dbm
        .unwrap_or_else(|| default_tx_power(band));
    let model = path_loss(environment);
    let loss = f64::from(tx_power - signal.signal_dbm);
    let meters = meters_for(loss, frequency_mhz, model);
    let (min_m, max_m) = rounded(
        meters_for(loss - model.spread_db, frequency_mhz, model),
        meters_for(loss + model.spread_db, frequency_mhz, model),
    );
    Some(WifiDistance {
        meters: (meters * 10.0).round() / 10.0,
        min_m,
        max_m,
        calibrated: signal.tx_power_dbm.is_some() && frequency_known,
        environment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sinal(signal_dbm: i32) -> RadioSignal {
        RadioSignal {
            signal_dbm,
            band: Some(WifiBand::Band5g),
            channel: Some(36),
            frequency_mhz: Some(5180),
            tx_power_dbm: Some(23),
        }
    }

    #[test]
    fn segue_o_modelo_log_distancia() {
        // Perda 81 dB; PL(1 m) = 20·log10(5180) − 27,55 + 7 = 53,7 dB;
        // d = 10^((81 − 53,7) / 30) ≈ 8,1 m.
        let perto = estimate(&sinal(-58), WifiEnvironment::Office).unwrap();
        assert!((perto.meters - 8.1).abs() < 0.2, "{perto:?}");
        assert!(perto.calibrated);
        assert_eq!(perto.environment, WifiEnvironment::Office);
        // ±6 dB de sombreamento: ~5,2 m a ~12,6 m, arredondado para fora.
        assert_eq!((perto.min_m, perto.max_m), (5.0, 15.0));
    }

    #[test]
    fn sinal_mais_fraco_fica_mais_longe_e_paredes_encurtam() {
        let forte = estimate(&sinal(-50), WifiEnvironment::Office).unwrap();
        let fraco = estimate(&sinal(-75), WifiEnvironment::Office).unwrap();
        assert!(fraco.meters > forte.meters * 3.0);
        let aberto = estimate(&sinal(-75), WifiEnvironment::Open).unwrap();
        let paredes = estimate(&sinal(-75), WifiEnvironment::Walls).unwrap();
        assert!(aberto.meters > fraco.meters && fraco.meters > paredes.meters);
        assert!(fraco.min_m < fraco.meters && fraco.meters < fraco.max_m);
    }

    #[test]
    fn potencia_menor_no_ap_aproxima_o_aparelho() {
        let cheia = estimate(&sinal(-65), WifiEnvironment::Office).unwrap();
        let reduzida = estimate(
            &RadioSignal {
                tx_power_dbm: Some(10),
                ..sinal(-65)
            },
            WifiEnvironment::Office,
        )
        .unwrap();
        assert!(reduzida.meters < cheia.meters);
    }

    #[test]
    fn sem_potencia_usa_o_padrao_da_banda_e_avisa() {
        let sem_potencia = estimate(
            &RadioSignal {
                tx_power_dbm: None,
                ..sinal(-58)
            },
            WifiEnvironment::Office,
        )
        .unwrap();
        assert!(!sem_potencia.calibrated, "menos confiável");
        assert_eq!(
            sem_potencia.meters,
            estimate(&sinal(-58), WifiEnvironment::Office)
                .unwrap()
                .meters,
            "o padrão de 5 GHz é 23 dBm"
        );

        // Plugin antigo: só banda e canal; a frequência sai do canal.
        let pelo_canal = RadioSignal {
            frequency_mhz: None,
            ..sinal(-58)
        };
        assert_eq!(frequency(&pelo_canal), Some((5180.0, true)));
        let canal_2g = RadioSignal {
            band: Some(WifiBand::Band2g),
            channel: Some(6),
            frequency_mhz: None,
            ..sinal(-58)
        };
        assert_eq!(frequency(&canal_2g), Some((2437.0, true)));
        let so_banda = RadioSignal {
            channel: None,
            frequency_mhz: None,
            tx_power_dbm: Some(23),
            ..sinal(-58)
        };
        let estimada = estimate(&so_banda, WifiEnvironment::Office).unwrap();
        assert!(
            !estimada.calibrated,
            "o meio da banda não é a frequência real"
        );
    }

    #[test]
    fn sem_banda_nem_frequencia_nao_estima() {
        let nada = RadioSignal {
            band: None,
            channel: None,
            frequency_mhz: None,
            ..sinal(-58)
        };
        assert_eq!(estimate(&nada, WifiEnvironment::Office), None);
    }

    #[test]
    fn extremos_ficam_dentro_dos_limites() {
        let colado = estimate(&sinal(-10), WifiEnvironment::Office).unwrap();
        assert_eq!(colado.meters, MIN_METERS);
        assert_eq!((colado.min_m, colado.max_m), (1.0, 2.0));
        let sumindo = estimate(&sinal(-100), WifiEnvironment::Open).unwrap();
        assert_eq!(sumindo.meters, MAX_METERS);
    }

    #[test]
    fn faixa_arredonda_para_fora_com_passo_pela_distancia() {
        assert_eq!(rounded(3.4, 6.2), (3.0, 7.0));
        assert_eq!(rounded(12.0, 27.0), (10.0, 30.0));
        assert_eq!(rounded(0.6, 0.9), (1.0, 2.0));
        assert_eq!(rounded(48.0, 91.0), (45.0, 100.0));
    }
}
