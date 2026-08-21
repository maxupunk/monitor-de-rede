//! Validação e sanitização de nomes de peer VPN (SEC-05).
//!
//! O nome de um peer vira comentário no `wg0.conf`; caracteres de controle
//! (especialmente quebras de linha e tabulação) podem distorcer o arquivo ou
//! injetar linhas extras. Esta camada valida a entrada na API e sanitiza o
//! valor antes da interpolação como defesa em profundidade.

use crate::services::shared::errors::{AppError, AppResult};

/// Rejeita nomes vazios ou que contenham caracteres de controle/whitespace
/// perigosos (`\n`, `\r`, `\t` e outros controles ASCII).
///
/// # Errors
///
/// Retorna `AppError::validation` quando o nome é inválido.
pub fn validate(name: &str) -> AppResult<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("Nome do dispositivo é obrigatório"));
    }
    if trimmed.chars().any(|c| c.is_control() || c == '\t') {
        return Err(AppError::validation(
            "Nome do dispositivo não pode conter caracteres de controle",
        ));
    }
    Ok(trimmed)
}

/// Sanitiza um nome para uso seguro dentro do `wg0.conf`.
///
/// Substitui caracteres de controle e tabulação por `_`, garantindo que a
/// linha de comentário não seja quebrada nem interprete conteúdo injetado.
#[must_use]
pub fn sanitize_for_config(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() || c == '\t' { '_' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_valido_e_aceito_e_trimado() {
        assert_eq!(validate("  Filial 01  ").unwrap(), "Filial 01");
    }

    #[test]
    fn nome_vazio_e_rejeitado() {
        assert!(validate("").is_err());
        assert!(validate("   ").is_err());
    }

    #[test]
    fn quebras_de_linha_sao_rejeitadas() {
        assert!(validate("filial\n01").is_err());
        assert!(validate("filial\r01").is_err());
        assert!(validate("filial\r\n01").is_err());
    }

    #[test]
    fn tabulacao_e_controles_sao_rejeitados() {
        assert!(validate("filial\t01").is_err());
        assert!(validate("filial\x0001").is_err());
        assert!(validate("filial\x7f").is_err());
    }

    #[test]
    fn sanitize_preserva_texto_longo() {
        assert_eq!(sanitize_for_config("Filial 01"), "Filial 01");
    }

    #[test]
    fn sanitize_substituie_controles_por_underscore() {
        assert_eq!(sanitize_for_config("filial\n01\t\rX\x00"), "filial_01__X_");
    }
}
