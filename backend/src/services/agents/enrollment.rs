//! Códigos de enrollment: a ponte de uso único entre o cadastro do agente na
//! central e a primeira conexão dele.
//!
//! O código é entregue ao operador (comando de instalação, script da VPN) e
//! trocado **uma vez** pelo token de longa duração em `POST /api/agents/enroll`.
//! Mesma semântica do cofre das chaves da VPN — em memória, TTL de 15 minutos,
//! leitura única — reaproveitando o [`EphemeralSecretStore`]. O banco guarda
//! só o hash do token; o código nunca vai a disco.

use std::sync::OnceLock;

use crate::services::{
    shared::crypto::{random_token, sha256_hex},
    vpn::secret_store::EphemeralSecretStore,
};

/// Prefixo legível: quem vê o código num script sabe o que ele é.
const PREFIX: &str = "nma_";

fn vault() -> &'static EphemeralSecretStore {
    static STORE: OnceLock<EphemeralSecretStore> = OnceLock::new();
    STORE.get_or_init(EphemeralSecretStore::default)
}

/// Emite um código novo para o probe. Códigos anteriores continuam válidos
/// até expirar ou serem usados.
#[must_use]
pub fn issue(probe_id: i64) -> String {
    let code = format!("{PREFIX}{}", random_token());
    vault().put(sha256_hex(&code), probe_id.to_string());
    code
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
    fn codigo_vale_uma_unica_vez() {
        let code = issue(42);
        assert!(code.starts_with(PREFIX));
        assert_eq!(redeem(&code), Some(42));
        assert_eq!(redeem(&code), None);
    }

    #[test]
    fn codigo_desconhecido_ou_sem_prefixo_e_recusado() {
        assert_eq!(redeem("nma_inexistente"), None);
        assert_eq!(redeem(&random_token()), None);
    }
}
