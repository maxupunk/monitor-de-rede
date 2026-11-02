//! Utilidades de texto usadas por mais de um domínio.

use std::sync::LazyLock;

use regex::{Captures, Regex};

static HTML_ENTITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"&(#[0-9]{1,7}|#[xX][0-9a-fA-F]{1,6}|[a-zA-Z]{2,8});").expect("regex de entidade")
});
/// Blocos cujo conteúdo não é texto da página.
static HTML_NON_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(script|style|noscript|template)\b[^>]*>.*?</(?:script|style|noscript|template)\s*>|<!--.*?-->")
        .expect("regex de bloco sem texto")
});
/// Formatação dentro da linha: sai sem deixar espaço (`v<b>7.15</b>` é `v7.15`).
static HTML_INLINE_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)</?(?:a|b|i|u|em|strong|span|code|mark|small|sup|sub|wbr)\b[^>]*>")
        .expect("regex de tag de linha")
});
static HTML_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").expect("regex de tag"));

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

/// Espaços, quebras e tabulações viram um espaço só, sem sobra nas pontas.
#[must_use]
pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decodifica as entidades HTML numa passada só (`&amp;lt;` vira `&lt;`, não
/// `<`). Entidade desconhecida ou código inválido fica como está.
#[must_use]
pub fn decode_html_entities(text: &str) -> String {
    HTML_ENTITY
        .replace_all(text, |captures: &Captures| {
            let entity = &captures[1];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => entity.strip_prefix('#').and_then(|code| {
                    let value = match code.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => code.parse().ok(),
                    };
                    value.and_then(char::from_u32)
                }),
            };
            decoded.map_or_else(|| captures[0].to_string(), String::from)
        })
        .into_owned()
}

/// Texto legível de um trecho HTML: sem scripts, estilos, comentários e tags,
/// com as entidades decodificadas e os espaços colapsados.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let without_blocks = HTML_NON_TEXT.replace_all(html, " ");
    let without_inline = HTML_INLINE_TAG.replace_all(&without_blocks, "");
    let without_tags = HTML_TAG.replace_all(&without_inline, " ");
    collapse_whitespace(&decode_html_entities(&without_tags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corta_por_caractere_e_marca() {
        assert_eq!(truncate_chars("  ação  ", 10), "ação");
        assert_eq!(truncate_chars("ççççç", 3), "ççç…");
    }

    #[test]
    fn entidades_decodificam_numa_passada_so() {
        assert_eq!(
            decode_html_entities("a &amp; b &lt;x&gt; &quot;c&quot; &#39;d&#39; &#x41;&nbsp;"),
            "a & b <x> \"c\" 'd' A "
        );
        assert_eq!(
            decode_html_entities("&amp;lt;"),
            "&lt;",
            "não decodifica duas vezes"
        );
        assert_eq!(
            decode_html_entities("&inventada; &#xFFFFFFF;"),
            "&inventada; &#xFFFFFFF;"
        );
    }

    #[test]
    fn html_vira_texto_sem_script_nem_tag() {
        let html = "<div>Bug <b>confirmado</b><script>alert(1)</script>\n<style>p{}</style>\
                    <!-- nota --> no &lt;7.15.3&gt;</div>";
        assert_eq!(html_to_text(html), "Bug confirmado no <7.15.3>");
        assert_eq!(
            html_to_text("Router<b>OS</b> v<b>7.15</b><br>fim"),
            "RouterOS v7.15 fim"
        );
    }

    #[test]
    fn espacos_colapsam() {
        assert_eq!(collapse_whitespace("  a \n\t b  "), "a b");
    }
}
