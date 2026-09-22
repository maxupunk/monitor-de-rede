//! O que casa: padrão (literal ou regex, sempre sem caixa) e exclusão.
//!
//! Literal vira regex escapada, então os dois modos passam pelo mesmo
//! caminho — inclusive o recorte do trecho, que precisa da posição do
//! casamento em bytes válidos. O motor `regex` é de tempo linear: uma
//! expressão escrita pela IA não tem como travar o servidor, e o teto de
//! tamanho impede uma expressão gigante de estourar memória.

use regex::{Regex, RegexBuilder};

/// Caracteres máximos de um padrão.
pub const MAX_PATTERN_CHARS: usize = 200;
/// Teto de memória da expressão compilada.
const REGEX_SIZE_LIMIT: usize = 1 << 20;
/// Caracteres de cada lado do casamento quando a mensagem é recortada.
const SNIPPET_RADIUS: usize = 100;

fn compile(pattern: &str, regex: bool) -> Result<Regex, String> {
    if pattern.chars().count() > MAX_PATTERN_CHARS {
        return Err(format!(
            "Padrão longo demais (máximo {MAX_PATTERN_CHARS} caracteres)"
        ));
    }
    let source = if regex {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };
    RegexBuilder::new(&source)
        .case_insensitive(true)
        .size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|error| format!("Regex inválida: {error}"))
}

fn non_empty(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

#[derive(Debug, Clone)]
pub struct Matcher {
    include: Option<Regex>,
    exclude: Option<Regex>,
}

impl Matcher {
    /// Padrão vazio casa tudo (útil para contar por severidade ou origem).
    ///
    /// # Errors
    ///
    /// Padrão longo demais ou regex inválida — a mensagem vai para a IA
    /// corrigir.
    pub fn new(pattern: Option<&str>, exclude: Option<&str>, regex: bool) -> Result<Self, String> {
        Ok(Self {
            include: non_empty(pattern)
                .map(|pattern| compile(pattern, regex))
                .transpose()?,
            exclude: non_empty(exclude)
                .map(|pattern| compile(pattern, regex))
                .transpose()?,
        })
    }

    /// Casa o padrão e não casa a exclusão.
    #[must_use]
    pub fn is_match(&self, text: &str) -> bool {
        self.include.as_ref().is_none_or(|re| re.is_match(text))
            && self.exclude.as_ref().is_none_or(|re| !re.is_match(text))
    }

    /// A mensagem inteira quando curta; quando longa, o trecho em volta do
    /// casamento — é ele que a IA precisa ler.
    #[must_use]
    pub fn snippet(&self, text: &str, max_chars: usize) -> String {
        let text = text.trim();
        if text.chars().count() <= max_chars {
            return text.to_string();
        }
        let start_byte = self
            .include
            .as_ref()
            .and_then(|re| re.find(text))
            .map_or(0, |found| found.start());
        let match_char = text[..start_byte].chars().count();
        let from = match_char.saturating_sub(SNIPPET_RADIUS);
        let window = SNIPPET_RADIUS * 2;
        let piece: String = text.chars().skip(from).take(window).collect();
        let prefix = if from > 0 { "…" } else { "" };
        let suffix = if from + window < text.chars().count() {
            "…"
        } else {
            ""
        };
        format!("{prefix}{}{suffix}", piece.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_ignora_caixa_e_trata_simbolo_como_texto() {
        let m = Matcher::new(Some("Link Down (ether3)"), None, false).unwrap();
        assert!(m.is_match("sfp: LINK DOWN (ether3) after 3s"));
        assert!(!m.is_match("link down ether3"), "parênteses são literais");
    }

    #[test]
    fn regex_alterna_e_exclusao_remove() {
        let m = Matcher::new(Some("link (down|flap)"), Some("ether9"), true).unwrap();
        assert!(m.is_match("ether1 link down"));
        assert!(m.is_match("ether2 link flap"));
        assert!(!m.is_match("ether9 link down"), "excluído");
        assert!(!m.is_match("ether1 link up"));
    }

    #[test]
    fn padrao_vazio_casa_tudo_e_invalido_explica() {
        assert!(Matcher::new(Some("  "), None, true)
            .unwrap()
            .is_match("qualquer"));
        let erro = Matcher::new(Some("(abc"), None, true).unwrap_err();
        assert!(erro.contains("Regex inválida"));
        assert!(Matcher::new(Some(&"a".repeat(300)), None, false).is_err());
    }

    #[test]
    fn mensagem_longa_vira_trecho_em_volta_do_casamento() {
        let m = Matcher::new(Some("PANIC"), None, false).unwrap();
        let texto = format!("{} panic: kernel oops {}", "x".repeat(500), "y".repeat(500));
        let trecho = m.snippet(&texto, 240);
        assert!(trecho.contains("panic: kernel oops"));
        assert!(trecho.starts_with('…') && trecho.ends_with('…'));
        assert!(trecho.chars().count() <= 202);

        let acentos = format!("{}ção falhou{}", "á".repeat(300), "é".repeat(300));
        let m = Matcher::new(Some("falhou"), None, false).unwrap();
        assert!(
            m.snippet(&acentos, 240).contains("ção falhou"),
            "fatia por caractere"
        );
        assert_eq!(m.snippet("curta", 240), "curta");
    }
}
