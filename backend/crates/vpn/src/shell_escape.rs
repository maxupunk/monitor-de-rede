//! Escape centralizado para valores interpolados em configs/scripts VPN.
//!
//! Todos os geradores de perfil devem passar strings dinâmicas por uma das
//! funções deste módulo antes de interpolá-las. Isso dá defesa em profundidade:
//! os campos já vêm validados, mas o ponto de interpolação não deve confiar
//! nisso sozinho.

/// Remove caracteres de controle e quebras de linha.
///
/// Base para formatos de linha simples (WireGuard `.conf`, cabeçalhos).
#[must_use]
pub fn strip_controls(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '\r' && *c != '\n' && *c != '\t')
        .collect()
}

/// Alias semântico para valores interpolados em arquivos `.conf` do WireGuard.
#[must_use]
pub fn escape_wg_value(s: &str) -> String {
    strip_controls(s)
}

/// Escape para comandos UCI / shell POSIX.
///
/// Remove controles/quebras e escapa aspas simples fechando a string,
/// inserindo uma aspas simples escapada e reabrindo.
#[must_use]
pub fn escape_uci(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '\r' && *c != '\n' && *c != '\t')
        .collect::<String>()
        .replace('\'', "'\\''")
}

/// Escape para comandos RouterOS (MikroTik).
///
/// Remove controles/quebras e escapa aspas duplas e barras invertidas.
#[must_use]
pub fn escape_routeros(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '\r' && *c != '\n' && *c != '\t')
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

/// Sanitiza um nome para uso em nome de arquivo.
///
/// Substitui controles, separadores de caminho e sequências perigosas (`../`,
/// `./`, ponto inicial) por `_`. Preserva acentos porque o nome do arquivo é
/// apenas rótulo de download.
#[must_use]
pub fn sanitize_file_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (idx, c) in s.chars().enumerate() {
        if c.is_control() || c == '/' || c == '\\' || c == '\0' {
            out.push('_');
        } else if idx == 0 && c == '.' {
            // Evita nomes ocultos ou relativos no início.
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_controls_remove_quebras_e_controles() {
        assert_eq!(strip_controls("a\nb\tc\r"), "abc");
        assert_eq!(
            strip_controls("chave\n[Peer]\nInjetado"),
            "chave[Peer]Injetado"
        );
    }

    #[test]
    fn escape_uci_escapa_aspas_simples() {
        assert_eq!(escape_uci("public"), "public");
        assert_eq!(escape_uci("o'neil"), "o'\\''neil");
        assert_eq!(escape_uci("a\nb"), "ab");
    }

    #[test]
    fn escape_routeros_escapa_aspas_e_barras() {
        assert_eq!(escape_routeros("pub"), "pub");
        assert_eq!(escape_routeros("a\\b"), "a\\\\b");
        assert_eq!(escape_routeros("a\"b"), "a\\\"b");
        assert_eq!(escape_routeros("a\nb"), "ab");
    }

    #[test]
    fn sanitize_file_name_remove_separadores_e_ponto_inicial() {
        assert_eq!(sanitize_file_name("Roteador São João"), "Roteador São João");
        assert_eq!(sanitize_file_name("../etc/passwd"), "_._etc_passwd");
        assert_eq!(sanitize_file_name(".oculto"), "_oculto");
        assert_eq!(sanitize_file_name("a\nb"), "a_b");
    }
}
