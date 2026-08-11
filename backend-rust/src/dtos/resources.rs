//! DTOs de entrada dos recursos entregues nas Fases 2 e 3.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteInput {
    pub name: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInput {
    pub site_id: Option<i64>,
    pub probe_id: Option<i64>,
    pub name: String,
    pub cidr: String,
    pub gateway: Option<String>,
    pub vlan: Option<i32>,
    pub dns_servers: Option<serde_json::Value>,
    pub scan_enabled: Option<bool>,
    pub scan_interval: Option<i32>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInput {
    pub site_id: Option<i64>,
    pub network_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub zabbix_template_id: Option<i64>,
    pub ip_address: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub description: Option<String>,
    pub is_monitored: Option<bool>,
    pub snmp_enabled: Option<bool>,
    pub snmp_community: Option<String>,
    pub snmp_version: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInput {
    pub device_id: Option<i64>,
    pub probe_id: Option<i64>,
    #[serde(rename = "type")]
    pub monitor_type: Option<String>,
    pub name: Option<String>,
    pub configuration: Option<serde_json::Value>,
    pub target: Option<String>,
    pub port: Option<i64>,
    pub interval_seconds: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub enabled: Option<bool>,
    pub is_enabled: Option<bool>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProbeInput {
    pub site_id: Option<i64>,
    pub name: Option<String>,
    pub token_hash: Option<String>,
    pub status: Option<String>,
    pub version: Option<String>,
    pub configuration: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DnsServerInput {
    pub name: Option<String>,
    pub address: Option<String>,
    pub protocol: Option<String>,
    pub is_default: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardLayoutInput {
    pub layout: Vec<serde_json::Value>,
    pub client_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZabbixImportInput {
    pub content: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaginationQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
}
