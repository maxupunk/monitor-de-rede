//! Checker SNMP: uptime, estado de interface e tráfego por interface.

use chrono::Utc;
use serde::Deserialize;

use crate::services::{
    monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus},
    snmp::{
        client::{SnmpClient, SnmpConfig, SnmpVersion},
        collectors::OID_SYS_UPTIME,
    },
};

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

pub struct SnmpChecker;
#[async_trait::async_trait]
impl Checker for SnmpChecker {
    type Config = SnmpCheckerConfig;
    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let result = async {
            let version = SnmpVersion::parse(&config.version).ok_or("Versão SNMP inválida")?;
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
            let oid = match metric {
                "interface_status" | "status" => format!(
                    "1.3.6.1.2.1.2.2.1.8.{}",
                    config.if_index.ok_or("ifIndex é obrigatório")?
                ),
                "traffic" => format!(
                    "1.3.6.1.2.1.31.1.1.1.6.{}",
                    config.if_index.ok_or("ifIndex é obrigatório")?
                ),
                _ => OID_SYS_UPTIME.into(),
            };
            let values = client
                .get(&[&oid])
                .await
                .map_err(|error| error.to_string())?;
            Ok::<_, String>((
                metric.to_string(),
                values
                    .get(&oid)
                    .and_then(Option::as_ref)
                    .and_then(|value| value.number())
                    .unwrap_or(0),
            ))
        }
        .await;
        let finished_at = Utc::now();
        match result {
            Ok((metric, value)) => {
                let status =
                    if matches!(metric.as_str(), "interface_status" | "status") && value != 1 {
                        MonitorStatus::Down
                    } else {
                        MonitorStatus::Up
                    };
                CheckResult {
                    success: status == MonitorStatus::Up,
                    status,
                    started_at,
                    finished_at,
                    duration_ms: (finished_at - started_at).num_milliseconds().max(0),
                    message: Some(format!("Consulta SNMP {} concluída", metric)),
                    metrics: vec![CheckMetric {
                        name: if metric == "traffic" {
                            "ifHCInOctets".into()
                        } else if metric == "uptime" {
                            "snmp_uptime".into()
                        } else {
                            "if_oper_status".into()
                        },
                        value: value as f64,
                        unit: if metric == "uptime" {
                            "ticks".into()
                        } else {
                            "value".into()
                        },
                    }],
                    data: serde_json::json!({ "ifName": config.if_name, "metric": metric }),
                }
            }
            Err(error) => CheckResult {
                success: false,
                status: MonitorStatus::Down,
                started_at,
                finished_at,
                duration_ms: (finished_at - started_at).num_milliseconds().max(0),
                message: Some(format!("Falha na consulta SNMP: {error}")),
                metrics: vec![],
                data: serde_json::json!({}),
            },
        }
    }
}
