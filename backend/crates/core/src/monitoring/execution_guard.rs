//! Controle de concorrência em tempo de execução e cálculo de timeout inteligente.
//!
//! Evita que duas checagens do mesmo monitor, consultas SNMP ao mesmo dispositivo
//! ou varreduras de portas no mesmo IP ocorram concorrentemente, prevenindo
//! colisões de socket, sobrecarga em equipamentos de rede e inconsistência de métricas.

use std::{
    collections::HashSet,
    hash::Hash,
    net::IpAddr,
    sync::{Mutex, OnceLock, PoisonError},
};

struct InFlightSet<K: Eq + Hash + Copy + 'static> {
    entries: Mutex<HashSet<K>>,
}

impl<K: Eq + Hash + Copy + 'static> InFlightSet<K> {
    fn new() -> Self {
        Self {
            entries: Mutex::new(HashSet::new()),
        }
    }

    fn try_acquire(&'static self, key: K) -> Option<InFlightGuard<K>> {
        let mut set = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        set.insert(key).then(|| InFlightGuard { set: self, key })
    }

    fn release(&self, key: K) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&key);
    }
}

#[derive(Debug)]
struct InFlightGuard<K: Eq + Hash + Copy + 'static> {
    set: &'static InFlightSet<K>,
    key: K,
}

impl<K: Eq + Hash + Copy + 'static> Drop for InFlightGuard<K> {
    fn drop(&mut self) {
        self.set.release(self.key);
    }
}

impl<K: Eq + Hash + Copy + 'static> std::fmt::Debug for InFlightSet<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InFlightSet").finish_non_exhaustive()
    }
}

fn in_flight_monitors() -> &'static InFlightSet<i64> {
    static SET: OnceLock<InFlightSet<i64>> = OnceLock::new();
    SET.get_or_init(InFlightSet::new)
}

fn in_flight_snmp_devices() -> &'static InFlightSet<i64> {
    static SET: OnceLock<InFlightSet<i64>> = OnceLock::new();
    SET.get_or_init(InFlightSet::new)
}

fn in_flight_port_scans() -> &'static InFlightSet<IpAddr> {
    static SET: OnceLock<InFlightSet<IpAddr>> = OnceLock::new();
    SET.get_or_init(InFlightSet::new)
}

fn in_flight_traceroutes() -> &'static InFlightSet<IpAddr> {
    static SET: OnceLock<InFlightSet<IpAddr>> = OnceLock::new();
    SET.get_or_init(InFlightSet::new)
}

fn in_flight_quick_pings() -> &'static InFlightSet<IpAddr> {
    static SET: OnceLock<InFlightSet<IpAddr>> = OnceLock::new();
    SET.get_or_init(InFlightSet::new)
}

fn in_flight_speedtest() -> &'static InFlightSet<()> {
    static SET: OnceLock<InFlightSet<()>> = OnceLock::new();
    SET.get_or_init(InFlightSet::new)
}

/// Guarda RAII que sinaliza a execução em curso de um monitor.
/// Libera o identificador automaticamente ao ser descartado.
#[derive(Debug)]
pub struct MonitorExecutionGuard(#[allow(dead_code)] InFlightGuard<i64>);

/// Tenta adquirir a trava de execução para o monitor informado.
/// Retorna `None` se já houver uma execução em andamento.
pub fn try_acquire_monitor(monitor_id: i64) -> Option<MonitorExecutionGuard> {
    in_flight_monitors()
        .try_acquire(monitor_id)
        .map(MonitorExecutionGuard)
}

/// Guarda RAII que sinaliza a consulta SNMP em curso para um dispositivo.
#[derive(Debug)]
pub struct DeviceSnmpExecutionGuard(#[allow(dead_code)] InFlightGuard<i64>);

/// Tenta adquirir a trava de consulta SNMP para o dispositivo informado.
pub fn try_acquire_snmp_device(device_id: i64) -> Option<DeviceSnmpExecutionGuard> {
    in_flight_snmp_devices()
        .try_acquire(device_id)
        .map(DeviceSnmpExecutionGuard)
}

/// Guarda RAII que sinaliza a varredura de portas em curso para um IP.
#[derive(Debug)]
pub struct PortScanExecutionGuard(#[allow(dead_code)] InFlightGuard<IpAddr>);

/// Tenta adquirir a trava de varredura de portas para o IP informado.
pub fn try_acquire_port_scan(ip: IpAddr) -> Option<PortScanExecutionGuard> {
    in_flight_port_scans()
        .try_acquire(ip)
        .map(PortScanExecutionGuard)
}

/// Guarda RAII que sinaliza o traceroute em curso para um IP.
#[derive(Debug)]
pub struct TracerouteExecutionGuard(#[allow(dead_code)] InFlightGuard<IpAddr>);

/// Tenta adquirir a trava de traceroute para o IP informado.
pub fn try_acquire_traceroute(ip: IpAddr) -> Option<TracerouteExecutionGuard> {
    in_flight_traceroutes()
        .try_acquire(ip)
        .map(TracerouteExecutionGuard)
}

/// Guarda RAII que sinaliza o diagnóstico rápido por ping em curso para um IP.
#[derive(Debug)]
pub struct QuickPingExecutionGuard(#[allow(dead_code)] InFlightGuard<IpAddr>);

/// Tenta adquirir a trava do diagnóstico rápido para o IP informado.
pub fn try_acquire_quick_ping(ip: IpAddr) -> Option<QuickPingExecutionGuard> {
    in_flight_quick_pings()
        .try_acquire(ip)
        .map(QuickPingExecutionGuard)
}

/// Guarda RAII que sinaliza o teste de velocidade em curso no servidor.
#[derive(Debug)]
pub struct SpeedTestExecutionGuard(#[allow(dead_code)] InFlightGuard<()>);

/// Tenta adquirir a trava do teste de velocidade.
pub fn try_acquire_speedtest() -> Option<SpeedTestExecutionGuard> {
    in_flight_speedtest()
        .try_acquire(())
        .map(SpeedTestExecutionGuard)
}

/// Converte o `timeout_seconds` salvo no monitor para um valor efetivo,
/// respeitando um mínimo de 1 s e um máximo de `interval - 1` (para não
/// ultrapassar o próximo ciclo).
#[must_use]
pub fn effective_timeout_seconds(timeout_seconds: i32, interval_seconds: i32) -> i32 {
    let interval = interval_seconds.max(1);
    let max_allowed = (interval - 1).max(1);
    timeout_seconds.max(1).min(max_allowed)
}

/// Calcula de forma inteligente o timeout ótimo em segundos com base no tipo
/// de monitor e no intervalo de verificação configurado.
#[must_use]
pub fn calculate_smart_timeout_seconds(kind: &str, interval_seconds: i32) -> i32 {
    let interval = interval_seconds.max(1);
    if interval <= 2 {
        return 1;
    }

    let max_allowed = (interval - 1).max(1);

    if kind.eq_ignore_ascii_case("http") || kind.eq_ignore_ascii_case("https") {
        // Requisições HTTP aceitam até metade do intervalo, limitado entre 3s e 10s
        let calculated = interval / 2;
        calculated.clamp(3, 10).min(max_allowed)
    } else {
        // Ping, TCP, DNS, SNMP e demais: fração segura de 1/3 do intervalo, entre 2s e 5s
        let calculated = interval / 3;
        calculated.clamp(2, 5).min(max_allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn monitor_execution_guard_adquire_e_libera() {
        let id = 999_991;
        let guard = try_acquire_monitor(id);
        assert!(guard.is_some());
        assert!(try_acquire_monitor(id).is_none());

        drop(guard);
        let second_guard = try_acquire_monitor(id);
        assert!(second_guard.is_some());
    }

    #[test]
    fn snmp_device_guard_adquire_e_libera() {
        let id = 999_992;
        let guard = try_acquire_snmp_device(id);
        assert!(guard.is_some());
        assert!(try_acquire_snmp_device(id).is_none());

        drop(guard);
        assert!(try_acquire_snmp_device(id).is_some());
    }

    #[test]
    fn port_scan_guard_adquire_e_libera() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 100, 50));
        let guard = try_acquire_port_scan(ip);
        assert!(guard.is_some());
        assert!(try_acquire_port_scan(ip).is_none());

        drop(guard);
        assert!(try_acquire_port_scan(ip).is_some());
    }

    #[test]
    fn quick_ping_guard_adquire_e_libera() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 100, 51));
        let guard = try_acquire_quick_ping(ip);
        assert!(guard.is_some());
        assert!(try_acquire_quick_ping(ip).is_none());

        drop(guard);
        assert!(try_acquire_quick_ping(ip).is_some());
    }

    #[test]
    fn calcula_timeout_inteligente_respeitando_limites() {
        // Intervalos muito curtos
        assert_eq!(calculate_smart_timeout_seconds("ping", 1), 1);
        assert_eq!(calculate_smart_timeout_seconds("ping", 2), 1);
        assert_eq!(calculate_smart_timeout_seconds("ping", 3), 2);
        assert_eq!(calculate_smart_timeout_seconds("ping", 5), 2);

        // Intervalos padrão
        assert_eq!(calculate_smart_timeout_seconds("ping", 10), 3);
        assert_eq!(calculate_smart_timeout_seconds("ping", 15), 5);
        assert_eq!(calculate_smart_timeout_seconds("ping", 60), 5);

        // HTTP
        assert_eq!(calculate_smart_timeout_seconds("http", 5), 3);
        assert_eq!(calculate_smart_timeout_seconds("http", 10), 5);
        assert_eq!(calculate_smart_timeout_seconds("http", 15), 7);
        assert_eq!(calculate_smart_timeout_seconds("http", 60), 10);
    }

    #[test]
    fn timeout_efetivo_respeita_minimo_e_intervalo() {
        // Menor que 1 s vira 1 s.
        assert_eq!(effective_timeout_seconds(0, 60), 1);
        assert_eq!(effective_timeout_seconds(-5, 60), 1);

        // Dentro do intervalo é preservado.
        assert_eq!(effective_timeout_seconds(5, 60), 5);
        assert_eq!(effective_timeout_seconds(10, 60), 10);

        // Nunca ultrapassa interval - 1.
        assert_eq!(effective_timeout_seconds(60, 60), 59);
        assert_eq!(effective_timeout_seconds(120, 60), 59);

        // Intervalo curto: teto é 1 s.
        assert_eq!(effective_timeout_seconds(5, 2), 1);
    }
}
