//! Códigos de enrollment: a ponte de uso único entre o cadastro do agente na
//! central e a primeira conexão dele.
//!
//! O código é entregue ao operador (comando de instalação, script da VPN) e
//! trocado **uma vez** pelo token de longa duração em `POST /api/agents/enroll`.
//! Mesma semântica do cofre das chaves da VPN — em memória, TTL de 15 minutos,
//! leitura única — reaproveitando o [`EphemeralSecretStore`]. O banco guarda
//! só o hash do token; o código nunca vai a disco.

use std::sync::OnceLock;

use crate::shared::{
    crypto::{random_token, sha256_hex},
    errors::{AppError, AppResult},
    secret_store::EphemeralSecretStore,
};

/// Prefixo legível: quem vê o código num script sabe o que ele é.
const PREFIX: &str = "nma_";
const HEX_LEN: usize = 64;

fn vault() -> &'static EphemeralSecretStore {
    static STORE: OnceLock<EphemeralSecretStore> = OnceLock::new();
    STORE.get_or_init(EphemeralSecretStore::default)
}

/// O código entra no comando colado no servidor: só o formato emitido pela
/// central (`nma_` + 64 hex) é aceito.
///
/// # Errors
///
/// `validation` para qualquer outro formato.
pub fn validate_code(code: &str) -> AppResult<&str> {
    let code = code.trim();
    let valid = code
        .strip_prefix(PREFIX)
        .is_some_and(|hex| hex.len() == HEX_LEN && hex.chars().all(|c| c.is_ascii_hexdigit()));
    if valid {
        Ok(code)
    } else {
        Err(AppError::validation("Código de instalação inválido"))
    }
}

/// Emite um código novo para o probe. Códigos anteriores continuam válidos
/// até expirar ou serem usados.
#[must_use]
pub fn issue(probe_id: i64) -> String {
    let code = format!("{PREFIX}{}", random_token());
    vault().put(sha256_hex(&code), probe_id.to_string());
    code
}

/// Consulta qual probe recebeu este código **sem** consumi-lo — usado quando a
/// tela reestampa os comandos com outra política antes de o operador rodar o
/// script.
#[must_use]
pub fn peek(code: &str) -> Option<i64> {
    let code = validate_code(code).ok()?;
    vault().get(&sha256_hex(code))?.parse().ok()
}

/// Troca o código pelo id do probe. A segunda tentativa com o mesmo código
/// devolve `None`.
#[must_use]
pub fn redeem(code: &str) -> Option<i64> {
    let code = code.trim();
    if !code.starts_with(PREFIX) {
        return None;
    }
    vault().consume(&sha256_hex(code))?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigo_vale_uma_unica_vez_mas_peek_nao_consome() {
        let code = issue(42);
        assert!(code.starts_with(PREFIX));
        assert!(validate_code(&code).is_ok());
        assert_eq!(peek(&code), Some(42));
        assert_eq!(redeem(&code), Some(42));
        assert_eq!(peek(&code), None);
        assert_eq!(redeem(&code), None);
    }

    #[test]
    fn codigo_desconhecido_ou_sem_prefixo_e_recusado() {
        assert_eq!(redeem("nma_inexistente"), None);
        assert_eq!(redeem(&random_token()), None);
        assert!(validate_code("nma_curto").is_err());
        assert!(validate_code(&format!("nma_{}", "z".repeat(HEX_LEN))).is_err());
    }
}
