//! Normalização e formatação da velocidade negociada de um link.
//!
//! `ifSpeed` é um contador de 32 bits: agentes que não expõem `ifHighSpeed`
//! devolvem o teto (4.294.967.295) para links acima de ~4,29 Gbps. Tratar esse
//! valor como velocidade real produzia falso downgrade/upgrade sempre que a
//! leitura alternava entre o teto e o valor verdadeiro (matriz de paridade #18).

/// Teto do contador de 32 bits: leitura inconclusiva, não velocidade.
pub const IF_SPEED_SATURATED: i64 = 4_294_967_295;

/// Converte a leitura crua em bps utilizável, ou `None` quando não é conclusiva.
#[must_use]
pub fn normalize_speed(bps: Option<i64>) -> Option<i64> {
    let value = bps?;
    if value <= 0 || value >= IF_SPEED_SATURATED {
        return None;
    }
    Some(value)
}

/// Rótulo legível da velocidade, com a mesma escala e arredondamento do
/// backend anterior — os textos aparecem em alertas e no feed em tempo real.
#[must_use]
pub fn format_speed(bps: Option<i64>) -> String {
    let Some(value) = bps.filter(|value| *value > 0) else {
        return "Desconhecido".to_string();
    };
    for (unit_bps, suffix) in [
        (1_000_000_000_f64, "Gbps"),
        (1_000_000_f64, "Mbps"),
        (1_000_f64, "Kbps"),
    ] {
        #[allow(clippy::cast_precision_loss)]
        let scaled = value as f64 / unit_bps;
        if scaled >= 1.0 {
            // Inteiro sai sem casa decimal ("1 Gbps"); o resto com uma só,
            // como o `Number.isInteger(x) ? x : x.toFixed(1)` do original.
            if (scaled.fract()).abs() < f64::EPSILON {
                return format!("{scaled:.0} {suffix}");
            }
            // `toFixed` arredonda meio para longe do zero; o `{:.1}` do Rust
            // arredonda meio para o par (1,25 viraria "1.2"). Arredondar antes
            // mantém os rótulos idênticos aos do backend anterior.
            let rounded = (scaled * 10.0).round() / 10.0;
            return format!("{rounded:.1} {suffix}");
        }
    }
    format!("{value} bps")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leitura_saturada_de_32_bits_nao_vira_velocidade() {
        assert_eq!(normalize_speed(Some(IF_SPEED_SATURATED)), None);
        assert_eq!(normalize_speed(Some(IF_SPEED_SATURATED + 1)), None);
        assert_eq!(normalize_speed(Some(1_000_000_000)), Some(1_000_000_000));
    }

    #[test]
    fn zero_negativo_e_ausente_sao_inconclusivos() {
        assert_eq!(normalize_speed(None), None);
        assert_eq!(normalize_speed(Some(0)), None);
        assert_eq!(normalize_speed(Some(-1)), None);
    }

    #[test]
    fn formata_na_escala_do_backend_anterior() {
        assert_eq!(format_speed(Some(1_000_000_000)), "1 Gbps");
        assert_eq!(format_speed(Some(2_500_000_000)), "2.5 Gbps");
        assert_eq!(format_speed(Some(100_000_000)), "100 Mbps");
        assert_eq!(format_speed(Some(1_500_000)), "1.5 Mbps");
        assert_eq!(format_speed(Some(64_000)), "64 Kbps");
        assert_eq!(format_speed(Some(512)), "512 bps");
        assert_eq!(format_speed(None), "Desconhecido");
        assert_eq!(format_speed(Some(0)), "Desconhecido");
    }

    #[test]
    fn arredonda_meio_para_longe_do_zero_como_o_tofixed() {
        // `format!("{:.1}", 1.25)` daria "1.2"; o backend anterior exibia "1.3".
        assert_eq!(format_speed(Some(1_250_000_000)), "1.3 Gbps");
    }

    #[test]
    fn variacao_irrelevante_de_leitura_formata_igual() {
        // É desta igualdade que o dataset de interfaces depende para não
        // reportar renegociação onde só houve ruído de leitura.
        assert_eq!(
            format_speed(Some(1_250_000_000)),
            format_speed(Some(1_260_000_000))
        );
    }
}
