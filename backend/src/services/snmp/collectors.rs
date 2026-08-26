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
pub const OID_ENT_PHYSICAL_MFG_NAME: &str = "1.3.6.1.2.1.47.1.1.1.1.12.1";
pub const OID_ENT_PHYSICAL_MODEL_NAME: &str = "1.3.6.1.2.1.47.1.1.1.1.13.1";

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpSystemInfo {
    pub sys_descr: Option<String>,
    pub sys_object_id: Option<String>,
    pub sys_up_time: Option<u64>,
    pub sys_contact: Option<String>,
    pub sys_name: Option<String>,
    pub sys_location: Option<String>,
    /// Fabricante e modelo do chassi, quando o agente implementa ENTITY-MIB.
    pub hardware_vendor: Option<String>,
    pub hardware_model: Option<String>,
}

impl SnmpSystemInfo {
    /// Houve contato real com o agente? (matriz de paridade #19)
    ///
    /// Só `sysDescr`, `sysName` e `sysUpTime` contam. `sysContact` e
    /// `sysLocation` ficam de fora de propósito: são campos de texto livre que
    /// muitos agentes devolvem vazios, e aceitá-los como prova marcaria o
    /// equipamento "online" sem que nenhum OID de identidade tivesse respondido.
    #[must_use]
    pub const fn responded(&self) -> bool {
        self.sys_descr.is_some() || self.sys_name.is_some() || self.sys_up_time.is_some()
    }
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
        hardware_vendor: None,
        hardware_model: None,
    })
}

/// Tenta enriquecer a identidade com o chassi sem comprometer o `system`.
///
/// ENTITY-MIB é opcional. A consulta fica separada porque agentes SNMPv1 podem
/// rejeitar o pacote inteiro quando recebem um OID que não conhecem; nesse caso
/// ainda preservamos `sysDescr`, `sysObjectID` e o restante da identificação.
pub async fn collect_hardware(
    client: &SnmpClient,
) -> Result<(Option<String>, Option<String>), SnmpError> {
    let values = client
        .get(&[OID_ENT_PHYSICAL_MFG_NAME, OID_ENT_PHYSICAL_MODEL_NAME])
        .await?;
    Ok((
        text(&values, OID_ENT_PHYSICAL_MFG_NAME),
        text(&values, OID_ENT_PHYSICAL_MODEL_NAME),
    ))
}

/// `ifTable` — RFC 1213.
pub const OID_IF_TABLE: &str = "1.3.6.1.2.1.2.2.1";
/// `ifXTable` — RFC 2863, contadores de 64 bits e nomes longos.
pub const OID_IF_X_TABLE: &str = "1.3.6.1.2.1.31.1.1.1";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpInterface {
    pub if_index: i32,
    pub if_name: String,
    pub if_descr: Option<String>,
    pub if_alias: Option<String>,
    pub if_type: Option<u64>,
    pub if_speed: Option<u64>,
    // A UI lê "up"/"down", não o inteiro da MIB; a conversão fica na
    // serialização para o campo continuar comparável dentro do backend.
    #[serde(serialize_with = "serialize_status")]
    pub if_admin_status: Option<u64>,
    #[serde(serialize_with = "serialize_status")]
    pub if_oper_status: Option<u64>,
    pub mac_address: Option<String>,
    /// Já existe monitor habilitado para esta interface? Preenchido por quem
    /// tem banco à mão (`service::scan_device`) — a coleta pura não sabe.
    pub is_monitored: bool,
}

pub async fn collect_interfaces(client: &SnmpClient) -> Result<Vec<SnmpInterface>, SnmpError> {
    let (base, extended) = tokio::join!(client.walk(OID_IF_TABLE), client.walk(OID_IF_X_TABLE));
    Ok(parse_interfaces(base?, extended.unwrap_or_default()))
}

/// As interfaces e seus contadores pertencem às mesmas tabelas SNMP. Uma coleta
/// consolidada lê cada tabela só uma vez e então deriva as duas visões locais.
pub async fn collect_interfaces_and_traffic(
    client: &SnmpClient,
) -> Result<(Vec<SnmpInterface>, Vec<InterfaceTraffic>), SnmpError> {
    let (base, extended) = tokio::join!(client.walk(OID_IF_TABLE), client.walk(OID_IF_X_TABLE));
    let base = base?;
    let extended = extended.unwrap_or_default();
    let interfaces = parse_interfaces(base.clone(), extended.clone());
    let traffic = parse_traffic(base, extended);
    Ok((interfaces, traffic))
}

/// Monta as interfaces a partir das linhas cruas das duas tabelas, sem
/// transporte no meio.
///
/// As tabelas entram separadas de propósito: elas numeram colunas do zero cada
/// uma, então juntá-las num mapa só fazia `ifHCInUcastPkts` (col. 7 da
/// `ifXTable`) sobrescrever `ifAdminStatus` (col. 7 da `ifTable`) e
/// `ifHCInOctets` (col. 6) sobrescrever o MAC — era daí que saíam os status e
/// endereços com valores de contador.
///
/// Separado do `collect_interfaces` também porque é aqui que vive a regra da
/// matriz de paridade #17 — `ifHighSpeed` prevalece sobre `ifSpeed` — e provar
/// isso pede linhas montadas à mão, não um agente SNMP de verdade.
#[must_use]
pub fn parse_interfaces(
    base: impl IntoIterator<Item = super::client::SnmpWalkEntry>,
    extended: impl IntoIterator<Item = super::client::SnmpWalkEntry>,
) -> Vec<SnmpInterface> {
    let base = table_rows(base);
    let extended = table_rows(extended);
    base.keys()
        .chain(extended.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|index| {
            let row = base.get(&index);
            let extra = extended.get(&index);
            let column = |row: Option<&BTreeMap<u32, SnmpValue>>, column: u32| {
                row.and_then(|fields| fields.get(&column)).cloned()
            };
            // `ifHighSpeed` (col. 15 da ifXTable) vem em Mbps e é a única leitura
            // confiável acima de ~4,29 Gbps, onde o `ifSpeed` de 32 bits satura.
            // O filtro `> 0` importa: agente que não implementa a coluna devolve
            // 0, e aceitá-lo apagaria a velocidade real vinda do `ifSpeed`.
            let high_speed = column(extra, 15)
                .and_then(|value| value.number())
                .filter(|value| *value > 0)
                .map(|value| value * 1_000_000);
            SnmpInterface {
                if_index: index,
                if_name: column(extra, 1)
                    .or_else(|| column(row, 2))
                    .map(|value| value.text())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| format!("eth{index}")),
                if_descr: non_empty_text(column(row, 2)),
                if_alias: non_empty_text(column(extra, 18)),
                if_type: column(row, 3).and_then(|value| value.number()),
                if_speed: high_speed.or_else(|| column(row, 5).and_then(|value| value.number())),
                if_admin_status: column(row, 7).and_then(|value| value.number()).or(Some(1)),
                if_oper_status: column(row, 8).and_then(|value| value.number()).or(Some(1)),
                mac_address: column(row, 6).and_then(|value| value.mac()),
                is_monitored: false,
            }
        })
        .collect()
}

/// Indexa as linhas de uma tabela SNMP por `índice` e depois por `coluna`.
fn table_rows(
    entries: impl IntoIterator<Item = super::client::SnmpWalkEntry>,
) -> BTreeMap<i32, BTreeMap<u32, SnmpValue>> {
    let mut rows = BTreeMap::<i32, BTreeMap<u32, SnmpValue>>::new();
    for entry in entries {
        if let Some((column, index)) = oid_column_and_index(&entry.oid) {
            rows.entry(index).or_default().insert(column, entry.value);
        }
    }
    rows
}

fn non_empty_text(value: Option<SnmpValue>) -> Option<String> {
    value
        .map(|value| value.text())
        .filter(|text| !text.is_empty())
}

/// Rótulo textual de `ifAdminStatus`/`ifOperStatus` (RFC 2863).
#[must_use]
pub const fn status_label(value: u64) -> &'static str {
    match value {
        1 => "up",
        2 => "down",
        3 => "testing",
        5 => "dormant",
        6 => "notPresent",
        7 => "lowerLayerDown",
        _ => "unknown",
    }
}

fn serialize_status<S: serde::Serializer>(
    value: &Option<u64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_str(status_label(*value)),
        None => serializer.serialize_none(),
    }
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
    let (base, high_capacity) =
        tokio::join!(client.walk(OID_IF_TABLE), client.walk(OID_IF_X_TABLE));
    Ok(parse_traffic(base?, high_capacity?))
}

/// Extrai contadores de tráfego das linhas já lidas de `ifTable` e `ifXTable`.
/// Separar o parser do transporte permite reaproveitar as mesmas respostas na
/// coleta consolidada do dispositivo.
#[must_use]
pub fn parse_traffic(
    base: impl IntoIterator<Item = super::client::SnmpWalkEntry>,
    high_capacity: impl IntoIterator<Item = super::client::SnmpWalkEntry>,
) -> Vec<InterfaceTraffic> {
    let mut counters = BTreeMap::<i32, BTreeMap<u32, u64>>::new();
    for entry in base {
        if let (Some((column, index)), Some(value)) =
            (oid_column_and_index(&entry.oid), entry.value.number())
        {
            counters.entry(index).or_default().insert(column, value);
        }
    }
    let mut high = BTreeMap::<i32, BTreeMap<u32, u64>>::new();
    for entry in high_capacity {
        if let (Some((column, index)), Some(value)) =
            (oid_column_and_index(&entry.oid), entry.value.number())
        {
            high.entry(index).or_default().insert(column, value);
        }
    }
    let recorded_at = Utc::now();
    counters
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
        .collect()
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

    fn linha(coluna: u32, indice: i32, value: SnmpValue) -> super::super::client::SnmpWalkEntry {
        super::super::client::SnmpWalkEntry {
            oid: format!("{OID_IF_TABLE}.{coluna}.{indice}"),
            value,
        }
    }
    fn linha_x(coluna: u32, indice: i32, value: SnmpValue) -> super::super::client::SnmpWalkEntry {
        super::super::client::SnmpWalkEntry {
            oid: format!("{OID_IF_X_TABLE}.{coluna}.{indice}"),
            value,
        }
    }

    #[test]
    fn mesmos_walks_geram_interfaces_e_contadores_de_trafego() {
        let base = [
            linha(2, 4, SnmpValue::Bytes(b"uplink".to_vec())),
            linha(10, 4, SnmpValue::Number(12)),
            linha(16, 4, SnmpValue::Number(34)),
        ];
        let extended = [
            linha_x(1, 4, SnmpValue::Bytes(b"uplink".to_vec())),
            linha_x(6, 4, SnmpValue::Number(56)),
            linha_x(10, 4, SnmpValue::Number(78)),
        ];
        let interfaces = parse_interfaces(base.clone(), extended.clone());
        let traffic = parse_traffic(base, extended);

        assert_eq!(interfaces[0].if_name, "uplink");
        assert_eq!(traffic.len(), 1);
        assert_eq!(traffic[0].if_index, 4);
        assert_eq!(traffic[0].in_octets, 56);
        assert_eq!(traffic[0].out_octets, 78);
        assert_eq!(traffic[0].counter_bits, 64);
    }

    /// `ifSpeed` saturado (col. 5 = 4294967295) com `ifHighSpeed` (col. 15) real.
    #[test]
    fn if_high_speed_prevalece_sobre_if_speed_saturado() {
        let interfaces = parse_interfaces(
            [
                linha(2, 1, SnmpValue::Bytes(b"Gi0/1".to_vec())),
                linha(5, 1, SnmpValue::Number(4_294_967_295)),
            ],
            // 10 Gbps expressos em Mbps, como manda a MIB.
            [linha_x(15, 1, SnmpValue::Number(10_000))],
        );
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].if_speed, Some(10_000_000_000));
    }

    #[test]
    fn sem_if_high_speed_a_leitura_cai_para_if_speed() {
        let interfaces = parse_interfaces(
            [
                linha(2, 1, SnmpValue::Bytes(b"Fa0/1".to_vec())),
                linha(5, 1, SnmpValue::Number(100_000_000)),
            ],
            [],
        );
        assert_eq!(interfaces[0].if_speed, Some(100_000_000));
    }

    #[test]
    fn if_high_speed_zerado_nao_apaga_a_velocidade_real() {
        // Agente que não implementa a coluna devolve 0: aceitá-lo faria uma
        // interface de 1 Gbps aparecer como velocidade desconhecida.
        let interfaces = parse_interfaces(
            [
                linha(2, 1, SnmpValue::Bytes(b"Gi0/2".to_vec())),
                linha(5, 1, SnmpValue::Number(1_000_000_000)),
            ],
            [linha_x(15, 1, SnmpValue::Number(0))],
        );
        assert_eq!(interfaces[0].if_speed, Some(1_000_000_000));
    }

    #[test]
    fn interface_sem_nome_recebe_rotulo_derivado_do_indice() {
        let interfaces = parse_interfaces([linha(5, 7, SnmpValue::Number(1_000))], []);
        assert_eq!(interfaces[0].if_name, "eth7");
        // Sem as colunas de status, o padrão é "1" (up) nos dois — é o que o
        // backend anterior assumia para não inventar queda de link.
        assert_eq!(interfaces[0].if_admin_status, Some(1));
        assert_eq!(interfaces[0].if_oper_status, Some(1));
    }

    /// As duas tabelas numeram colunas do zero cada uma: juntá-las num mapa só
    /// fazia os contadores de 64 bits da `ifXTable` sobrescreverem status e MAC.
    #[test]
    fn contadores_da_if_x_table_nao_sobrescrevem_status_nem_mac() {
        let interfaces = parse_interfaces(
            [
                linha(2, 2, SnmpValue::Bytes(b"eth0".to_vec())),
                linha(3, 2, SnmpValue::Number(6)),
                linha(
                    6,
                    2,
                    SnmpValue::Bytes(vec![0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]),
                ),
                linha(7, 2, SnmpValue::Number(1)),
                linha(8, 2, SnmpValue::Number(2)),
            ],
            [
                linha_x(1, 2, SnmpValue::Bytes(b"br-lan".to_vec())),
                // ifHCInOctets (col. 6) e ifHCInUcastPkts (col. 7).
                linha_x(6, 2, SnmpValue::Number(8_382_243_847_261)),
                linha_x(7, 2, SnmpValue::Number(1_874_150)),
            ],
        );

        assert_eq!(interfaces[0].if_name, "br-lan");
        assert_eq!(interfaces[0].if_descr.as_deref(), Some("eth0"));
        assert_eq!(interfaces[0].if_type, Some(6));
        assert_eq!(
            interfaces[0].mac_address.as_deref(),
            Some("00:1a:2b:3c:4d:5e")
        );
        assert_eq!(interfaces[0].if_admin_status, Some(1));
        assert_eq!(interfaces[0].if_oper_status, Some(2));
    }

    #[test]
    fn status_da_interface_sai_serializado_como_rotulo() {
        let interfaces = parse_interfaces([linha(8, 1, SnmpValue::Number(2))], []);
        let json = serde_json::to_value(&interfaces[0]).unwrap();

        assert_eq!(json["ifOperStatus"], "down");
        assert_eq!(json["ifAdminStatus"], "up");
    }

    #[test]
    fn so_oid_de_identidade_prova_contato_com_o_agente() {
        // Matriz de paridade #19: sem isso o poll marcaria "online" no escuro e
        // o ping seguinte derrubaria o dispositivo, publicando transição a cada
        // ciclo.
        let mudo = SnmpSystemInfo::default();
        assert!(!mudo.responded());

        for info in [
            SnmpSystemInfo {
                sys_descr: Some("Cisco IOS".into()),
                ..Default::default()
            },
            SnmpSystemInfo {
                sys_name: Some("sw-core".into()),
                ..Default::default()
            },
            SnmpSystemInfo {
                sys_up_time: Some(0),
                ..Default::default()
            },
        ] {
            assert!(info.responded(), "OID de identidade ignorado: {info:?}");
        }

        // Texto livre nao conta como prova de vida.
        let so_texto_livre = SnmpSystemInfo {
            sys_contact: Some("noc@exemplo".into()),
            sys_location: Some("Rack 3".into()),
            ..Default::default()
        };
        assert!(!so_texto_livre.responded());
    }
}
