//! Checker SNMP para uptime, estado RFC 2863 e contadores de interface.

use chrono::Utc;
use serde::Deserialize;

use crate::services::{
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
    snmp::{
        client::{SnmpClient, SnmpConfig, SnmpError, SnmpVersion},
        collectors::{collect_cpu, collect_memory, status_label, OID_SYS_UPTIME},
    },
};

const OID_IF_ADMIN_STATUS: &str = "1.3.6.1.2.1.2.2.1.7";
const OID_IF_OPER_STATUS: &str = "1.3.6.1.2.1.2.2.1.8";
const OID_IF_IN_OCTETS: &str = "1.3.6.1.2.1.2.2.1.10";
const OID_IF_OUT_OCTETS: &str = "1.3.6.1.2.1.2.2.1.16";
const OID_IF_HC_IN_OCTETS: &str = "1.3.6.1.2.1.31.1.1.1.6";
const OID_IF_HC_OUT_OCTETS: &str = "1.3.6.1.2.1.31.1.1.1.10";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpCheckerConfig {
    pub host: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_community")]
    pub community: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub if_index: Option<i32>,
    #[serde(default)]
    pub if_name: Option<String>,
}
fn default_version() -> String {
    "v2c".into()
}
fn default_community() -> String {
    "public".into()
}
const fn default_port() -> u16 {
    161
}
const fn default_timeout() -> u64 {
    4_000
}

struct SnmpObservation {
    metric: String,
    status: MonitorStatus,
    metrics: Vec<CheckMetric>,
    data: serde_json::Value,
}

pub struct SnmpChecker;
#[async_trait::async_trait]
impl Checker for SnmpChecker {
    type Config = SnmpCheckerConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let result = execute_query(&config).await;
        let finished_at = Utc::now();
        match result {
            Ok(observation) => CheckResult {
                success: matches!(
                    observation.status,
                    MonitorStatus::Up | MonitorStatus::Disabled
                ),
                status: observation.status,
                started_at,
                finished_at,
                duration_ms: (finished_at - started_at).num_milliseconds().max(0),
                message: Some(format!("Consulta SNMP {} concluida", observation.metric)),
                metrics: observation.metrics,
                data: observation.data,
            },
            Err(error) => check_result_from_snmp_error(error, started_at, finished_at),
        }
    }
}

fn check_result_from_snmp_error(
    error: SnmpError,
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
) -> CheckResult {
    let status = if matches!(error, SnmpError::Timeout) {
        MonitorStatus::Unknown
    } else {
        MonitorStatus::Down
    };
    CheckResult {
        success: false,
        status,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message: Some(format!("Falha na consulta SNMP: {error}")),
        metrics: vec![],
        data: serde_json::json!({}),
    }
}

async fn execute_query(config: &SnmpCheckerConfig) -> Result<SnmpObservation, SnmpError> {
    let version = SnmpVersion::parse(&config.version)
        .ok_or_else(|| SnmpError::InvalidConfig("Versão SNMP inválida".into()))?;
    let client = SnmpClient::new(SnmpConfig {
        host: config.host.clone(),
        version,
        community: config.community.clone(),
        username: None,
        auth_protocol: None,
        auth_key: None,
        priv_protocol: None,
        priv_key: None,
        port: config.port,
        timeout_ms: config.timeout_ms,
    });
    let metric = config.metric.as_deref().unwrap_or("uptime");
    match metric {
        "interface_status" | "status" => interface_status(&client, config, metric).await,
        "traffic" | "interface_traffic" => interface_traffic(&client, config).await,
        "uptime" => uptime(&client).await,
        "cpu_usage" => cpu_usage(&client).await,
        "memory_usage" => memory_usage(&client).await,
        _ => Err(SnmpError::InvalidConfig("Métrica SNMP inválida".into())),
    }
}

async fn uptime(client: &SnmpClient) -> Result<SnmpObservation, SnmpError> {
    let values = client.get(&[OID_SYS_UPTIME]).await?;
    let value = value(&values, OID_SYS_UPTIME)?;
    Ok(SnmpObservation {
        metric: "uptime".into(),
        status: MonitorStatus::Up,
        metrics: vec![CheckMetric {
            name: "snmp_uptime".into(),
            value: value as f64,
            unit: "ticks".into(),
        }],
        data: serde_json::json!({}),
    })
}

async fn cpu_usage(client: &SnmpClient) -> Result<SnmpObservation, SnmpError> {
    let cpu = collect_cpu(client).await?;
    let usage = cpu
        .usage_percent
        .ok_or_else(|| SnmpError::InvalidConfig("O agente SNMP não informou uso de CPU".into()))?;
    Ok(SnmpObservation {
        metric: "cpu_usage".into(),
        status: MonitorStatus::Up,
        metrics: vec![CheckMetric {
            name: "cpu_usage".into(),
            value: usage,
            unit: "percent".into(),
        }],
        data: serde_json::json!({}),
    })
}

async fn memory_usage(client: &SnmpClient) -> Result<SnmpObservation, SnmpError> {
    let memory = collect_memory(client).await?;
    let usage = memory.used_percent.ok_or_else(|| {
        SnmpError::InvalidConfig("O agente SNMP não informou memória total e disponível".into())
    })?;
    Ok(SnmpObservation {
        metric: "memory_usage".into(),
        status: MonitorStatus::Up,
        metrics: vec![CheckMetric {
            name: "memory_usage".into(),
            value: usage,
            unit: "percent".into(),
        }],
        data: serde_json::json!({
            "totalKb": memory.total_kb,
            "usedKb": memory.used_kb,
        }),
    })
}

async fn interface_status(
    client: &SnmpClient,
    config: &SnmpCheckerConfig,
    metric: &str,
) -> Result<SnmpObservation, SnmpError> {
    let index = resolve_interface_index(client, config).await?;
    let admin_oid = format!("{OID_IF_ADMIN_STATUS}.{index}");
    let oper_oid = format!("{OID_IF_OPER_STATUS}.{index}");
    let values = client.get(&[&admin_oid, &oper_oid]).await?;
    let admin = value(&values, &admin_oid)?;
    let oper = value(&values, &oper_oid)?;
    Ok(SnmpObservation {
        metric: metric.into(),
        status: interface_monitor_status(admin, oper),
        metrics: vec![
            CheckMetric {
                name: "if_admin_status".into(),
                value: admin as f64,
                unit: "value".into(),
            },
            CheckMetric {
                name: "if_oper_status".into(),
                value: oper as f64,
                unit: "value".into(),
            },
        ],
        data: serde_json::json!({
            "ifName": config.if_name,
            "ifIndex": index,
            "adminStatus": status_label(admin),
            "operStatus": status_label(oper),
        }),
    })
}

async fn interface_traffic(
    client: &SnmpClient,
    config: &SnmpCheckerConfig,
) -> Result<SnmpObservation, SnmpError> {
    let index = resolve_interface_index(client, config).await?;
    let hc_in = format!("{OID_IF_HC_IN_OCTETS}.{index}");
    let hc_out = format!("{OID_IF_HC_OUT_OCTETS}.{index}");
    let low_in = format!("{OID_IF_IN_OCTETS}.{index}");
    let low_out = format!("{OID_IF_OUT_OCTETS}.{index}");
    let values = client.get(&[&hc_in, &hc_out, &low_in, &low_out]).await?;
    let high_capacity = values
        .get(&hc_in)
        .and_then(Option::as_ref)
        .and_then(|v| v.number())
        .zip(
            values
                .get(&hc_out)
                .and_then(Option::as_ref)
                .and_then(|v| v.number()),
        );
    let (in_octets, out_octets, bits) = if let Some((in_octets, out_octets)) = high_capacity {
        (in_octets, out_octets, 64)
    } else {
        (value(&values, &low_in)?, value(&values, &low_out)?, 32)
    };
    Ok(SnmpObservation {
        metric: "traffic".into(),
        status: MonitorStatus::Up,
        metrics: vec![
            CheckMetric {
                name: if bits == 64 {
                    "ifHCInOctets"
                } else {
                    "ifInOctets"
                }
                .into(),
                value: in_octets as f64,
                unit: "bytes".into(),
            },
            CheckMetric {
                name: if bits == 64 {
                    "ifHCOutOctets"
                } else {
                    "ifOutOctets"
                }
                .into(),
                value: out_octets as f64,
                unit: "bytes".into(),
            },
        ],
        data: serde_json::json!({ "ifName": config.if_name, "ifIndex": index, "counterBits": bits }),
    })
}

async fn resolve_interface_index(
    client: &SnmpClient,
    config: &SnmpCheckerConfig,
) -> Result<i32, SnmpError> {
    if let Some(index) = config.if_index.filter(|index| *index > 0) {
        return Ok(index);
    }
    let Some(target_name) = config
        .if_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Err(SnmpError::InvalidConfig(
            "ifIndex ou ifName é obrigatório".into(),
        ));
    };

    let (base_descr, x_name) = tokio::join!(
        client.walk("1.3.6.1.2.1.2.2.1.2"),
        client.walk("1.3.6.1.2.1.31.1.1.1.1")
    );
    let entries = x_name
        .unwrap_or_default()
        .into_iter()
        .chain(base_descr.unwrap_or_default());
    for entry in entries {
        if let Some((_, index)) =
            crate::services::snmp::collectors::oid_column_and_index(&entry.oid)
        {
            let name = entry.value.text();
            if name.trim().eq_ignore_ascii_case(target_name) {
                return Ok(index);
            }
        }
    }
    Err(SnmpError::InvalidConfig(format!(
        "Interface '{target_name}' não encontrada no agente SNMP"
    )))
}

fn value(
    values: &std::collections::BTreeMap<String, Option<crate::services::snmp::client::SnmpValue>>,
    oid: &str,
) -> Result<u64, SnmpError> {
    values
        .get(oid)
        .and_then(Option::as_ref)
        .and_then(|item| item.number())
        .ok_or_else(|| SnmpError::Oid(format!("OID {oid} não retornou valor numérico")))
}

fn interface_monitor_status(admin: u64, oper: u64) -> MonitorStatus {
    if admin == 2 {
        MonitorStatus::Disabled
    } else if oper == 1 {
        MonitorStatus::Up
    } else if oper == 2 {
        MonitorStatus::Down
    } else {
        MonitorStatus::Warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_desabilitada_nao_gera_alarme() {
        assert_eq!(interface_monitor_status(2, 2), MonitorStatus::Disabled);
    }

    #[test]
    fn mapeia_estado_operacional_rfc_2863() {
        assert_eq!(interface_monitor_status(1, 1), MonitorStatus::Up);
        assert_eq!(interface_monitor_status(1, 2), MonitorStatus::Down);
        assert_eq!(interface_monitor_status(1, 7), MonitorStatus::Warning);
    }

    #[test]
    fn timeout_snmp_gera_resultado_unknown() {
        let started_at = Utc::now();
        let result = check_result_from_snmp_error(SnmpError::Timeout, started_at, started_at);
        assert_eq!(result.status, MonitorStatus::Unknown);
        assert!(!result.success);
        assert!(result
            .message
            .as_deref()
            .expect("mensagem presente")
            .contains("Tempo esgotado na consulta SNMP"));
    }

    #[test]
    fn outros_erros_snmp_geram_resultado_down() {
        let started_at = Utc::now();
        let result = check_result_from_snmp_error(
            SnmpError::Network("host inalcançável".into()),
            started_at,
            started_at,
        );
        assert_eq!(result.status, MonitorStatus::Down);
        assert!(!result.success);
    }
}
