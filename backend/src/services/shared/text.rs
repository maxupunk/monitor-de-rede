//! Utilidades de texto usadas por mais de um domínio.

/// Corta em `max` caracteres (não bytes), marcando o corte.
#[must_use]
pub fn truncate_chars(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(max).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corta_por_caractere_e_marca() {
        assert_eq!(truncate_chars("  ação  ", 10), "ação");
        assert_eq!(truncate_chars("ççççç", 3), "ççç…");
    }
}
