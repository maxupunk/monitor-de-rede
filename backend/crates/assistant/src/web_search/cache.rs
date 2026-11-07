//! Memória curta das pesquisas.
//!
//! Um incidente costuma abrir vários alertas com o mesmo erro, e cada resumo
//! automático faria a mesma busca: a primeira paga a ida à web, as demais
//! leem daqui. Fica só em memória — some no reinício, que é o prazo certo
//! para conteúdo de fórum e advisory.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct TtlCache<V> {
    entries: Mutex<HashMap<String, (Instant, V)>>,
    ttl: Duration,
    capacity: usize,
}

impl<V: Clone> TtlCache<V> {
    #[must_use]
    pub fn new(ttl: Duration, capacity: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
            capacity: capacity.max(1),
        }
    }

    /// O valor guardado, se ainda não venceu.
    pub fn get(&self, key: &str) -> Option<V> {
        self.get_at(key, Instant::now())
    }

    pub fn insert(&self, key: String, value: V) {
        self.insert_at(key, value, Instant::now());
    }

    fn get_at(&self, key: &str, now: Instant) -> Option<V> {
        let entries = self.entries.lock().ok()?;
        entries
            .get(key)
            .filter(|(stored, _)| now.duration_since(*stored) < self.ttl)
            .map(|(_, value)| value.clone())
    }

    /// Cheio, sai primeiro o vencido e depois o mais antigo.
    fn insert_at(&self, key: String, value: V, now: Instant) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        entries.retain(|_, (stored, _)| now.duration_since(*stored) < self.ttl);
        if entries.len() >= self.capacity && !entries.contains_key(&key) {
            let oldest = entries
                .iter()
                .min_by_key(|(_, (stored, _))| *stored)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                entries.remove(&oldest);
            }
        }
        entries.insert(key, (now, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vence_no_prazo() {
        let cache = TtlCache::new(Duration::from_secs(60), 8);
        let start = Instant::now();
        cache.insert_at("a".into(), 1, start);
        assert_eq!(cache.get_at("a", start + Duration::from_secs(59)), Some(1));
        assert_eq!(cache.get_at("a", start + Duration::from_secs(60)), None);
        assert_eq!(cache.get_at("b", start), None);
    }

    #[test]
    fn cheio_descarta_o_mais_antigo() {
        let cache = TtlCache::new(Duration::from_secs(60), 2);
        let start = Instant::now();
        cache.insert_at("a".into(), 1, start);
        cache.insert_at("b".into(), 2, start + Duration::from_secs(1));
        cache.insert_at("c".into(), 3, start + Duration::from_secs(2));
        let now = start + Duration::from_secs(3);
        assert_eq!(cache.get_at("a", now), None);
        assert_eq!(cache.get_at("b", now), Some(2));
        assert_eq!(cache.get_at("c", now), Some(3));
    }
}
