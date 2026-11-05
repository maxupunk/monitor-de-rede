//! Classificação da qualidade Wi-Fi: a faixa de cada medida e a saúde da
//! estação. Tabela e regras no §3 de `docs/monitoramento_wifi_sinais.md` —
//! este é o único lugar que as aplica; plugin e tela só as consomem.

use crate::dtos::wifi::{WifiGrade, WifiHealth};

/// Parada há mais que isto, com sinal bom, é economia de energia.
pub const POWER_SAVE_INACTIVE_MS: u64 = 10_000;
/// Sinal mínimo para ler inatividade como repouso, não como estação sumindo.
pub const POWER_SAVE_MIN_SIGNAL_DBM: i32 = -72;

/// Sinal (RSSI): ≥ -60 excelente, até -70 bom, até -78 atenção, até -84
/// ruim, abaixo disso crítico.
#[must_use]
pub const fn signal(dbm: i32) -> WifiGrade {
    match dbm {
        -60.. => WifiGrade::Excellent,
        -70.. => WifiGrade::Good,
        -78.. => WifiGrade::Fair,
        -84.. => WifiGrade::Poor,
        _ => WifiGrade::Critical,
    }
}

/// SNR: ≥ 30 excelente, ≥ 25 bom, ≥ 15 atenção, ≥ 10 ruim, abaixo crítico.
#[must_use]
pub const fn snr(db: i32) -> WifiGrade {
    match db {
        30.. => WifiGrade::Excellent,
        25.. => WifiGrade::Good,
        15.. => WifiGrade::Fair,
        10.. => WifiGrade::Poor,
        _ => WifiGrade::Critical,
    }
}

/// Retransmissões (%): < 10 excelente, ≤ 15 bom, ≤ 25 atenção, ≤ 40 ruim,
/// acima crítico.
#[must_use]
pub fn retries(pct: f64) -> WifiGrade {
    if pct < 10.0 {
        WifiGrade::Excellent
    } else if pct <= 15.0 {
        WifiGrade::Good
    } else if pct <= 25.0 {
        WifiGrade::Fair
    } else if pct <= 40.0 {
        WifiGrade::Poor
    } else {
        WifiGrade::Critical
    }
}

/// Latência do equipamento até a estação: < 10 ms excelente, ≤ 25 bom,
/// ≤ 60 atenção, ≤ 150 ruim; acima disso, ou com perda, crítico.
#[must_use]
pub fn latency(avg_ms: Option<f64>, lost: bool) -> WifiGrade {
    match avg_ms {
        Some(ms) if !lost && ms < 10.0 => WifiGrade::Excellent,
        Some(ms) if !lost && ms <= 25.0 => WifiGrade::Good,
        Some(ms) if !lost && ms <= 60.0 => WifiGrade::Fair,
        Some(ms) if !lost && ms <= 150.0 => WifiGrade::Poor,
        _ => WifiGrade::Critical,
    }
}

/// As medidas de uma estação que decidem a saúde.
#[derive(Debug, Clone, Copy)]
pub struct Reading {
    pub signal_dbm: i32,
    pub snr_db: Option<i32>,
    pub retries_pct: Option<f64>,
    pub inactive_ms: Option<u64>,
}

/// A saúde e o motivo, em português, quando não é saudável.
///
/// Ordem: repouso primeiro (não é problema da rede, §3), depois o pior
/// sintoma — sinal ou SNR ruim é crítico; retransmissão ruim com sinal bom é
/// interferência; qualquer medida na faixa de atenção é atenção.
#[must_use]
pub fn assess(reading: Reading) -> (WifiHealth, Option<String>) {
    let Reading {
        signal_dbm,
        snr_db,
        retries_pct,
        inactive_ms,
    } = reading;
    if inactive_ms.is_some_and(|ms| ms > POWER_SAVE_INACTIVE_MS)
        && signal_dbm >= POWER_SAVE_MIN_SIGNAL_DBM
    {
        return (
            WifiHealth::PowerSave,
            Some("Em repouso: parada há mais de 10 s, com sinal bom".into()),
        );
    }

    let signal_grade = signal(signal_dbm);
    let snr_grade = snr_db.map(snr);
    let retries_grade = retries_pct.map(retries);

    if signal_grade >= WifiGrade::Poor {
        return (
            WifiHealth::Critical,
            Some(format!("Sinal fraco: {signal_dbm} dBm")),
        );
    }
    if let (Some(db), Some(grade)) = (snr_db, snr_grade) {
        if grade >= WifiGrade::Poor {
            return (
                WifiHealth::Critical,
                Some(format!("Muito ruído: SNR de {db} dB")),
            );
        }
    }
    if let (Some(pct), Some(grade)) = (retries_pct, retries_grade) {
        if grade >= WifiGrade::Poor {
            return (
                WifiHealth::HighRetries,
                Some(format!("Interferência: {pct:.1}% dos envios repetidos")),
            );
        }
    }
    let attention = [Some(signal_grade), snr_grade, retries_grade]
        .into_iter()
        .flatten()
        .any(|grade| grade == WifiGrade::Fair);
    if attention {
        return (
            WifiHealth::Fair,
            Some(fair_reason(signal_dbm, snr_db, retries_pct)),
        );
    }
    (WifiHealth::Healthy, None)
}

/// O que está na faixa de atenção, na ordem da tabela.
fn fair_reason(signal_dbm: i32, snr_db: Option<i32>, retries_pct: Option<f64>) -> String {
    let mut parts = Vec::new();
    if signal(signal_dbm) == WifiGrade::Fair {
        parts.push(format!("sinal de {signal_dbm} dBm"));
    }
    if let Some(db) = snr_db.filter(|db| snr(*db) == WifiGrade::Fair) {
        parts.push(format!("SNR de {db} dB"));
    }
    if let Some(pct) = retries_pct.filter(|pct| retries(*pct) == WifiGrade::Fair) {
        parts.push(format!("{pct:.1}% de retransmissão"));
    }
    format!("Atenção: {}", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(signal_dbm: i32) -> Reading {
        Reading {
            signal_dbm,
            snr_db: Some(35),
            retries_pct: Some(2.0),
            inactive_ms: Some(100),
        }
    }

    #[test]
    fn faixas_seguem_a_tabela_nas_fronteiras() {
        use WifiGrade::{Critical, Excellent, Fair, Good, Poor};
        let sinal: Vec<_> = [-60, -61, -70, -71, -78, -79, -84, -85].map(signal).into();
        assert_eq!(
            sinal,
            [Excellent, Good, Good, Fair, Fair, Poor, Poor, Critical]
        );
        let ruido: Vec<_> = [30, 29, 25, 24, 15, 14, 10, 9].map(snr).into();
        assert_eq!(
            ruido,
            [Excellent, Good, Good, Fair, Fair, Poor, Poor, Critical]
        );
        let repeticao: Vec<_> = [9.9, 10.0, 15.0, 15.1, 25.0, 25.1, 40.0, 40.1]
            .map(retries)
            .into();
        assert_eq!(
            repeticao,
            [Excellent, Good, Good, Fair, Fair, Poor, Poor, Critical]
        );
        assert_eq!(latency(Some(9.9), false), Excellent);
        assert_eq!(latency(Some(25.0), false), Good);
        assert_eq!(latency(Some(60.0), false), Fair);
        assert_eq!(latency(Some(150.0), false), Poor);
        assert_eq!(latency(Some(150.1), false), Critical);
        assert_eq!(latency(Some(2.0), true), Critical, "perda é crítica");
        assert_eq!(latency(None, true), Critical, "sem resposta");
    }

    #[test]
    fn repouso_com_sinal_bom_nao_penaliza() {
        let parada = Reading {
            inactive_ms: Some(12_000),
            retries_pct: Some(60.0),
            ..reading(-65)
        };
        assert_eq!(assess(parada).0, WifiHealth::PowerSave);
        // Parada e longe não é repouso: é estação sumindo.
        let longe = Reading {
            inactive_ms: Some(12_000),
            ..reading(-80)
        };
        assert_eq!(assess(longe).0, WifiHealth::Critical);
    }

    #[test]
    fn pior_sintoma_decide_a_saude() {
        assert_eq!(assess(reading(-55)), (WifiHealth::Healthy, None));

        let (saude, motivo) = assess(reading(-82));
        assert_eq!(saude, WifiHealth::Critical);
        assert_eq!(motivo.as_deref(), Some("Sinal fraco: -82 dBm"));

        let ruidoso = Reading {
            snr_db: Some(12),
            ..reading(-65)
        };
        assert_eq!(
            assess(ruidoso),
            (
                WifiHealth::Critical,
                Some("Muito ruído: SNR de 12 dB".into())
            )
        );

        let interferencia = Reading {
            retries_pct: Some(32.5),
            ..reading(-60)
        };
        assert_eq!(
            assess(interferencia),
            (
                WifiHealth::HighRetries,
                Some("Interferência: 32.5% dos envios repetidos".into())
            )
        );

        let atencao = Reading {
            snr_db: Some(20),
            ..reading(-74)
        };
        assert_eq!(
            assess(atencao),
            (
                WifiHealth::Fair,
                Some("Atenção: sinal de -74 dBm, SNR de 20 dB".into())
            )
        );
    }

    #[test]
    fn sem_ruido_nem_contadores_decide_pelo_sinal() {
        let so_sinal = Reading {
            signal_dbm: -66,
            snr_db: None,
            retries_pct: None,
            inactive_ms: None,
        };
        assert_eq!(assess(so_sinal).0, WifiHealth::Healthy);
    }
}
