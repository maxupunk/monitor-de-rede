//! Resolve o que a IA escreve ("roteador borda", "10.0.0.1", "12", "ether1")
//! para a linha do banco.
//!
//! A comparação sem caixa roda em memória, sobre a lista do dispositivo: um
//! `LOWER(...) LIKE` teria semântica diferente entre SQLite e PostgreSQL, e as
//! listas envolvidas (dispositivos, interfaces de um aparelho) são pequenas.

use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::{
    models::{_entities::device_interfaces, devices, monitors},
    services::shared::errors::AppResult,
};

/// Normaliza para comparação sem caixa.
fn fold(text: &str) -> String {
    text.trim().to_lowercase()
}

/// O termo casa com o dispositivo pelo nome, IP ou fabricante (substring, sem caixa).
#[must_use]
pub fn device_matches(device: &devices::Model, term: &str) -> bool {
    let term = fold(term);
    if term.is_empty() {
        return true;
    }
    [
        Some(device.name.as_str()),
        device.ip_address.as_deref(),
        device.vendor.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|field| fold(field).contains(&term))
}

/// Escolhe o dispositivo: id exato, depois nome/IP exatos, depois substring
/// única. Ambiguidade devolve `None` — melhor a IA perguntar do que olhar o
/// aparelho errado.
#[must_use]
pub fn pick_device<'a>(list: &'a [devices::Model], identifier: &str) -> Option<&'a devices::Model> {
    let wanted = fold(identifier);
    if wanted.is_empty() {
        return None;
    }
    if let Ok(id) = wanted.parse::<i64>() {
        if let Some(device) = list.iter().find(|device| device.id == id) {
            return Some(device);
        }
    }
    let exact = list.iter().find(|device| {
        fold(&device.name) == wanted
            || device
                .ip_address
                .as_deref()
                .is_some_and(|ip| fold(ip) == wanted)
    });
    if exact.is_some() {
        return exact;
    }
    let mut partial = list.iter().filter(|device| device_matches(device, &wanted));
    match (partial.next(), partial.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// Nomes de dispositivo por id, para trocar ids por nomes legíveis.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn device_names<C: ConnectionTrait>(
    db: &C,
    ids: impl IntoIterator<Item = i64>,
) -> AppResult<HashMap<i64, String>> {
    let ids: HashSet<i64> = ids.into_iter().collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(devices::Entity::find()
        .filter(devices::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|device| (device.id, device.name))
        .collect())
}

/// # Errors
///
/// Propaga erro do banco.
pub async fn find_device<C: ConnectionTrait>(
    db: &C,
    identifier: &str,
) -> AppResult<Option<devices::Model>> {
    let list = devices::Entity::find().all(db).await?;
    Ok(pick_device(&list, identifier).cloned())
}

/// Escolhe a interface por id, nome, alias, descrição ou `ifIndex`.
#[must_use]
pub fn pick_interface<'a>(
    list: &'a [device_interfaces::Model],
    identifier: &str,
) -> Option<&'a device_interfaces::Model> {
    let wanted = fold(identifier);
    if wanted.is_empty() {
        return None;
    }
    if let Ok(number) = wanted.parse::<i64>() {
        let by_number = list.iter().find(|iface| iface.id == number).or_else(|| {
            list.iter()
                .find(|iface| iface.snmp_index.map(i64::from) == Some(number))
        });
        if by_number.is_some() {
            return by_number;
        }
    }
    let names = |iface: &'a device_interfaces::Model| {
        [
            Some(iface.name.as_str()),
            iface.alias.as_deref(),
            iface.description.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(fold)
    };
    list.iter()
        .find(|iface| names(iface).any(|name| name == wanted))
        .or_else(|| {
            let mut partial = list
                .iter()
                .filter(|iface| names(iface).any(|name| name.contains(&wanted)));
            match (partial.next(), partial.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            }
        })
}

/// # Errors
///
/// Propaga erro do banco.
pub async fn device_interfaces_of<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
) -> AppResult<Vec<device_interfaces::Model>> {
    Ok(device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(device_id))
        .all(db)
        .await?)
}

/// Monitor por id ou, dado um dispositivo, o primeiro do tipo pedido
/// (`ping` quando o tipo não vem — é o que mede latência).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn find_monitor<C: ConnectionTrait>(
    db: &C,
    monitor_id: Option<i64>,
    device_identifier: Option<&str>,
    monitor_type: Option<&str>,
) -> AppResult<Result<monitors::Model, String>> {
    if let Some(id) = monitor_id {
        return Ok(monitors::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| format!("Monitor {id} não encontrado")));
    }
    let Some(identifier) = device_identifier else {
        return Ok(Err(
            "Informe monitor_id ou device (nome/IP do dispositivo)".to_string()
        ));
    };
    let Some(device) = find_device(db, identifier).await? else {
        return Ok(Err(format!(
            "Dispositivo '{identifier}' não encontrado ou ambíguo; use list_devices"
        )));
    };
    let kind = monitor_type.unwrap_or("ping");
    let found = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .filter(monitors::Column::Type.eq(kind))
        .one(db)
        .await?;
    Ok(found.ok_or_else(|| {
        format!(
            "'{}' não tem monitor do tipo '{kind}'; use get_device_detail para ver os monitores",
            device.name
        )
    }))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn device(id: i64, name: &str, ip: &str) -> devices::Model {
        let now = Utc::now().into();
        devices::Model {
            id,
            site_id: None,
            network_id: None,
            parent_id: None,
            ip_address: Some(ip.into()),
            name: name.into(),
            r#type: "router".into(),
            vendor: Some("MikroTik".into()),
            model: None,
            serial_number: None,
            description: None,
            is_monitored: true,
            snmp_enabled: false,
            snmp_community: None,
            snmp_version: None,
            snmp_poll_interval_seconds: 60,
            access_mode: None,
            operating_system: None,
            syslog_server_address: None,
            system_key: None,
            link_interface_id: None,
            link_interface_name: None,
            status: "up".into(),
            last_seen_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn iface(id: i64, index: i32, name: &str, alias: Option<&str>) -> device_interfaces::Model {
        let now = Utc::now().into();
        device_interfaces::Model {
            id,
            device_id: 1,
            snmp_index: Some(index),
            name: name.into(),
            description: None,
            alias: alias.map(Into::into),
            mac_address: None,
            r#type: None,
            speed: None,
            admin_status: Some("up".into()),
            oper_status: Some("up".into()),
            last_seen_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn dispositivo_por_id_nome_ip_e_substring_unica() {
        let list = vec![
            device(1, "Borda Principal", "10.0.0.1"),
            device(2, "Switch Andar 2", "10.0.0.2"),
            device(3, "Switch Andar 3", "10.0.0.3"),
        ];
        assert_eq!(pick_device(&list, "2").map(|d| d.id), Some(2));
        assert_eq!(pick_device(&list, "borda principal").map(|d| d.id), Some(1));
        assert_eq!(pick_device(&list, "10.0.0.3").map(|d| d.id), Some(3));
        assert_eq!(pick_device(&list, "borda").map(|d| d.id), Some(1));
        assert_eq!(pick_device(&list, "switch"), None, "ambíguo");
        assert_eq!(pick_device(&list, "   "), None);
    }

    #[test]
    fn busca_de_dispositivo_ignora_caixa_e_olha_o_fabricante() {
        let borda = device(1, "Borda", "10.0.0.1");
        assert!(device_matches(&borda, "BOR"));
        assert!(device_matches(&borda, "mikrotik"));
        assert!(device_matches(&borda, ""));
        assert!(!device_matches(&borda, "cisco"));
    }

    #[test]
    fn interface_por_nome_alias_e_indice() {
        let list = vec![
            iface(10, 1, "ether1", Some("WAN Vivo")),
            iface(11, 2, "ether2", None),
            iface(12, 7, "sfp-sfpplus1", None),
        ];
        assert_eq!(pick_interface(&list, "ETHER2").map(|i| i.id), Some(11));
        assert_eq!(pick_interface(&list, "wan vivo").map(|i| i.id), Some(10));
        assert_eq!(pick_interface(&list, "7").map(|i| i.id), Some(12));
        assert_eq!(pick_interface(&list, "12").map(|i| i.id), Some(12));
        assert_eq!(pick_interface(&list, "sfp").map(|i| i.id), Some(12));
        assert_eq!(pick_interface(&list, "ether"), None, "ambíguo");
    }
}
