//! Gerenciador de cadência periódica em memória para as rotinas do ciclo do
//! scheduler — as do núcleo e as que as áreas contribuem. Cada rotina guarda
//! a própria cadência (a constante mora com ela).

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Mutex, OnceLock},
};

use chrono::{DateTime, Utc};

/// Próximo instante de cada tarefa periódica do ciclo.
fn next_run_at() -> &'static Mutex<HashMap<&'static str, DateTime<Utc>>> {
    static NEXT: OnceLock<Mutex<HashMap<&'static str, DateTime<Utc>>>> = OnceLock::new();
    NEXT.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Próxima coleta SNMP de cada dispositivo sem monitor SNMP que a agende.
fn next_device_collection_at() -> &'static Mutex<HashMap<i64, DateTime<Utc>>> {
    static NEXT: OnceLock<Mutex<HashMap<i64, DateTime<Utc>>>> = OnceLock::new();
    NEXT.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `true` quando a chave venceu; já reserva o próximo horário no mapa.
fn reserve<K: Eq + Hash>(
    map: &Mutex<HashMap<K, DateTime<Utc>>>,
    key: K,
    interval_seconds: i64,
    now: DateTime<Utc>,
) -> bool {
    let Ok(mut next) = map.lock() else {
        return true;
    };
    match next.get(&key) {
        Some(scheduled) if now < *scheduled => false,
        _ => {
            next.insert(key, now + chrono::Duration::seconds(interval_seconds));
            true
        }
    }
}

/// `true` quando a tarefa venceu; já reserva o próximo horário no mapa.
pub fn is_due(key: &'static str, interval_seconds: i64, now: DateTime<Utc>) -> bool {
    reserve(next_run_at(), key, interval_seconds, now)
}

/// `true` quando a coleta SNMP do dispositivo venceu pelo intervalo dele;
/// já reserva a próxima.
pub fn is_device_collection_due(device_id: i64, interval_seconds: i64, now: DateTime<Utc>) -> bool {
    reserve(
        next_device_collection_at(),
        device_id,
        interval_seconds.max(1),
        now,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_coleta_do_dispositivo_respeita_o_intervalo_dele() {
        let inicio = Utc::now();
        let id = -4242;
        assert!(is_device_collection_due(id, 15, inicio));
        assert!(!is_device_collection_due(
            id,
            15,
            inicio + chrono::Duration::seconds(5)
        ));
        assert!(!is_device_collection_due(
            id,
            15,
            inicio + chrono::Duration::seconds(14)
        ));
        assert!(is_device_collection_due(
            id,
            15,
            inicio + chrono::Duration::seconds(15)
        ));
    }
}
