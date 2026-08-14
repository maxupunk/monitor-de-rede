//! Provisionamento automático do monitoramento de um dispositivo da VPN (§8.10.4).
//!
//! Os monitores são atribuídos ao `vpn-probe`, o único agente que enxerga a
//! interface `wg0` — o probe da LAN continua intocado.

use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

use crate::{
    models::{devices, monitors, probes},
    services::shared::errors::AppResult,
};

/// Nome do probe dedicado que compartilha o namespace de rede do WireGuard.
#[must_use]
pub fn vpn_probe_name() -> String {
    std::env::var("VPN_PROBE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "vpn-probe".to_string())
}

#[derive(Debug, Clone, Default)]
pub struct MonitorProvisioningOptions {
    pub snmp_enabled: bool,
    pub snmp_community: Option<String>,
    pub snmp_version: Option<String>,
    pub interval_seconds: Option<i32>,
}

/// Id do `vpn-probe`; `None` quando ainda não registrado — nesse caso o monitor
/// roda local, e o `peer_hints` sinaliza `pingOutsideTunnel`.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn resolve_probe_id<C: ConnectionTrait>(db: &C) -> AppResult<Option<i64>> {
    Ok(probes::Entity::find()
        .filter(probes::Column::Name.eq(vpn_probe_name()))
        .filter(probes::Column::Status.ne(probes::STATUS_REVOKED))
        .one(db)
        .await?
        .map(|probe| probe.id))
}

/// Cria o monitor de ping e, opcionalmente, o de SNMP.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn provision<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
    options: &MonitorProvisioningOptions,
) -> AppResult<Vec<monitors::Model>> {
    let probe_id = resolve_probe_id(db).await?;
    let host = device
        .ip_address
        .clone()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| device.name.clone());
    let interval = options.interval_seconds.unwrap_or(60);
    let mut created = Vec::new();

    created.push(
        monitors::ActiveModel {
            device_id: Set(Some(device.id)),
            probe_id: Set(probe_id),
            r#type: Set("ping".into()),
            name: Set(format!("Ping {}", device.name)),
            configuration: Set(serde_json::json!({ "host": host })),
            interval_seconds: Set(interval),
            timeout_seconds: Set(5),
            retry_count: Set(3),
            enabled: Set(true),
            status: Set("unknown".into()),
            ..Default::default()
        }
        .insert(db)
        .await?,
    );

    if options.snmp_enabled {
        created.push(
            monitors::ActiveModel {
                device_id: Set(Some(device.id)),
                probe_id: Set(probe_id),
                r#type: Set("snmp".into()),
                name: Set(format!("SNMP {}", device.name)),
                configuration: Set(serde_json::json!({
                    "host": host,
                    "version": options.snmp_version.as_deref().unwrap_or("v2c"),
                    "community": options.snmp_community.as_deref().unwrap_or("public"),
                    "port": 161,
                })),
                interval_seconds: Set(device.snmp_poll_interval_seconds),
                timeout_seconds: Set(5),
                retry_count: Set(3),
                enabled: Set(true),
                status: Set("unknown".into()),
                ..Default::default()
            }
            .insert(db)
            .await?,
        );
    }

    Ok(created)
}

/// Prefixo do nome gerado para um monitor provisionado aqui.
///
/// Usado no `rename` do peer: só acompanha o novo nome o monitor que **ainda**
/// se chama como foi criado — um monitor renomeado à mão fica como está.
#[must_use]
pub fn generated_name_prefix(monitor_type: &str) -> Option<&'static str> {
    match monitor_type {
        "ping" => Some("Ping"),
        "snmp" => Some("SNMP"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn o_nome_do_probe_vem_do_ambiente_com_padrao() {
        std::env::remove_var("VPN_PROBE_NAME");
        assert_eq!(vpn_probe_name(), "vpn-probe");
        std::env::set_var("VPN_PROBE_NAME", "probe-tunel");
        assert_eq!(vpn_probe_name(), "probe-tunel");
        // Valor vazio no compose não pode virar nome de probe.
        std::env::set_var("VPN_PROBE_NAME", "  ");
        assert_eq!(vpn_probe_name(), "vpn-probe");
        std::env::remove_var("VPN_PROBE_NAME");
    }

    #[test]
    fn so_ping_e_snmp_tem_nome_gerado() {
        assert_eq!(generated_name_prefix("ping"), Some("Ping"));
        assert_eq!(generated_name_prefix("snmp"), Some("SNMP"));
        assert_eq!(generated_name_prefix("http"), None);
    }
}
