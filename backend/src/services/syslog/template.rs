//! O padrão de uma mensagem de log: o que sobra quando números, IPs e MACs
//! viram `#`. Mil linhas de "link down on ether3" diferem só no horário e num
//! contador — são um padrão só.
//!
//! Serve a dois donos: as ferramentas da IA agrupam por ele na hora da
//! consulta, e a classificação de eventos o usa como chave — o Laya responde
//! uma vez por padrão, não uma vez por linha.

use sha2::{Digest, Sha256};

use crate::services::shared::text::truncate_chars;

/// Caracteres máximos de um padrão.
const PATTERN_CHARS: usize = 120;
/// Versão da normalização no hash: mudar a regra gera chaves novas, e os
/// padrões antigos são reclassificados em vez de casarem errado.
const HASH_VERSION: &str = "v1";

/// Token que é só identificador variável: número, IP, MAC, hexadecimal.
fn is_variable_token(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_digit())
        && token
            .chars()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.' | '-' | '/' | 'x' | 'X'))
}

/// Padrão da mensagem: identificadores viram `#`, dígitos soltos também.
#[must_use]
pub fn normalize_message(message: &str) -> String {
    let normalized: Vec<String> = message
        .split_whitespace()
        .map(|token| {
            let core = token.trim_matches(|c: char| matches!(c, ',' | ';' | '(' | ')' | '[' | ']'));
            if !core.is_empty() && is_variable_token(core) {
                return token.replace(core, "#");
            }
            let mut out = String::with_capacity(token.len());
            let mut in_digits = false;
            for c in token.chars() {
                if c.is_ascii_digit() {
                    if !in_digits {
                        out.push('#');
                    }
                    in_digits = true;
                } else {
                    in_digits = false;
                    out.push(c);
                }
            }
            out
        })
        .collect();
    truncate_chars(&normalized.join(" "), PATTERN_CHARS)
}

/// Chave estável do padrão: SHA-256 de `"v1|padrão"`, primeiros 8 bytes.
///
/// Estável entre versões do Rust e entre processos, ao contrário do
/// `DefaultHasher` — a chave é gravada no banco. Vai à API como hexadecimal,
/// porque um `i64` perde precisão no JavaScript.
#[must_use]
pub fn template_hash(template: &str) -> i64 {
    let digest = Sha256::digest(format!("{HASH_VERSION}|{template}").as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    i64::from_be_bytes(bytes)
}

/// O hash como a API o mostra (16 dígitos hexadecimais).
#[must_use]
pub fn hash_hex(hash: i64) -> String {
    format!("{:016x}", hash.cast_unsigned())
}

/// O contrário de [`hash_hex`]; `None` se não for um hash.
#[must_use]
pub fn parse_hash_hex(text: &str) -> Option<i64> {
    u64::from_str_radix(text.trim(), 16)
        .ok()
        .map(u64::cast_signed)
}

/// Regex que casa as linhas deste padrão, para pré-preencher uma regra de
/// alerta `log_pattern`: o texto fixo escapado, cada `#` como `\S+`. O corte
/// do padrão (`…`) vira "qualquer coisa depois".
#[must_use]
pub fn template_to_regex(template: &str) -> String {
    let (body, truncated) = template
        .strip_suffix('…')
        .map_or((template, false), |body| (body, true));
    let pattern = body
        .split('#')
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join(r"\S*");
    if truncated {
        format!("{pattern}.*")
    } else {
        pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identificadores_viram_marcador() {
        assert_eq!(
            normalize_message("login failure for user admin from 10.0.0.5 via ssh"),
            "login failure for user admin from # via ssh"
        );
        assert_eq!(normalize_message("ether3 link down"), "ether# link down");
    }

    #[test]
    fn hash_e_estavel_e_versionado() {
        // Valor dourado: mudar a normalização ou o hash muda este número, e
        // isso precisa ser uma decisão (subir `HASH_VERSION`), não um acidente.
        let hash = template_hash("ether# link down");
        assert_eq!(hash, template_hash("ether# link down"));
        assert_ne!(hash, template_hash("ether# link up"));
        assert_eq!(hash_hex(hash), "5946f5852770d51e");
        assert_eq!(parse_hash_hex(&hash_hex(hash)), Some(hash));
        assert_eq!(parse_hash_hex("nao-e-hash"), None);
    }

    #[test]
    fn regex_do_padrao_casa_a_linha_de_exemplo() {
        let line = "login failure for user admin from 10.0.0.5 via ssh (attempt 3)";
        let regex = regex::Regex::new(&template_to_regex(&normalize_message(line))).unwrap();
        assert!(regex.is_match(line));
        assert!(
            regex.is_match("login failure for user admin from 192.168.1.9 via ssh (attempt 12)")
        );
        assert!(!regex.is_match("login ok for user admin"));
    }
}
