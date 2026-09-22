//! Tratamento puro das séries lidas do banco antes de irem para o chat.
//!
//! Uma interface coletada a cada minuto gera 1 440 pontos por dia. A tela
//! não distingue mais que umas poucas centenas, e a IA só precisa do resumo —
//! então a série é reduzida por média em janelas e resumida em estatísticas.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::dtos::ai::AiChartPoint;

/// Pontos máximos por série desenhada no chat.
pub const MAX_CHART_POINTS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub at: DateTime<Utc>,
    pub value: f64,
}

/// Resumo que a IA recebe no lugar dos pontos.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SeriesStats {
    pub samples: usize,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub last: f64,
}

/// Arredonda para duas casas: precisão de sobra para ms, % e bps, e menos
/// dígitos no JSON que a IA lê.
#[must_use]
pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Estatísticas da série; `None` quando não há amostra.
#[must_use]
pub fn stats(samples: &[Sample]) -> Option<SeriesStats> {
    let last = samples.last()?.value;
    let (min, max, sum) = samples.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY, 0.0),
        |(min, max, sum), sample| {
            (
                min.min(sample.value),
                max.max(sample.value),
                sum + sample.value,
            )
        },
    );
    Some(SeriesStats {
        samples: samples.len(),
        min: round2(min),
        max: round2(max),
        avg: round2(sum / samples.len() as f64),
        last: round2(last),
    })
}

/// Reduz a série a no máximo `max_points` pela média de janelas consecutivas.
///
/// Cada janela vira um ponto no instante da sua última amostra, então o fim
/// da série continua no mesmo lugar. A média (e não o máximo) mantém a escala
/// coerente com o que o operador vê nas telas.
#[must_use]
pub fn downsample(samples: &[Sample], max_points: usize) -> Vec<Sample> {
    if max_points == 0 || samples.len() <= max_points {
        return samples.to_vec();
    }
    let window = samples.len().div_ceil(max_points);
    samples
        .chunks(window)
        .filter_map(|chunk| {
            let last = chunk.last()?;
            let avg = chunk.iter().map(|sample| sample.value).sum::<f64>() / chunk.len() as f64;
            Some(Sample {
                at: last.at,
                value: avg,
            })
        })
        .collect()
}

/// Serializa um instante em RFC 3339 (para `#[serde(serialize_with)]`).
///
/// # Errors
///
/// Os do serializador.
pub fn serialize_rfc3339<S: serde::Serializer>(
    at: &DateTime<Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&at.to_rfc3339())
}

/// Versão opcional de [`serialize_rfc3339`].
///
/// # Errors
///
/// Os do serializador.
pub fn serialize_rfc3339_opt<S: serde::Serializer>(
    at: &Option<DateTime<Utc>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match at {
        Some(at) => serializer.serialize_str(&at.to_rfc3339()),
        None => serializer.serialize_none(),
    }
}

/// Converte para os pontos do gráfico, já reduzidos.
#[must_use]
pub fn chart_points(samples: &[Sample]) -> Vec<AiChartPoint> {
    downsample(samples, MAX_CHART_POINTS)
        .into_iter()
        .map(|sample| AiChartPoint {
            time: sample.at.to_rfc3339(),
            value: round2(sample.value),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;

    fn serie(values: &[f64]) -> Vec<Sample> {
        let start = Utc.with_ymd_and_hms(2026, 9, 21, 0, 0, 0).unwrap();
        values
            .iter()
            .enumerate()
            .map(|(i, value)| Sample {
                at: start + Duration::minutes(i as i64),
                value: *value,
            })
            .collect()
    }

    #[test]
    fn estatisticas_da_serie() {
        let resumo = stats(&serie(&[10.0, 30.0, 20.0])).unwrap();
        assert_eq!(resumo.samples, 3);
        assert_eq!(resumo.min, 10.0);
        assert_eq!(resumo.max, 30.0);
        assert_eq!(resumo.avg, 20.0);
        assert_eq!(resumo.last, 20.0);
        assert_eq!(stats(&[]), None);
    }

    #[test]
    fn serie_curta_nao_e_reduzida() {
        let original = serie(&[1.0, 2.0, 3.0]);
        assert_eq!(downsample(&original, 10), original);
    }

    #[test]
    fn serie_longa_vira_medias_e_mantem_o_ultimo_instante() {
        let original = serie(&[1.0, 3.0, 5.0, 7.0, 9.0]);
        let reduzida = downsample(&original, 2);
        assert_eq!(reduzida.len(), 2);
        assert_eq!(reduzida[0].value, 3.0);
        assert_eq!(reduzida[1].value, 8.0);
        assert_eq!(reduzida[1].at, original[4].at);
    }

    #[test]
    fn pontos_do_grafico_respeitam_o_teto() {
        let longa = serie(&vec![1.0; 1440]);
        assert!(chart_points(&longa).len() <= MAX_CHART_POINTS);
    }
}
