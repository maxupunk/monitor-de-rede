//! Coletores puros sobre o cliente SNMP. A interpretação de OIDs não mistura
//! transporte UDP nem persistência de banco.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use super::client::{SnmpClient, SnmpError, SnmpValue};

pub const OID_SYS_DESCR: &str = "1.3.6.1.2.1.1.1.0";
pub const OID_SYS_OBJECT_ID: &str = "1.3.6.1.2.1.1.2.0";
pub const OID_SYS_UPTIME: &str = "1.3.6.1.2.1.1.3.0";
pub const OID_SYS_CONTACT: &str = "1.3.6.1.2.1.1.4.0";
pub const OID_SYS_NAME: &str = "1.3.6.1.2.1.1.5.0";
pub const OID_SYS_LOCATION: &str = "1.3.6.1.2.1.1.6.0";

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpSystemInfo {
    pub sys_descr: Option<String>,
    pub sys_object_id: Option<String>,
    pub sys_up_time: Option<u64>,
    pub sys_contact: Option<String>,
    pub sys_name: Option<String>,
    pub sys_location: Option<String>,
}

pub async fn collect_system(client: &SnmpClient) -> Result<SnmpSystemInfo, SnmpError> {
    let values = client
        .get(&[
            OID_SYS_DESCR,
            OID_SYS_OBJECT_ID,
            OID_SYS_UPTIME,
            OID_SYS_CONTACT,
            OID_SYS_NAME,
            OID_SYS_LOCATION,
        ])
        .await?;
    Ok(SnmpSystemInfo {
        sys_descr: text(&values, OID_SYS_DESCR),
        sys_object_id: text(&values, OID_SYS_OBJECT_ID),
        sys_up_time: number(&values, OID_SYS_UPTIME),
        sys_contact: text(&values, OID_SYS_CONTACT),
        sys_name: text(&values, OID_SYS_NAME),
        sys_location: text(&values, OID_SYS_LOCATION),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpInterface {
    pub if_index: i32,
    pub if_name: String,
    pub if_descr: Option<String>,
    pub if_alias: Option<String>,
    pub if_type: Option<u64>,
    pub if_speed: Option<u64>,
    pub if_admin_status: Option<u64>,
    pub if_oper_status: Option<u64>,
    pub mac_address: Option<String>,
}

pub async fn collect_interfaces(client: &SnmpClient) -> Result<Vec<SnmpInterface>, SnmpError> {
    let entries = client.walk("1.3.6.1.2.1.2.2.1").await?;
    let extended = client.walk("1.3.6.1.2.1.31.1.1.1").await?;
    let mut values = BTreeMap::<i32, BTreeMap<u32, SnmpValue>>::new();
    for entry in entries.into_iter().chain(extended) {
        let parts: Vec<u32> = entry
            .oid
            .split('.')
            .filter_map(|part| part.parse().ok())
            .collect();
        if parts.len() >= 2 {
            values
                .entry(*parts.last().unwrap() as i32)
                .or_default()
                .insert(parts[parts.len() - 2], entry.value);
        }
    }
    Ok(values
        .into_iter()
        .map(|(index, fields)| {
            let high_speed = fields
                .get(&15)
                .and_then(SnmpValue::number)
                .filter(|value| *value > 0)
                .map(|value| value * 1_000_000);
            SnmpInterface {
                if_index: index,
                if_name: fields
                    .get(&1)
                    .map(SnmpValue::text)
                    .or_else(|| fields.get(&2).map(SnmpValue::text))
                    .unwrap_or_else(|| format!("eth{index}")),
                if_descr: fields.get(&2).map(SnmpValue::text),
                if_alias: fields.get(&18).map(SnmpValue::text),
                if_type: fields.get(&3).and_then(SnmpValue::number),
                if_speed: high_speed.or_else(|| fields.get(&5).and_then(SnmpValue::number)),
                if_admin_status: fields.get(&7).and_then(SnmpValue::number).or(Some(1)),
                if_oper_status: fields.get(&8).and_then(SnmpValue::number).or(Some(1)),
                mac_address: fields.get(&6).map(SnmpValue::text),
            }
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct InterfaceTraffic {
    pub if_index: i32,
    pub in_octets: u64,
    pub out_octets: u64,
    pub in_errors: u64,
    pub out_errors: u64,
    pub recorded_at: DateTime<Utc>,
}
#[must_use]
pub fn calculate_rates(previous: &InterfaceTraffic, current: &InterfaceTraffic) -> (f64, f64) {
    let seconds = (current.recorded_at - previous.recorded_at).num_milliseconds() as f64 / 1_000.0;
    if seconds <= 0.0 {
        return (0.0, 0.0);
    }
    (
        counter_diff(previous.in_octets, current.in_octets) as f64 * 8.0 / seconds,
        counter_diff(previous.out_octets, current.out_octets) as f64 * 8.0 / seconds,
    )
}
fn counter_diff(previous: u64, current: u64) -> u64 {
    if current >= previous {
        current - previous
    } else if previous > u32::MAX as u64 {
        current.wrapping_sub(previous)
    } else {
        current + (u32::MAX as u64 + 1) - previous
    }
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpCpuInfo {
    pub usage_percent: Option<f64>,
}
pub async fn collect_cpu(client: &SnmpClient) -> Result<SnmpCpuInfo, SnmpError> {
    let entries = client.walk("1.3.6.1.2.1.25.3.3.1.2").await?;
    let cores: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.value.number())
        .collect();
    Ok(SnmpCpuInfo {
        usage_percent: (!cores.is_empty())
            .then(|| cores.iter().sum::<u64>() as f64 / cores.len() as f64),
    })
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpMemoryInfo {
    pub total_kb: Option<u64>,
    pub avail_kb: Option<u64>,
    pub free_kb: Option<u64>,
    pub used_kb: Option<u64>,
    pub used_percent: Option<f64>,
}
pub async fn collect_memory(client: &SnmpClient) -> Result<SnmpMemoryInfo, SnmpError> {
    let values = client
        .get(&[
            "1.3.6.1.4.1.2021.4.5.0",
            "1.3.6.1.4.1.2021.4.6.0",
            "1.3.6.1.4.1.2021.4.11.0",
        ])
        .await?;
    let total = number(&values, "1.3.6.1.4.1.2021.4.5.0");
    let avail = number(&values, "1.3.6.1.4.1.2021.4.6.0");
    let free = number(&values, "1.3.6.1.4.1.2021.4.11.0");
    let used = total
        .zip(avail)
        .map(|(total, avail)| total.saturating_sub(avail));
    Ok(SnmpMemoryInfo {
        total_kb: total,
        avail_kb: avail,
        free_kb: free,
        used_kb: used,
        used_percent: used
            .zip(total)
            .map(|(used, total)| used as f64 * 100.0 / total.max(1) as f64),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LldpNeighbor {
    pub local_port: String,
    pub remote_port: Option<String>,
    pub remote_sys_name: Option<String>,
    pub remote_mgmt_address: Option<String>,
    pub protocol: String,
}
pub async fn collect_lldp(_client: &SnmpClient) -> Result<Vec<LldpNeighbor>, SnmpError> {
    Ok(vec![])
}

fn number(values: &BTreeMap<String, Option<SnmpValue>>, oid: &str) -> Option<u64> {
    values
        .get(oid)
        .and_then(Option::as_ref)
        .and_then(SnmpValue::number)
}
fn text(values: &BTreeMap<String, Option<SnmpValue>>, oid: &str) -> Option<String> {
    values
        .get(oid)
        .and_then(Option::as_ref)
        .map(SnmpValue::text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calcula_rollover_de_32_bits() {
        let previous = InterfaceTraffic {
            if_index: 1,
            in_octets: u32::MAX as u64 - 3,
            out_octets: u32::MAX as u64 - 3,
            in_errors: 0,
            out_errors: 0,
            recorded_at: Utc::now(),
        };
        let current = InterfaceTraffic {
            if_index: 1,
            in_octets: 4,
            out_octets: 4,
            in_errors: 0,
            out_errors: 0,
            recorded_at: previous.recorded_at + chrono::Duration::seconds(1),
        };
        assert_eq!(calculate_rates(&previous, &current), (64.0, 64.0));
    }
}
