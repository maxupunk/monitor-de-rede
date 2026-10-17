//! Trava de concorrência dos testes de enlace (ADR 015 §5): um teste por
//! dispositivo por vez e no máximo [`MAX_CONCURRENT`] no sistema.
//!
//! O registro é do processo — é nele que os testes rodam (ADR 007). Cada
//! teste em curso segura uma [`Reservation`]: os dispositivos que ele ocupa,
//! a porta iperf3 que os equipamentos dele usam e o token que o cancela. Ela
//! sai do registro quando é descartada, então um teste que termina de
//! qualquer jeito (até em pânico) libera a vaga.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard, OnceLock, PoisonError,
    },
};

use tokio_util::sync::CancellationToken;

use crate::services::shared::errors::{AppError, AppResult};

/// Testes simultâneos no sistema.
pub const MAX_CONCURRENT: usize = 2;
/// Faixa de portas que os equipamentos usam para o `iperf3 -s` (ADR 015 §4).
pub const IPERF3_PORTS: std::ops::RangeInclusive<u16> = 5201..=5299;

#[derive(Debug, Clone)]
struct Slot {
    test_id: Option<i64>,
    devices: BTreeSet<i64>,
    port: u16,
    cancel: CancellationToken,
}

fn registry() -> MutexGuard<'static, BTreeMap<u64, Slot>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<u64, Slot>>> = OnceLock::new();
    REGISTRY
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// A vaga de um teste. Descartar libera.
#[derive(Debug)]
pub struct Reservation {
    key: u64,
    /// Porta iperf3 deste teste (5201–5299), sem colidir com os outros.
    pub port: u16,
    /// Cancelar este token cancela o teste.
    pub cancel: CancellationToken,
}

impl Reservation {
    /// Associa a vaga ao teste gravado (para o cancelamento por id).
    pub fn bind(&self, test_id: i64) {
        if let Some(slot) = registry().get_mut(&self.key) {
            slot.test_id = Some(test_id);
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        registry().remove(&self.key);
    }
}

/// Conflito com o que já roda, se houver: o teto do sistema ou um
/// dispositivo ocupado.
///
/// Função pura (o registro é passado), testada sem estado global.
#[must_use]
pub fn conflict(
    active: &[(Option<i64>, BTreeSet<i64>)],
    wanted: &BTreeSet<i64>,
    name_of: &dyn Fn(i64) -> String,
) -> Option<String> {
    if active.len() >= MAX_CONCURRENT {
        return Some(format!(
            "Já há {MAX_CONCURRENT} testes de enlace em andamento, o máximo do sistema. Espere um \
             terminar ou cancele-o."
        ));
    }
    active.iter().find_map(|(test_id, devices)| {
        devices.intersection(wanted).next().map(|busy| {
            let which = test_id.map_or_else(String::new, |id| format!(" (#{id})"));
            format!(
                "{} já está em outro teste de enlace{which}. Espere terminar ou cancele-o.",
                name_of(*busy)
            )
        })
    })
}

/// A primeira porta livre da faixa iperf3.
#[must_use]
pub fn allocate_port(in_use: &BTreeSet<u16>) -> Option<u16> {
    IPERF3_PORTS.into_iter().find(|port| !in_use.contains(port))
}

/// Reserva a vaga do teste que ocupa `devices`.
///
/// # Errors
///
/// [`AppError::Conflict`] (409) com o motivo em português.
pub fn reserve(devices: &BTreeSet<i64>, name_of: &dyn Fn(i64) -> String) -> AppResult<Reservation> {
    static NEXT_KEY: AtomicU64 = AtomicU64::new(1);
    let mut active = registry();
    let snapshot: Vec<(Option<i64>, BTreeSet<i64>)> = active
        .values()
        .map(|slot| (slot.test_id, slot.devices.clone()))
        .collect();
    if let Some(problem) = conflict(&snapshot, devices, name_of) {
        return Err(AppError::conflict(problem));
    }
    let ports: BTreeSet<u16> = active.values().map(|slot| slot.port).collect();
    let port = allocate_port(&ports)
        .ok_or_else(|| AppError::conflict("Nenhuma porta livre para o servidor iperf3"))?;
    let key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
    let cancel = CancellationToken::new();
    active.insert(
        key,
        Slot {
            test_id: None,
            devices: devices.clone(),
            port,
            cancel: cancel.clone(),
        },
    );
    Ok(Reservation { key, port, cancel })
}

/// Pede o cancelamento do teste em curso. `false` se ele não roda aqui.
#[must_use]
pub fn cancel(test_id: i64) -> bool {
    registry()
        .values()
        .find(|slot| slot.test_id == Some(test_id))
        .map(|slot| slot.cancel.cancel())
        .is_some()
}

/// O teste roda neste processo.
#[must_use]
pub fn is_running(test_id: i64) -> bool {
    registry()
        .values()
        .any(|slot| slot.test_id == Some(test_id))
}

/// Dispositivos ocupados agora, com o teste que os ocupa.
#[must_use]
pub fn busy_devices() -> BTreeMap<i64, Option<i64>> {
    registry()
        .values()
        .flat_map(|slot| slot.devices.iter().map(|device| (*device, slot.test_id)))
        .collect()
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    fn set(ids: &[i64]) -> BTreeSet<i64> {
        ids.iter().copied().collect()
    }

    fn name(id: i64) -> String {
        format!("D{id}")
    }

    #[test]
    fn conflito_por_dispositivo_ocupado() {
        let active = vec![(Some(7), set(&[1, 2]))];
        assert_eq!(conflict(&active, &set(&[3, 4]), &name), None);
        let problem = conflict(&active, &set(&[2, 3]), &name).unwrap();
        assert!(
            problem.starts_with("D2 já está em outro teste de enlace (#7)"),
            "{problem}"
        );
    }

    #[test]
    fn conflito_pelo_teto_do_sistema() {
        let active = vec![(Some(1), set(&[1])), (None, set(&[2]))];
        let problem = conflict(&active, &set(&[9]), &name).unwrap();
        assert!(problem.contains("Já há 2 testes"), "{problem}");
    }

    #[test]
    fn porta_iperf3_sem_colisao() {
        assert_eq!(allocate_port(&BTreeSet::new()), Some(5201));
        assert_eq!(
            allocate_port(&[5201, 5202].into_iter().collect()),
            Some(5203)
        );
        let cheia: BTreeSet<u16> = IPERF3_PORTS.collect();
        assert_eq!(allocate_port(&cheia), None);
    }

    #[test]
    #[serial(link_tests)]
    fn reserva_libera_ao_descartar_e_cancela_por_id() {
        let primeira = reserve(&set(&[-901, -902]), &name).unwrap();
        primeira.bind(-91);
        assert!(is_running(-91));
        assert_eq!(busy_devices().get(&-901), Some(&Some(-91)));
        let erro = reserve(&set(&[-902]), &name).unwrap_err();
        assert!(matches!(erro, AppError::Conflict(_)));
        let segunda = reserve(&set(&[-903]), &name).unwrap();
        assert_ne!(primeira.port, segunda.port, "portas diferentes");
        assert!(matches!(
            reserve(&set(&[-904]), &name),
            Err(AppError::Conflict(_))
        ));
        assert!(cancel(-91));
        assert!(primeira.cancel.is_cancelled());
        assert!(!cancel(-92));
        drop(primeira);
        assert!(!is_running(-91));
        drop(segunda);
        assert!(reserve(&set(&[-902]), &name).is_ok());
    }
}
