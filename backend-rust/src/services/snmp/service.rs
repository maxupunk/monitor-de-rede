//! Casos de uso SNMP para controllers, scheduler e checker.

use futures::future;

use crate::services::{
    shared::errors::{AppError, AppResult},
    snmp::{
        client::{SnmpConfig, SnmpError, SnmpVersion},
        collectors::{
            collect_cpu, collect_interfaces, collect_memory, collect_system, SnmpCpuInfo,
            SnmpInterface, SnmpMemoryInfo, SnmpSystemInfo,
        },
    },
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpTestResult {
    pub success: bool,
    pub system: SnmpSystemInfo,
    pub message: String,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpScanResult {
    pub snmp_responded: bool,
    pub system: SnmpSystemInfo,
    pub interfaces: Vec<SnmpInterface>,
    pub cpu: SnmpCpuInfo,
    pub memory: SnmpMemoryInfo,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpDetectResult {
    pub detected: bool,
    pub version: Option<String>,
    pub community: Option<String>,
    pub result: Option<SnmpTestResult>,
}

pub async fn test_connection(config: SnmpConfig) -> AppResult<SnmpTestResult> {
    let system = collect_system(&super::client::SnmpClient::new(config))
        .await
        .map_err(map_error)?;
    let success =
        system.sys_descr.is_some() || system.sys_name.is_some() || system.sys_up_time.is_some();
    Ok(SnmpTestResult {
        success,
        system,
        message: if success {
            "Conexão SNMP estabelecida".into()
        } else {
            "Agente SNMP respondeu sem OIDs utilizáveis".into()
        },
    })
}

pub async fn scan(config: SnmpConfig) -> AppResult<SnmpScanResult> {
    let client = super::client::SnmpClient::new(config);
    let (system, interfaces, cpu, memory) = tokio::join!(
        collect_system(&client),
        collect_interfaces(&client),
        collect_cpu(&client),
        collect_memory(&client)
    );
    let system = system.map_err(map_error)?;
    Ok(SnmpScanResult {
        snmp_responded: system.sys_descr.is_some()
            || system.sys_name.is_some()
            || system.sys_up_time.is_some(),
        system,
        interfaces: interfaces.unwrap_or_default(),
        cpu: cpu.unwrap_or_default(),
        memory: memory.unwrap_or_default(),
    })
}

pub async fn detect_connection(
    host: &str,
    port: u16,
    preferred: Option<SnmpConfig>,
) -> AppResult<SnmpDetectResult> {
    let mut candidates = Vec::new();
    if let Some(config) = preferred {
        candidates.push(config);
    }
    for (version, community) in [
        (SnmpVersion::V2c, "public"),
        (SnmpVersion::V2c, "private"),
        (SnmpVersion::V1, "public"),
        (SnmpVersion::V1, "private"),
    ] {
        if !candidates
            .iter()
            .any(|config| config.version == version && config.community == community)
        {
            let mut config = SnmpConfig::v2c(host, community, port);
            config.version = version;
            config.timeout_ms = 2_500;
            candidates.push(config);
        }
    }
    let attempts = candidates.into_iter().map(|config| async move {
        let label = (config.version, config.community.clone());
        test_connection(config).await.map(|result| (label, result))
    });
    let outcomes = future::join_all(attempts).await;
    match outcomes.into_iter().find_map(Result::ok) {
        Some(((version, community), result)) => Ok(SnmpDetectResult {
            detected: result.success,
            version: Some(
                match version {
                    SnmpVersion::V1 => "v1",
                    SnmpVersion::V2c => "v2c",
                    SnmpVersion::V3 => "v3",
                }
                .into(),
            ),
            community: Some(community),
            result: Some(result),
        }),
        None => Ok(SnmpDetectResult {
            detected: false,
            version: None,
            community: None,
            result: None,
        }),
    }
}
fn map_error(error: SnmpError) -> AppError {
    AppError::BusinessRule(error.to_string())
}
