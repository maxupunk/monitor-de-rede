//! Política de confirmação para falhas de ping e preparação de sua execução.
//!
//! O checker ICMP mede uma tentativa. Este módulo coordena retentativas e usa
//! respostas TCP somente como evidência positiva de que o host continua vivo.

use std::{collections::HashSet, net::IpAddr, time::Duration};

use chrono::Utc;
use futures::future::join_all;
use loco_rs::app::AppContext;
use sea_orm::{
    ColumnTrait, EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    models::{
        _entities::{discovery_results as discovery_results_entity, discovery_runs},
        devices, discovery_results, monitors,
    },
    services::{
        monitoring::{
            checkers::ping::{PingChecker, PingConfig},
            contracts::{CheckResult, Checker, MonitorStatus},
        },
        network_tools::tcp_probe::{probe_tcp, TcpProbeObservation, TcpProbeState},
        shared::errors::AppResult,
    },
};

pub const REACHABILITY_CAUSE: &str = "reachabilityCause";
pub const ICMP_FILTERED: &str = "icmp_filtered";
const DIAGNOSTICS_KEY: &str = "_diagnostics";
const MAX_CANDIDATE_PORTS: usize = 3;
const MAX_RETRIES: u8 = 5;
const DEFAULT_TCP_TIMEOUT_MS: u64 = 1_500;
const TCP_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Opções internas anexadas apenas à configuração enviada para execução.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingDiagnosticsConfig {
    #[serde(default)]
    pub tcp_ports: Vec<u16>,
    #[serde(default)]
    pub retry_count: u8,
    #[serde(default = "default_tcp_timeout_ms")]
    pub timeout_ms: u64,
}

const fn default_tcp_timeout_ms() -> u64 {
    DEFAULT_TCP_TIMEOUT_MS
}

/// Acrescenta política e portas candidatas sem alterar a configuração salva.
pub async fn prepare_configuration(
    ctx: &AppContext,
    monitor: &monitors::Model,
) -> AppResult<Value> {
    if !monitor.r#type.eq_ignore_ascii_case("ping") {
        return Ok(monitor.configuration.clone());
    }

    let tcp_ports = tcp_monitor_ports(ctx, monitor).await?;
    let discovered_ports = discovery_ports(ctx, monitor).await?;
    let ports = select_candidate_ports(&tcp_ports, &discovered_ports);
    let mut configuration = monitor.configuration.clone();
    if let Value::Object(object) = &mut configuration {
        object.insert(
            DIAGNOSTICS_KEY.into(),
            json!({
                "tcpPorts": ports,
                "retryCount": monitor.retry_count.clamp(0, i32::from(MAX_RETRIES)),
                "timeoutMs": (u64::from(monitor.timeout_seconds.max(1) as u32) * 1_000)
                    .min(DEFAULT_TCP_TIMEOUT_MS),
            }),
        );
    }
    Ok(configuration)
}

async fn tcp_monitor_ports(ctx: &AppContext, monitor: &monitors::Model) -> AppResult<Vec<u16>> {
    let mut query = monitors::Entity::find()
        .filter(monitors::Column::Type.eq("tcp"))
        .filter(monitors::Column::Enabled.eq(true))
        .filter(monitors::Column::ProbeId.eq(monitor.probe_id));
    query = match monitor.device_id {
        Some(device_id) => query.filter(monitors::Column::DeviceId.eq(device_id)),
        None => query.filter(monitors::Column::DeviceId.is_null()),
    };
    let ping_host = monitor
        .configuration
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut rows = query.all(&ctx.db).await?;
    rows.sort_by_key(|row| (row.status != "up", row.id));
    Ok(rows
        .into_iter()
        .filter(|row| {
            monitor.device_id.is_some()
                || row.configuration.get("host").and_then(Value::as_str) == Some(ping_host)
        })
        .filter_map(|row| row.port().and_then(|port| u16::try_from(port).ok()))
        .collect())
}

async fn discovery_ports(ctx: &AppContext, monitor: &monitors::Model) -> AppResult<Vec<u16>> {
    let target_ip = if let Some(ip) = monitor
        .configuration
        .get("host")
        .and_then(Value::as_str)
        .and_then(|host| host.parse::<IpAddr>().ok())
    {
        Some(ip.to_string())
    } else if let Some(device_id) = monitor.device_id {
        devices::Entity::find_by_id(device_id)
            .one(&ctx.db)
            .await?
            .and_then(|device| device.ip_address)
    } else {
        None
    };
    let Some(target_ip) = target_ip else {
        return Ok(Vec::new());
    };

    let row = discovery_results::Entity::find()
        .join(
            JoinType::InnerJoin,
            discovery_results_entity::Relation::DiscoveryRuns.def(),
        )
        .filter(discovery_results_entity::Column::IpAddress.eq(target_ip))
        .filter(discovery_runs::Column::ProbeId.eq(monitor.probe_id))
        .filter(discovery_runs::Column::Status.eq("completed"))
        .order_by_desc(discovery_runs::Column::FinishedAt)
        .one(&ctx.db)
        .await?;
    Ok(row
        .and_then(|row| row.data)
        .and_then(|data| data.get("openPorts").cloned())
        .and_then(|ports| ports.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|port| port.as_u64().and_then(|port| u16::try_from(port).ok()))
        .collect())
}

#[must_use]
fn select_candidate_ports(tcp_ports: &[u16], discovered_ports: &[u16]) -> Vec<u16> {
    let mut seen = HashSet::new();
    tcp_ports
        .iter()
        .chain(discovered_ports)
        .copied()
        .filter(|port| *port > 0 && seen.insert(*port))
        .take(MAX_CANDIDATE_PORTS)
        .collect()
}

/// Executa o ping com retentativas e diagnostica somente a perda ICMP total.
pub async fn execute_ping(checker: &PingChecker, config: PingConfig) -> CheckResult {
    let execution_started = Utc::now();
    let attempts = 1 + config.diagnostics.retry_count.min(MAX_RETRIES);
    let mut used = 0_u8;
    let mut result;
    loop {
        used += 1;
        result = checker.execute(config.clone()).await;
        if !is_icmp_no_reply(&result) || used >= attempts {
            break;
        }
    }

    if used > 1 {
        ensure_data_object(&mut result).insert("attempts".into(), json!(used));
    }
    if is_icmp_no_reply(&result) {
        apply_tcp_confirmation(&mut result, &config).await;
    }
    let finished_at = Utc::now();
    result.started_at = execution_started;
    result.finished_at = finished_at;
    result.duration_ms = (finished_at - execution_started).num_milliseconds().max(0);
    result
}

fn is_icmp_no_reply(result: &CheckResult) -> bool {
    result.status == MonitorStatus::Down
        && result.data.get("failureKind").and_then(Value::as_str) == Some("icmp_no_reply")
}

async fn apply_tcp_confirmation(result: &mut CheckResult, config: &PingConfig) {
    let Some(ip) = result
        .data
        .get("resolvedIp")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<IpAddr>().ok())
    else {
        return;
    };
    let timeout = Duration::from_millis(
        config
            .diagnostics
            .timeout_ms
            .clamp(1, DEFAULT_TCP_TIMEOUT_MS),
    );
    let observations = join_all(
        config
            .diagnostics
            .tcp_ports
            .iter()
            .copied()
            .take(MAX_CANDIDATE_PORTS)
            .map(|port| probe_with_retry(ip, port, timeout)),
    )
    .await;
    apply_tcp_observations(result, &observations);
}

fn apply_tcp_observations(
    result: &mut CheckResult,
    observations: &[(u16, u8, TcpProbeObservation)],
) {
    let evidence: Vec<Value> = observations
        .iter()
        .map(|(port, attempts, observation)| {
            json!({
                "port": port,
                "status": observation.state.as_str(),
                "attempts": attempts,
                "latencyMs": observation.latency_ms,
                "error": observation.error,
            })
        })
        .collect();
    let proof = observations
        .iter()
        .find(|(_, _, observation)| observation.state.proves_reachability());
    let data = ensure_data_object(result);
    data.insert("tcpEvidence".into(), Value::Array(evidence));

    let Some((port, _, _)) = proof else {
        data.insert("tcpConfirmation".into(), json!("inconclusive"));
        return;
    };
    data.insert(REACHABILITY_CAUSE.into(), json!(ICMP_FILTERED));
    data.insert("tcpConfirmation".into(), json!("reachable"));
    result.status = MonitorStatus::Warning;
    result.success = true;
    result.message = Some(format!(
        "O host responde via TCP na porta {port}, mas não responde ao ICMP; o ICMP pode estar filtrado ou desativado."
    ));
}

async fn probe_with_retry(
    ip: IpAddr,
    port: u16,
    timeout: Duration,
) -> (u16, u8, TcpProbeObservation) {
    let address = std::net::SocketAddr::new(ip, port);
    let mut attempts = 1;
    let mut observation = probe_tcp(address, timeout).await;
    if observation.state == TcpProbeState::Filtered {
        tokio::time::sleep(TCP_RETRY_DELAY).await;
        attempts += 1;
        observation = probe_tcp(address, timeout).await;
    }
    (port, attempts, observation)
}

fn ensure_data_object(result: &mut CheckResult) -> &mut serde_json::Map<String, Value> {
    if !result.data.is_object() {
        result.data = json!({});
    }
    result.data.as_object_mut().expect("objeto recém-criado")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::monitoring::contracts::CheckMetric;

    #[test]
    fn portas_tcp_tem_prioridade_e_o_limite_e_tres() {
        assert_eq!(
            select_candidate_ports(&[443, 22, 443], &[80, 8080]),
            vec![443, 22, 80]
        );
    }

    #[test]
    fn portas_invalidas_e_repetidas_sao_descartadas() {
        assert_eq!(select_candidate_ports(&[0, 22], &[22, 443]), vec![22, 443]);
    }

    fn ping_sem_resposta() -> CheckResult {
        let now = Utc::now();
        CheckResult {
            success: false,
            status: MonitorStatus::Down,
            started_at: now,
            finished_at: now,
            duration_ms: 1_000,
            message: Some("Host inacessível".into()),
            metrics: vec![CheckMetric {
                name: "packet_loss".into(),
                value: 100.0,
                unit: "%".into(),
            }],
            data: json!({ "failureKind": "icmp_no_reply", "resolvedIp": "127.0.0.1" }),
        }
    }

    fn observation(state: TcpProbeState) -> TcpProbeObservation {
        TcpProbeObservation {
            state,
            latency_ms: 1.0,
            error: None,
        }
    }

    #[test]
    fn porta_aberta_transforma_queda_em_icmp_filtrado() {
        let mut result = ping_sem_resposta();
        apply_tcp_observations(&mut result, &[(22, 1, observation(TcpProbeState::Open))]);
        assert_eq!(result.status, MonitorStatus::Warning);
        assert!(result.success);
        assert_eq!(result.data[REACHABILITY_CAUSE], ICMP_FILTERED);
        assert!(result.message.unwrap().contains("responde via TCP"));
    }

    #[test]
    fn porta_fechada_tambem_prova_vida_sem_afirmar_que_esta_bloqueada() {
        let mut result = ping_sem_resposta();
        apply_tcp_observations(&mut result, &[(22, 1, observation(TcpProbeState::Closed))]);
        assert_eq!(result.status, MonitorStatus::Warning);
        assert_eq!(result.data[REACHABILITY_CAUSE], ICMP_FILTERED);
        assert!(result.message.unwrap().contains("responde via TCP"));
    }

    #[test]
    fn silencio_e_inacessibilidade_tcp_mantem_down_inconclusivo() {
        for state in [
            TcpProbeState::Filtered,
            TcpProbeState::Unreachable,
            TcpProbeState::Error,
        ] {
            let mut result = ping_sem_resposta();
            apply_tcp_observations(&mut result, &[(22, 2, observation(state))]);
            assert_eq!(result.status, MonitorStatus::Down);
            assert!(!result.success);
            assert_eq!(result.data["tcpConfirmation"], "inconclusive");
            assert!(result.data.get(REACHABILITY_CAUSE).is_none());
        }
    }

    #[test]
    fn ausencia_de_portas_registra_evidencia_vazia_e_mantem_down() {
        let mut result = ping_sem_resposta();
        apply_tcp_observations(&mut result, &[]);
        assert_eq!(result.status, MonitorStatus::Down);
        assert_eq!(result.data["tcpEvidence"], json!([]));
        assert_eq!(result.data["tcpConfirmation"], "inconclusive");
    }

    #[test]
    fn perda_parcial_e_falhas_de_infraestrutura_nao_acionam_diagnostico_tcp() {
        let mut partial = ping_sem_resposta();
        partial.status = MonitorStatus::Warning;
        partial.data = json!({ "failureKind": "packet_loss" });
        assert!(!is_icmp_no_reply(&partial));

        for failure_kind in ["dns_timeout", "dns_error", "icmp_socket_error"] {
            let mut failure = ping_sem_resposta();
            failure.data = json!({ "failureKind": failure_kind });
            assert!(!is_icmp_no_reply(&failure));
        }
    }
}
