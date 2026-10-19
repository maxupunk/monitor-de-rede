//! Autorizações do espelho: um token por pedido do plugin, válido por poucos
//! minutos. Vivem na memória do processo — reiniciar a central encerra todas,
//! e a próxima instalação pede outra.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock, PoisonError},
    time::{Duration, Instant},
};

use rand::{distributions::Alphanumeric, Rng};

/// Uma instalação lenta (índice grande, rádio fraco) cabe com folga.
pub const TTL: Duration = Duration::from_secs(20 * 60);
const TOKEN_CHARS: usize = 32;
/// Teto de autorizações vivas — um plugin em laço não enche a memória.
const MAX_GRANTS: usize = 256;

/// O que a execução de plugin recebe para oferecer o espelho ao script
/// (`device.package_mirror()`): a porta da API vista pelo equipamento. Só
/// existe quando a central fala direto com o equipamento — pelo agente
/// remoto, o roteador do outro site não alcança esta porta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorAccess {
    pub port: u16,
    /// Token fixo para os testes de unidade dos plugins; `None` emite um novo
    /// a cada pedido.
    pub fixed_token: Option<String>,
}

impl MirrorAccess {
    #[must_use]
    pub const fn new(port: u16) -> Self {
        Self {
            port,
            fixed_token: None,
        }
    }

    /// O token desta chamada.
    #[must_use]
    pub fn token(&self) -> String {
        self.fixed_token.clone().unwrap_or_else(issue)
    }
}

fn store() -> &'static Mutex<HashMap<String, Instant>> {
    static GRANTS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    GRANTS.get_or_init(Mutex::default)
}

/// Emite um token válido por [`TTL`].
#[must_use]
pub fn issue() -> String {
    let token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(TOKEN_CHARS)
        .map(char::from)
        .collect();
    let now = Instant::now();
    let mut grants = store().lock().unwrap_or_else(PoisonError::into_inner);
    grants.retain(|_, expires| *expires > now);
    if grants.len() >= MAX_GRANTS {
        // Sai o que vence primeiro.
        if let Some(oldest) = grants
            .iter()
            .min_by_key(|(_, expires)| **expires)
            .map(|(token, _)| token.clone())
        {
            grants.remove(&oldest);
        }
    }
    grants.insert(token.clone(), now + TTL);
    token
}

/// O token foi emitido e ainda vale.
#[must_use]
pub fn is_valid(token: &str) -> bool {
    store()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(token)
        .is_some_and(|expires| *expires > Instant::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_emitido_vale_e_desconhecido_nao() {
        let token = issue();
        assert_eq!(token.len(), TOKEN_CHARS);
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(is_valid(&token));
        assert!(!is_valid("desconhecido"));
        assert!(!is_valid(""));
        assert_ne!(issue(), token, "cada pedido recebe o seu");
    }

    #[test]
    fn token_fixo_dos_testes_nao_entra_no_registro() {
        let access = MirrorAccess {
            port: 3333,
            fixed_token: Some("TOKEN".into()),
        };
        assert_eq!(access.token(), "TOKEN");
        assert!(!is_valid("TOKEN"));
        assert!(is_valid(&MirrorAccess::new(3333).token()));
    }
}
