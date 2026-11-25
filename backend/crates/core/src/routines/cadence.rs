//! Gerenciador de cadência periódica em memória para as rotinas do ciclo do
//! scheduler — as do núcleo e as que as áreas contribuem. Cada rotina guarda
//! a própria cadência (a constante mora com ela).

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{
        atomic::{AtomicI64, Ordering},
        Mutex, OnceLock, PoisonError,
    },
};

use chrono::{DateTime, Utc};

/// Acima deste volume, o mapa de dispositivos expurga entradas vencidas há mais
/// de 24 h para evitar crescimento ilimitado após exclusões em massa.
const DEVICE_MAP_SWEEP_THRESHOLD: usize = 1_024;
const STALE_DEVICE_TTL_HOURS: i64 = 24;
/// Intervalo mínimo entre duas varreduras: com o mapa acima do limiar, varrer
/// a cada chamada custaria O(n) por dispositivo — O(n²) por ciclo.
const DEVICE_MAP_SWEEP_INTERVAL_SECONDS: i64 = 3_600;

/// Instante (Unix, segundos) da última varredura do mapa de dispositivos.
static LAST_DEVICE_SWEEP_AT: AtomicI64 = AtomicI64::new(i64::MIN);

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
    last_sweep_at: Option<&AtomicI64>,
) -> bool {
    let mut next = map.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(last_sweep_at) = last_sweep_at {
        sweep_if_due(&mut next, last_sweep_at, now);
    }
    match next.get(&key) {
        Some(scheduled) if now < *scheduled => false,
        _ => {
            next.insert(key, now + chrono::Duration::seconds(interval_seconds));
            true
        }
    }
}

/// Expurga as entradas vencidas há mais de 24 h, no máximo uma vez por
/// [`DEVICE_MAP_SWEEP_INTERVAL_SECONDS`] e só acima do limiar.
fn sweep_if_due<K: Eq + Hash>(
    next: &mut HashMap<K, DateTime<Utc>>,
    last_sweep_at: &AtomicI64,
    now: DateTime<Utc>,
) {
    if next.len() <= DEVICE_MAP_SWEEP_THRESHOLD {
        return;
    }
    let now_secs = now.timestamp();
    let last = last_sweep_at.load(Ordering::Relaxed);
    if now_secs.saturating_sub(last) < DEVICE_MAP_SWEEP_INTERVAL_SECONDS {
        return;
    }
    last_sweep_at.store(now_secs, Ordering::Relaxed);
    let cutoff = now - chrono::Duration::hours(STALE_DEVICE_TTL_HOURS);
    next.retain(|_, scheduled| *scheduled >= cutoff);
}

/// `true` quando a tarefa venceu; já reserva o próximo horário no mapa.
pub fn is_due(key: &'static str, interval_seconds: i64, now: DateTime<Utc>) -> bool {
    reserve(next_run_at(), key, interval_seconds, now, None)
}

/// `true` quando a coleta SNMP do dispositivo venceu pelo intervalo dele;
/// já reserva a próxima.
pub fn is_device_collection_due(device_id: i64, interval_seconds: i64, now: DateTime<Utc>) -> bool {
    reserve(
        next_device_collection_at(),
        device_id,
        interval_seconds.max(1),
        now,
        Some(&LAST_DEVICE_SWEEP_AT),
    )
}

/// Remove a cadência em memória de um dispositivo excluído.
pub fn forget_device(device_id: i64) {
    let mut next = next_device_collection_at()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    next.remove(&device_id);
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

    #[test]
    fn a_varredura_do_mapa_e_periodica() {
        let inicio = Utc::now();
        let ultima = AtomicI64::new(i64::MIN);
        let antigo = inicio - chrono::Duration::hours(STALE_DEVICE_TTL_HOURS + 1);
        let mut mapa: HashMap<i64, DateTime<Utc>> = (0..=i64::try_from(DEVICE_MAP_SWEEP_THRESHOLD)
            .unwrap())
            .map(|id| (id, antigo))
            .collect();
        sweep_if_due(&mut mapa, &ultima, inicio);
        assert!(mapa.is_empty(), "a primeira varredura expurga os vencidos");

        mapa = (0..=i64::try_from(DEVICE_MAP_SWEEP_THRESHOLD).unwrap())
            .map(|id| (id, antigo))
            .collect();
        sweep_if_due(&mut mapa, &ultima, inicio + chrono::Duration::seconds(60));
        assert_eq!(
            mapa.len(),
            DEVICE_MAP_SWEEP_THRESHOLD + 1,
            "dentro do intervalo, não varre de novo"
        );
        sweep_if_due(
            &mut mapa,
            &ultima,
            inicio + chrono::Duration::seconds(DEVICE_MAP_SWEEP_INTERVAL_SECONDS),
        );
        assert!(mapa.is_empty());
    }
}
