//! Cofre efêmero das chaves privadas de cliente (§8.10.4).
//!
//! A chave privada do peer **nunca** vai ao banco: fica em memória até a
//! primeira leitura ou até expirar. Depois disso só resta rotacionar o peer —
//! é o que garante que uma cópia do banco não seja um molho de chaves da VPN.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock, PoisonError},
    time::{Duration, Instant},
};

/// Placeholder exibido quando a chave privada já foi entregue e descartada.
pub const PRIVATE_KEY_UNAVAILABLE: &str = "<CHAVE-PRIVADA-INDISPONIVEL-ROTACIONE-AS-CHAVES>";

/// Janela em que o artefato ainda pode ser buscado depois de criado o peer.
const DEFAULT_TTL: Duration = Duration::from_secs(15 * 60);

/// Teto de entradas simultâneas em memória para evitar crescimento ilimitado.
const MAX_ENTRIES: usize = 4_096;

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
        let mut secrets = self.secrets.lock().unwrap_or_else(PoisonError::into_inner);
        let key = key.into();
        let now = Instant::now();
        // Vencido sai a cada gravação: uma chave privada nunca baixada não pode
        // ficar no heap até o cofre encher (AGENTS §7).
        purge_expired(&mut secrets, now);
        if secrets.len() >= MAX_ENTRIES && !secrets.contains_key(&key) {
            if let Some(oldest_key) = secrets
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at)
                .map(|(k, _)| k.clone())
            {
                secrets.remove(&oldest_key);
            }
        }
        secrets.insert(
            key,
            StoredSecret {
                value: value.into(),
                expires_at: now + self.ttl,
            },
        );
    }

    /// Lê e descarta: a segunda chamada devolve `None`.
    pub fn consume(&self, key: &str) -> Option<String> {
        let mut secrets = self.secrets.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        secrets
            .remove(key)
            .filter(|secret| secret.expires_at > now)
            .map(|secret| secret.value)
    }

    /// Lê **sem** descartar — para o segredo de sessão que vale por várias
    /// leituras até expirar (senha de equipamento pedida a cada sessão).
    pub fn get(&self, key: &str) -> Option<String> {
        let mut secrets = self.secrets.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        match secrets.get(key) {
            Some(secret) if secret.expires_at > now => Some(secret.value.clone()),
            Some(_) => {
                secrets.remove(key);
                None
            }
            None => None,
        }
    }

    /// Descarta o segredo, se houver.
    pub fn forget(&self, key: &str) {
        let mut secrets = self.secrets.lock().unwrap_or_else(PoisonError::into_inner);
        secrets.remove(key);
    }

    /// Indica se ainda existe segredo disponível, **sem** consumi-lo.
    pub fn has(&self, key: &str) -> bool {
        let mut secrets = self.secrets.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        match secrets.get(key) {
            Some(secret) if secret.expires_at > now => true,
            Some(_) => {
                secrets.remove(key);
                false
            }
            None => false,
        }
    }

    pub fn clear(&self) {
        let mut secrets = self.secrets.lock().unwrap_or_else(PoisonError::into_inner);
        secrets.clear();
    }
}

fn purge_expired(secrets: &mut HashMap<String, StoredSecret>, now: Instant) {
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
    fn gravar_expurga_os_vencidos_abaixo_do_teto() {
        let store = EphemeralSecretStore::new(Duration::from_millis(0));
        store.put("vpn-peer:a", "chave");
        store.put("vpn-peer:b", "chave");
        let secrets = store.secrets.lock().unwrap_or_else(PoisonError::into_inner);
        // Só a última gravação resta: a anterior, vencida, saiu no `put`.
        assert_eq!(secrets.len(), 1);
        assert!(secrets.contains_key("vpn-peer:b"));
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
