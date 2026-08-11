//! Cofre efêmero das chaves privadas de cliente (§8.10.4).
//!
//! A chave privada do peer **nunca** vai ao banco: fica em memória até a
//! primeira leitura ou até expirar. Depois disso só resta rotacionar o peer —
//! é o que garante que uma cópia do banco não seja um molho de chaves da VPN.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

/// Placeholder exibido quando a chave privada já foi entregue e descartada.
pub const PRIVATE_KEY_UNAVAILABLE: &str = "<CHAVE-PRIVADA-INDISPONIVEL-ROTACIONE-AS-CHAVES>";

/// Janela em que o artefato ainda pode ser buscado depois de criado o peer.
const DEFAULT_TTL: Duration = Duration::from_secs(15 * 60);

struct StoredSecret {
    value: String,
    expires_at: Instant,
}

pub struct EphemeralSecretStore {
    ttl: Duration,
    secrets: Mutex<HashMap<String, StoredSecret>>,
}

impl Default for EphemeralSecretStore {
    fn default() -> Self {
        Self::new(DEFAULT_TTL)
    }
}

impl EphemeralSecretStore {
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            secrets: Mutex::new(HashMap::new()),
        }
    }

    pub fn put(&self, key: impl Into<String>, value: impl Into<String>) {
        let Ok(mut secrets) = self.secrets.lock() else {
            return;
        };
        purge_expired(&mut secrets);
        secrets.insert(
            key.into(),
            StoredSecret {
                value: value.into(),
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    /// Lê e descarta: a segunda chamada devolve `None`.
    pub fn consume(&self, key: &str) -> Option<String> {
        let mut secrets = self.secrets.lock().ok()?;
        purge_expired(&mut secrets);
        secrets.remove(key).map(|secret| secret.value)
    }

    /// Indica se ainda existe segredo disponível, **sem** consumi-lo.
    pub fn has(&self, key: &str) -> bool {
        let Ok(mut secrets) = self.secrets.lock() else {
            return false;
        };
        purge_expired(&mut secrets);
        secrets.contains_key(key)
    }

    pub fn clear(&self) {
        if let Ok(mut secrets) = self.secrets.lock() {
            secrets.clear();
        }
    }
}

fn purge_expired(secrets: &mut HashMap<String, StoredSecret>) {
    let now = Instant::now();
    secrets.retain(|_, secret| secret.expires_at > now);
}

/// Instância compartilhada pelo processo da API.
///
/// É `static` de propósito: a chave precisa sobreviver entre a resposta do
/// `POST /api/vpn/peers` e o `GET /api/vpn/peers/:id/config` que o wizard faz
/// em seguida, mas **não** pode atravessar processos — o scheduler nunca
/// deveria conseguir ler uma chave de cliente.
pub fn client_key_store() -> &'static EphemeralSecretStore {
    static STORE: OnceLock<EphemeralSecretStore> = OnceLock::new();
    STORE.get_or_init(EphemeralSecretStore::default)
}

/// Chave de indexação do cofre para um peer.
#[must_use]
pub fn secret_key(peer_id: i64) -> String {
    format!("vpn-peer:{peer_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chave_e_entregue_uma_unica_vez() {
        let store = EphemeralSecretStore::default();
        store.put("vpn-peer:1", "chave-privada");

        assert!(store.has("vpn-peer:1"));
        assert_eq!(
            store.consume("vpn-peer:1").as_deref(),
            Some("chave-privada")
        );
        // Matriz de paridade #33: a segunda leitura não devolve nada.
        assert_eq!(store.consume("vpn-peer:1"), None);
        assert!(!store.has("vpn-peer:1"));
    }

    #[test]
    fn segredo_expirado_some_sozinho() {
        let store = EphemeralSecretStore::new(Duration::from_millis(0));
        store.put("vpn-peer:2", "chave");
        assert!(!store.has("vpn-peer:2"));
        assert_eq!(store.consume("vpn-peer:2"), None);
    }

    #[test]
    fn rotacionar_substitui_a_chave_pendente() {
        let store = EphemeralSecretStore::default();
        store.put("vpn-peer:3", "antiga");
        store.put("vpn-peer:3", "nova");
        assert_eq!(store.consume("vpn-peer:3").as_deref(), Some("nova"));
    }

    #[test]
    fn a_chave_de_indexacao_e_estavel() {
        assert_eq!(secret_key(42), "vpn-peer:42");
    }
}
