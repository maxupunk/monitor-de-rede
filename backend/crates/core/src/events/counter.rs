//! Contagem de conexões SSE abertas por chave, sem batimento nem consulta
//! periódica (AGENTS §9): cada conexão segura um guarda, e o guarda desconta
//! ao sair de escopo — quando o navegador fecha a aba, o stream cai e a
//! contagem desce sozinha.
//!
//! Base comum de [`super::Presence`] (quem está online) e
//! [`super::Interests`] (quem está olhando que dado).

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

type Counts<K> = Arc<Mutex<HashMap<K, usize>>>;

pub(crate) struct ConnectionCounter<K> {
    counts: Counts<K>,
}

impl<K> Clone for ConnectionCounter<K> {
    fn clone(&self) -> Self {
        Self {
            counts: Arc::clone(&self.counts),
        }
    }
}

impl<K> Default for ConnectionCounter<K> {
    fn default() -> Self {
        Self {
            counts: Arc::default(),
        }
    }
}

impl<K: Hash + Eq + Clone> ConnectionCounter<K> {
    /// Mais uma conexão com a chave, até o guarda sair de escopo.
    pub(crate) fn connect(&self, key: K) -> CounterGuard<K> {
        *lock(&self.counts).entry(key.clone()).or_default() += 1;
        CounterGuard {
            counts: Arc::clone(&self.counts),
            key,
        }
    }

    pub(crate) fn is_active(&self, key: &K) -> bool {
        lock(&self.counts).get(key).is_some_and(|count| *count > 0)
    }
}

/// Uma conexão contada. Ao cair, desconta a da chave.
pub struct CounterGuard<K: Hash + Eq> {
    counts: Counts<K>,
    key: K,
}

impl<K: Hash + Eq> Drop for CounterGuard<K> {
    fn drop(&mut self) {
        let mut counts = lock(&self.counts);
        if let Some(count) = counts.get_mut(&self.key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                counts.remove(&self.key);
            }
        }
    }
}

fn lock<K>(counts: &Counts<K>) -> MutexGuard<'_, HashMap<K, usize>> {
    counts.lock().unwrap_or_else(PoisonError::into_inner)
}
