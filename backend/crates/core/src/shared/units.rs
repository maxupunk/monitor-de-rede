//! Conversões numéricas canônicas. O caminho de uso continua sendo
//! `network_tools::icmp_probe` (reexportado de lá); aqui só mora a definição,
//! para quem não enxerga o pacote raiz (o Decisor, no crate do assistente).

use std::time::Duration;

/// Arredonda um valor de ponto flutuante para duas casas decimais.
#[must_use]
pub fn round_two(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

/// Converte um `Duration` em milissegundos com arredondamento para duas casas decimais.
#[must_use]
pub fn duration_to_ms(duration: Duration) -> f64 {
    round_two(duration.as_secs_f64() * 1_000.0)
}
