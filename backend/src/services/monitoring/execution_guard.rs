//! Controle de concorrência em tempo de execução e cálculo de timeout inteligente.
//!
//! Evita que duas checagens do mesmo monitor, consultas SNMP ao mesmo dispositivo
//! ou varreduras de portas no mesmo IP ocorram concorrentemente, prevenindo
//! colisões de socket, sobrecarga em equipamentos de rede e inconsistência de métricas.

use std::{
    collections::HashSet,
    net::IpAddr,
    sync::{Mutex, OnceLock},
};

fn in_flight_monitors() -> &'static Mutex<HashSet<i64>> {
    static SET: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

fn in_flight_snmp_devices() -> &'static Mutex<HashSet<i64>> {
    static SET: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

fn in_flight_port_scans() -> &'static Mutex<HashSet<IpAddr>> {
    static SET: OnceLock<Mutex<HashSet<IpAddr>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Guarda RAII que sinaliza a execução em curso de um monitor.
/// Libera o identificador automaticamente ao ser descartado.
#[derive(Debug)]
pub struct MonitorExecutionGuard(i64);

impl Drop for MonitorExecutionGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = in_flight_monitors().lock() {
            set.remove(&self.0);
        }
    }
}

/// Tenta adquirir a trava de execução para o monitor informado.
/// Retorna `None` se já houver uma execução em andamento.
pub fn try_acquire_monitor(monitor_id: i64) -> Option<MonitorExecutionGuard> {
    let mut set = in_flight_monitors().lock().ok()?;
    if set.insert(monitor_id) {
        Some(MonitorExecutionGuard(monitor_id))
    } else {
        None
    }
}

/// Guarda RAII que sinaliza a consulta SNMP em curso para um dispositivo.
#[derive(Debug)]
pub struct DeviceSnmpExecutionGuard(i64);

impl Drop for DeviceSnmpExecutionGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = in_flight_snmp_devices().lock() {
            set.remove(&self.0);
        }
    }
}

/// Tenta adquirir a trava de consulta SNMP para o dispositivo informado.
pub fn try_acquire_snmp_device(device_id: i64) -> Option<DeviceSnmpExecutionGuard> {
    let mut set = in_flight_snmp_devices().lock().ok()?;
    if set.insert(device_id) {
        Some(DeviceSnmpExecutionGuard(device_id))
    } else {
        None
    }
}

/// Guarda RAII que sinaliza a varredura de portas em curso para um IP.
#[derive(Debug)]
pub struct PortScanExecutionGuard(IpAddr);

impl Drop for PortScanExecutionGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = in_flight_port_scans().lock() {
            set.remove(&self.0);
        }
    }
}

/// Tenta adquirir a trava de varredura de portas para o IP informado.
pub fn try_acquire_port_scan(ip: IpAddr) -> Option<PortScanExecutionGuard> {
    let mut set = in_flight_port_scans().lock().ok()?;
    if set.insert(ip) {
        Some(PortScanExecutionGuard(ip))
    } else {
        None
    }
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

    match kind.to_lowercase().as_str() {
        "ping" | "tcp" | "dns" | "snmp" => {
            // Fração segura de 1/3 do intervalo, limitada entre 2s e 5s
            let calculated = interval / 3;
            calculated.clamp(2, 5).min(max_allowed)
        }
        "http" | "https" => {
            // Requisições HTTP aceitam até metade do intervalo, limitado entre 3s e 10s
            let calculated = interval / 2;
            calculated.clamp(3, 10).min(max_allowed)
        }
        _ => {
            let calculated = interval / 3;
            calculated.clamp(2, 5).min(max_allowed)
        }
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
