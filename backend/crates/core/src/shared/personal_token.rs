//! O que todo token de acesso pessoal tem em comum (`nmm_` do MCP, `nmr_` do
//! registry): o segredo aparece uma vez, o banco guarda só o SHA-256 e os 4
//! últimos caracteres, a validade é obrigatória, e o token vale enquanto não
//! foi revogado nem venceu.
//!
//! Cada área guarda os seus na própria tabela e decide o próprio escopo; aqui
//! fica só a regra comum, para não divergir entre elas.

use axum::http::{header, HeaderMap};
use chrono::{DateTime, FixedOffset, Utc};

use super::{
    crypto::{random_token, sha256_hex},
    errors::{AppError, AppResult},
};

/// Tamanho máximo do nome dado pelo usuário.
pub const MAX_NAME_CHARS: usize = 120;

/// Um token recém-emitido: o segredo (mostrado uma vez) e o que vai ao banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issued {
    pub secret: String,
    pub hash: String,
    /// Os 4 últimos caracteres, para o usuário reconhecer o token na lista.
    pub hint: String,
}

/// Emite um segredo `prefix` + 32 bytes aleatórios.
#[must_use]
pub fn issue(prefix: &str) -> Issued {
    let secret = format!("{prefix}{}", random_token());
    let skip = secret.chars().count().saturating_sub(4);
    Issued {
        hash: sha256_hex(&secret),
        hint: secret.chars().skip(skip).collect(),
        secret,
    }
}

/// O nome sem espaços nas pontas.
///
/// # Errors
///
/// `Validation` com nome vazio ou longo demais.
pub fn validate_name(raw: &str) -> AppResult<&str> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(AppError::validation(format!(
            "Dê um nome ao token (até {MAX_NAME_CHARS} caracteres)"
        )));
    }
    Ok(name)
}

/// # Errors
///
/// `Validation` com validade fora de `1..=max_days`.
pub fn validate_ttl(days: i64, max_days: i64) -> AppResult<()> {
    if (1..=max_days).contains(&days) {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "A validade vai de 1 a {max_days} dias"
        )))
    }
}

/// O hash a procurar no banco, se o segredo tem o prefixo desta área.
#[must_use]
pub fn lookup_hash(prefix: &str, secret: &str) -> Option<String> {
    secret.starts_with(prefix).then(|| sha256_hex(secret))
}

/// O token ainda vale: não foi revogado nem venceu.
#[must_use]
pub fn is_usable(
    revoked_at: Option<DateTime<FixedOffset>>,
    expires_at: DateTime<FixedOffset>,
    now: DateTime<Utc>,
) -> bool {
    revoked_at.is_none() && expires_at.with_timezone(&Utc) > now
}

/// O segredo de `Authorization: Bearer …` (o esquema sem distinguir
/// maiúsculas, como manda a RFC 7235).
#[must_use]
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;
    use chrono::Duration;

    use super::*;

    #[test]
    fn emite_segredo_hash_e_dica() {
        let issued = issue("nmm_");
        assert!(issued.secret.starts_with("nmm_"));
        assert_eq!(issued.hash, sha256_hex(&issued.secret));
        assert_eq!(issued.hint.chars().count(), 4);
        assert!(issued.secret.ends_with(&issued.hint));
        assert_ne!(issue("nmm_").secret, issued.secret);
    }

    #[test]
    fn nome_e_validade() {
        assert_eq!(validate_name("  ide  ").expect("ok"), "ide");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
        assert!(validate_ttl(1, 366).is_ok());
        assert!(validate_ttl(366, 366).is_ok());
        assert!(validate_ttl(0, 366).is_err());
        assert!(validate_ttl(367, 366).is_err());
    }

    #[test]
    fn hash_so_com_o_prefixo_da_area() {
        assert_eq!(lookup_hash("nmm_", "nmr_abc"), None);
        assert_eq!(lookup_hash("nmm_", "nmm_abc"), Some(sha256_hex("nmm_abc")));
    }

    #[test]
    fn vale_ate_revogar_ou_vencer() {
        let now = Utc::now();
        let later = (now + Duration::days(1)).fixed_offset();
        let earlier = (now - Duration::seconds(1)).fixed_offset();
        assert!(is_usable(None, later, now));
        assert!(!is_usable(None, earlier, now));
        assert!(!is_usable(Some(earlier), later, now));
    }

    #[test]
    fn le_o_bearer() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), None);
        for (raw, expected) in [
            ("Bearer nmm_1", Some("nmm_1")),
            ("bearer  nmm_2 ", Some("nmm_2")),
            ("BEARER nmm_3", Some("nmm_3")),
            ("Bearer ", None),
            ("Basic dXNlcg==", None),
        ] {
            headers.insert(header::AUTHORIZATION, HeaderValue::from_static(raw));
            assert_eq!(bearer_token(&headers), expected, "{raw}");
        }
    }
}
