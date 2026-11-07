//! Estado de um enlace derivado das pontas: status, velocidade, utilização e
//! envelhecimento. Funções puras — o serviço só junta as linhas do banco.

use chrono::{DateTime, Duration, FixedOffset, Utc};

/// Enlace descoberto por LLDP/CDP que não é revisto há este tempo é marcado
/// como "obsoleto" na resposta. Não é apagado: pode ser só o equipamento que
/// parou de responder SNMP, e a decisão de remover é do operador.
pub const STALE_AFTER_DAYS: i64 = 7;

/// Métodos que são **observação** do cabeamento — os únicos que envelhecem.
/// Enlace manual (`user_defined`), inferido ou de hierarquia nunca fica obsoleto.
pub const OBSERVED_METHODS: [&str; 2] = ["lldp", "cdp"];

/// Leitura de uma ponta do enlace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Up,
    Down,
    Unknown,
}

/// Interpreta o `ifOperStatus` gravado (rótulos de `snmp::collectors::status_label`).
fn interface_side(oper: Option<&str>, admin: Option<&str>) -> Side {
    match oper {
        Some("up" | "dormant") => Side::Up,
        Some("down" | "notPresent" | "lowerLayerDown") => Side::Down,
        // Sem oper conhecido, só um `admin down` explícito diz algo.
        _ if admin == Some("down") => Side::Down,
        _ => Side::Unknown,
    }
}

/// Interpreta o status agregado do dispositivo (`DeviceStatus::as_str`).
fn device_side(status: Option<&str>) -> Side {
    match status {
        Some("online" | "warning") => Side::Up,
        Some("offline") => Side::Down,
        _ => Side::Unknown,
    }
}

/// Uma ponta do enlace: o status da interface (quando o enlace a conhece) e o
/// do dispositivo.
#[derive(Debug, Clone, Copy, Default)]
pub struct EndpointState<'a> {
    pub oper_status: Option<&'a str>,
    pub admin_status: Option<&'a str>,
    pub device_status: Option<&'a str>,
}

/// Status do enlace: `up`, `down` ou `unknown`.
///
/// - qualquer dispositivo da ponta `offline` → `down`;
/// - qualquer interface caída → `down`;
/// - alguma interface conhecida `up` (e nenhuma caída) → `up`;
/// - sem interface conhecida, os dois dispositivos no ar → `up` (não há o que
///   contradiga o enlace); senão `unknown`.
#[must_use]
pub fn edge_status(source: EndpointState<'_>, target: EndpointState<'_>) -> &'static str {
    let devices = [
        device_side(source.device_status),
        device_side(target.device_status),
    ];
    let interfaces = [
        interface_side(source.oper_status, source.admin_status),
        interface_side(target.oper_status, target.admin_status),
    ];
    if devices.contains(&Side::Down) || interfaces.contains(&Side::Down) {
        return "down";
    }
    if interfaces.contains(&Side::Up) || devices.iter().all(|side| *side == Side::Up) {
        return "up";
    }
    "unknown"
}

/// Velocidade do enlace em bps: a menor das velocidades conhecidas das pontas
/// (um enlace negocia pelo lado mais lento). Zero ou negativo é "desconhecido".
#[must_use]
pub fn edge_speed_bps(source: Option<i64>, target: Option<i64>) -> Option<i64> {
    [source, target]
        .into_iter()
        .flatten()
        .filter(|speed| *speed > 0)
        .min()
}

/// Utilização do enlace, de 0 a 1: o sentido mais carregado sobre a velocidade.
#[must_use]
pub fn edge_utilization(
    in_bps: Option<f64>,
    out_bps: Option<f64>,
    speed_bps: Option<i64>,
) -> Option<f64> {
    let speed = speed_bps.filter(|speed| *speed > 0)?;
    let busiest = [in_bps, out_bps]
        .into_iter()
        .flatten()
        .filter(|value| value.is_finite())
        .reduce(f64::max)?;
    #[allow(clippy::cast_precision_loss)]
    let ratio = busiest / speed as f64;
    Some(ratio.clamp(0.0, 1.0))
}

/// O enlace observado deixou de ser revisto há mais de [`STALE_AFTER_DAYS`]?
#[must_use]
pub fn is_stale(
    discovery_method: &str,
    last_seen_at: Option<&DateTime<FixedOffset>>,
    now: DateTime<Utc>,
) -> bool {
    OBSERVED_METHODS.contains(&discovery_method)
        && last_seen_at
            .is_some_and(|seen| now - seen.with_timezone(&Utc) > Duration::days(STALE_AFTER_DAYS))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ponta<'a>(oper: Option<&'a str>, device: Option<&'a str>) -> EndpointState<'a> {
        EndpointState {
            oper_status: oper,
            admin_status: None,
            device_status: device,
        }
    }

    #[test]
    fn interfaces_no_ar_deixam_o_enlace_up() {
        assert_eq!(
            edge_status(
                ponta(Some("up"), Some("online")),
                ponta(Some("up"), Some("online"))
            ),
            "up"
        );
        // Uma ponta sem interface cadastrada não derruba a outra.
        assert_eq!(
            edge_status(ponta(Some("up"), None), ponta(None, None)),
            "up"
        );
    }

    #[test]
    fn qualquer_interface_caida_derruba_o_enlace() {
        for caida in ["down", "lowerLayerDown", "notPresent"] {
            assert_eq!(
                edge_status(
                    ponta(Some("up"), Some("online")),
                    ponta(Some(caida), Some("online"))
                ),
                "down",
                "{caida}"
            );
        }
        let admin_down = EndpointState {
            oper_status: None,
            admin_status: Some("down"),
            device_status: Some("online"),
        };
        assert_eq!(edge_status(admin_down, ponta(None, Some("online"))), "down");
    }

    #[test]
    fn dispositivo_offline_derruba_o_enlace_mesmo_com_porta_up() {
        // A última leitura da porta é anterior à queda do equipamento.
        assert_eq!(
            edge_status(
                ponta(Some("up"), Some("offline")),
                ponta(Some("up"), Some("online"))
            ),
            "down"
        );
    }

    #[test]
    fn sem_interface_vale_o_status_dos_dispositivos() {
        assert_eq!(
            edge_status(ponta(None, Some("online")), ponta(None, Some("warning"))),
            "up"
        );
        assert_eq!(
            edge_status(ponta(None, Some("online")), ponta(None, Some("unknown"))),
            "unknown"
        );
        assert_eq!(
            edge_status(ponta(Some("testing"), None), ponta(None, None)),
            "unknown"
        );
    }

    #[test]
    fn velocidade_e_a_menor_conhecida() {
        assert_eq!(
            edge_speed_bps(Some(1_000_000_000), Some(100_000_000)),
            Some(100_000_000)
        );
        assert_eq!(edge_speed_bps(None, Some(10)), Some(10));
        assert_eq!(edge_speed_bps(Some(0), None), None);
        assert_eq!(edge_speed_bps(None, None), None);
    }

    #[test]
    fn utilizacao_usa_o_sentido_mais_carregado() {
        let uso = edge_utilization(Some(10_000_000.0), Some(50_000_000.0), Some(100_000_000))
            .expect("utilização");
        assert!((uso - 0.5).abs() < f64::EPSILON);
        // Contador estourado não passa de 100%.
        assert_eq!(
            edge_utilization(Some(5e9), None, Some(1_000_000_000)),
            Some(1.0)
        );
        assert_eq!(edge_utilization(Some(1.0), None, None), None);
        assert_eq!(edge_utilization(None, None, Some(1000)), None);
    }

    #[test]
    fn so_enlace_observado_envelhece() {
        let agora = Utc::now();
        let antigo: DateTime<FixedOffset> = (agora - Duration::days(STALE_AFTER_DAYS + 1)).into();
        let recente: DateTime<FixedOffset> = (agora - Duration::hours(1)).into();

        assert!(is_stale("lldp", Some(&antigo), agora));
        assert!(is_stale("cdp", Some(&antigo), agora));
        assert!(!is_stale("lldp", Some(&recente), agora));
        assert!(!is_stale("user_defined", Some(&antigo), agora));
        assert!(!is_stale("subnet_inference", Some(&antigo), agora));
        assert!(!is_stale("lldp", None, agora));
    }
}
