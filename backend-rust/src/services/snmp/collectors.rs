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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceTraffic {
    pub if_index: i32,
    pub in_octets: u64,
    pub out_octets: u64,
    pub in_errors: u64,
    pub out_errors: u64,
    pub counter_bits: u8,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrafficRates {
    pub in_bps: f64,
    pub out_bps: f64,
    pub reboot_detected: bool,
}

pub async fn collect_traffic(client: &SnmpClient) -> Result<Vec<InterfaceTraffic>, SnmpError> {
    let (base, high_capacity) = tokio::join!(
        client.walk("1.3.6.1.2.1.2.2.1"),
        client.walk("1.3.6.1.2.1.31.1.1.1")
    );
    let mut counters = BTreeMap::<i32, BTreeMap<u32, u64>>::new();
    for entry in base? {
        if let (Some((column, index)), Some(value)) =
            (oid_column_and_index(&entry.oid), entry.value.number())
        {
            counters.entry(index).or_default().insert(column, value);
        }
    }
    let mut high = BTreeMap::<i32, BTreeMap<u32, u64>>::new();
    for entry in high_capacity? {
        if let (Some((column, index)), Some(value)) =
            (oid_column_and_index(&entry.oid), entry.value.number())
        {
            high.entry(index).or_default().insert(column, value);
        }
    }
    let recorded_at = Utc::now();
    Ok(counters
        .into_iter()
        .filter_map(|(if_index, fields)| {
            let hc = high.get(&if_index);
            let in_octets = hc
                .and_then(|values| values.get(&6))
                .copied()
                .or_else(|| fields.get(&10).copied())?;
            let out_octets = hc
                .and_then(|values| values.get(&10))
                .copied()
                .or_else(|| fields.get(&16).copied())?;
            Some(InterfaceTraffic {
                if_index,
                in_octets,
                out_octets,
                in_errors: fields.get(&14).copied().unwrap_or_default(),
                out_errors: fields.get(&20).copied().unwrap_or_default(),
                counter_bits: if hc
                    .map(|values| values.contains_key(&6) || values.contains_key(&10))
                    .unwrap_or(false)
                {
                    64
                } else {
                    32
                },
                recorded_at,
            })
        })
        .collect())
}
#[must_use]
pub fn calculate_rates(previous: &InterfaceTraffic, current: &InterfaceTraffic) -> (f64, f64) {
    let rates = calculate_rates_detailed(previous, current, None, None);
    (rates.in_bps, rates.out_bps)
}
#[must_use]
pub fn calculate_rates_detailed(
    previous: &InterfaceTraffic,
    current: &InterfaceTraffic,
    previous_uptime: Option<u64>,
    current_uptime: Option<u64>,
) -> TrafficRates {
    let seconds = (current.recorded_at - previous.recorded_at).num_milliseconds() as f64 / 1_000.0;
    let reboot_detected = previous_uptime
        .zip(current_uptime)
        .map(|(previous, current)| current < previous)
        .unwrap_or(false);
    if seconds <= 0.0 || reboot_detected {
        return TrafficRates {
            in_bps: 0.0,
            out_bps: 0.0,
            reboot_detected,
        };
    }
    let counter_bits = previous.counter_bits.max(current.counter_bits);
    TrafficRates {
        in_bps: counter_diff(previous.in_octets, current.in_octets, counter_bits) as f64 * 8.0
            / seconds,
        out_bps: counter_diff(previous.out_octets, current.out_octets, counter_bits) as f64 * 8.0
            / seconds,
        reboot_detected,
    }
}
fn counter_diff(previous: u64, current: u64, counter_bits: u8) -> u64 {
    if current >= previous {
        current - previous
    } else if counter_bits >= 64 {
        current.wrapping_sub(previous)
    } else {
        current + (u32::MAX as u64 + 1) - previous
    }
}

fn oid_column_and_index(oid: &str) -> Option<(u32, i32)> {
    let mut parts = oid.rsplit('.');
    let index = parts.next()?.parse().ok()?;
    let column = parts.next()?.parse().ok()?;
    Some((column, index))
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
pub async fn collect_lldp(client: &SnmpClient) -> Result<Vec<LldpNeighbor>, SnmpError> {
    const LLDP_REMOTE: &str = "1.0.8802.1.1.2.1.4.1.1";
    const CDP_CACHE: &str = "1.3.6.1.4.1.9.9.23.1.2.1.1";
    let (lldp, cdp) = tokio::join!(client.walk(LLDP_REMOTE), client.walk(CDP_CACHE));
    let mut neighbors = Vec::new();

    let mut lldp_fields = BTreeMap::<(i32, i32), BTreeMap<u32, String>>::new();
    for entry in lldp.unwrap_or_default() {
        if let Some((column, index)) = table_column_and_index(&entry.oid, LLDP_REMOTE) {
            if index.len() >= 3 {
                lldp_fields
                    .entry((index[1] as i32, index[2] as i32))
                    .or_default()
                    .insert(column, entry.value.text());
            }
        }
    }
    neighbors.extend(lldp_fields.into_iter().filter_map(
        |((local_port, _remote_index), fields)| {
            let remote_sys_name = fields.get(&9).cloned();
            let remote_port = fields.get(&7).cloned();
            (remote_sys_name.is_some() || remote_port.is_some()).then(|| LldpNeighbor {
                local_port: local_port.to_string(),
                remote_port,
                remote_sys_name,
                remote_mgmt_address: fields.get(&4).cloned(),
                protocol: "lldp".into(),
            })
        },
    ));

    let mut cdp_fields = BTreeMap::<(i32, i32), BTreeMap<u32, String>>::new();
    for entry in cdp.unwrap_or_default() {
        if let Some((column, index)) = table_column_and_index(&entry.oid, CDP_CACHE) {
            if index.len() >= 2 {
                cdp_fields
                    .entry((index[0] as i32, index[1] as i32))
                    .or_default()
                    .insert(column, entry.value.text());
            }
        }
    }
    neighbors.extend(
        cdp_fields
            .into_iter()
            .filter_map(|((local_port, _remote_index), fields)| {
                let remote_sys_name = fields.get(&6).cloned();
                let remote_port = fields.get(&7).cloned();
                (remote_sys_name.is_some() || remote_port.is_some()).then(|| LldpNeighbor {
                    local_port: local_port.to_string(),
                    remote_port,
                    remote_sys_name,
                    remote_mgmt_address: fields.get(&3).cloned(),
                    protocol: "cdp".into(),
                })
            }),
    );
    Ok(neighbors)
}

fn table_column_and_index(oid: &str, base: &str) -> Option<(u32, Vec<u32>)> {
    let oid = oid
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()
        .ok()?;
    let base = base
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()
        .ok()?;
    oid.starts_with(&base).then_some(()).and_then(|_| {
        oid.get(base.len())
            .copied()
            .map(|column| (column, oid[base.len() + 1..].to_vec()))
    })
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
            counter_bits: 32,
            recorded_at: Utc::now(),
        };
        let current = InterfaceTraffic {
            if_index: 1,
            in_octets: 4,
            out_octets: 4,
            in_errors: 0,
            out_errors: 0,
            counter_bits: 32,
            recorded_at: previous.recorded_at + chrono::Duration::seconds(1),
        };
        assert_eq!(calculate_rates(&previous, &current), (64.0, 64.0));
    }

    #[test]
    fn calcula_rollover_de_64_bits() {
        let previous = InterfaceTraffic {
            if_index: 1,
            in_octets: u64::MAX - 3,
            out_octets: u64::MAX - 3,
            in_errors: 0,
            out_errors: 0,
            counter_bits: 64,
            recorded_at: Utc::now(),
        };
        let current = InterfaceTraffic {
            in_octets: 4,
            out_octets: 4,
            recorded_at: previous.recorded_at + chrono::Duration::seconds(1),
            ..previous.clone()
        };

        assert_eq!(calculate_rates(&previous, &current), (64.0, 64.0));
    }

    #[test]
    fn nao_interpreta_reboot_como_rollover() {
        let previous = InterfaceTraffic {
            if_index: 1,
            in_octets: 1_000,
            out_octets: 1_000,
            in_errors: 0,
            out_errors: 0,
            counter_bits: 64,
            recorded_at: Utc::now(),
        };
        let current = InterfaceTraffic {
            in_octets: 10,
            out_octets: 10,
            recorded_at: previous.recorded_at + chrono::Duration::seconds(1),
            ..previous.clone()
        };

        assert_eq!(
            calculate_rates_detailed(&previous, &current, Some(10_000), Some(20)),
            TrafficRates {
                in_bps: 0.0,
                out_bps: 0.0,
                reboot_detected: true,
            }
        );
    }

    #[test]
    fn identifica_coluna_e_indices_de_tabela_snmp() {
        assert_eq!(
            table_column_and_index("1.0.8802.1.1.2.1.4.1.1.9.10.4.2", "1.0.8802.1.1.2.1.4.1.1"),
            Some((9, vec![10, 4, 2]))
        );
    }
}
