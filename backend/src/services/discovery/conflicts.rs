//! Auditoria de Camada 2: Detecção de IPs clonados, MACs duplicados e divergências de rede.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::{
    models::_entities::{alert_events, device_interfaces, devices},
    services::{
        discovery::oui_lookup::lookup_vendor,
        events::EventBus,
        monitoring::ip_reconciliation::can_inspect_l2,
        network_tools::neighbor_cache::{self, NeighborEntry},
        shared::errors::AppResult,
    },
};

/// Tipo de anomalia identificada na camada de rede.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictType {
    /// O mesmo endereço IP está associado a múltiplos endereços MAC diferentes (colisão/ARP spoofing)
    IpCollision,
    /// O mesmo endereço MAC está ativo em mais de um IP simultaneamente
    MacDuplicated,
    /// O IP de um dispositivo cadastrado está respondendo com um MAC diferente do cadastrado
    DeviceMacMismatch,
}

/// Representa um conflito ou anomalia de endereçamento detectado na rede.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConflict {
    pub id: String,
    pub conflict_type: ConflictType,
    pub ip_address: String,
    pub mac_addresses: Vec<String>,
    pub affected_device_id: Option<i64>,
    pub affected_device_name: Option<String>,
    pub vendors: Vec<String>,
    pub severity: String,
    pub description: String,
    pub detected_at: String,
}

/// Identifica conflitos de IP e clonagem de MAC na rede local.
pub async fn analyze_network_conflicts(db: &DatabaseConnection) -> AppResult<Vec<NetworkConflict>> {
    if !can_inspect_l2() {
        return Ok(Vec::new());
    }

    let mut neighbors = neighbor_cache::read_system_neighbors().await;

    // Complementa com resultados observados na descoberta recente
    if let Ok(recent) = crate::models::_entities::discovery_results::Entity::find()
        .filter(crate::models::_entities::discovery_results::Column::MacAddress.is_not_null())
        .all(db)
        .await
    {
        for disc in recent {
            if let Some(mac) = disc
                .mac_address
                .as_deref()
                .and_then(neighbor_cache::normalize_mac)
            {
                neighbors.push(NeighborEntry {
                    ip_address: disc.ip_address,
                    mac_address: mac,
                    interface: None,
                    state: Some("DISCOVERED".into()),
                });
            }
        }
    }

    let deduped = neighbor_cache::dedup_neighbors(neighbors);
    evaluate_conflicts_pure(db, &deduped).await
}

/// Avalia conflitos contra uma lista de vizinhos (função isolada e testável).
pub async fn evaluate_conflicts_pure(
    db: &DatabaseConnection,
    neighbors: &[NeighborEntry],
) -> AppResult<Vec<NetworkConflict>> {
    let mut conflicts = Vec::new();
    let now_str = Utc::now().to_rfc3339();

    // 1. Carrega os dispositivos e interfaces cadastrados
    let all_devices = devices::Entity::find().all(db).await?;
    let all_interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::MacAddress.is_not_null())
        .all(db)
        .await?;

    let mut device_by_ip: HashMap<String, devices::Model> = HashMap::new();
    let mut device_by_id: HashMap<i64, devices::Model> = HashMap::new();
    for dev in all_devices {
        if let Some(ref ip) = dev.ip_address {
            if !ip.is_empty() {
                device_by_ip.insert(ip.clone(), dev.clone());
            }
        }
        device_by_id.insert(dev.id, dev);
    }

    let mut macs_by_device: HashMap<i64, Vec<String>> = HashMap::new();
    let mut device_by_mac: HashMap<String, i64> = HashMap::new();
    for intf in all_interfaces {
        if let Some(ref raw_mac) = intf.mac_address {
            if let Some(mac) = neighbor_cache::normalize_mac(raw_mac) {
                macs_by_device
                    .entry(intf.device_id)
                    .or_default()
                    .push(mac.clone());
                device_by_mac.insert(mac, intf.device_id);
            }
        }
    }

    // 2. DETECÇÃO DE IP COLONADO / COLISÃO DE IP (Múltiplos MACs no mesmo IP)
    let ip_collisions = neighbor_cache::find_ip_collisions(neighbors);
    for (ip, macs) in ip_collisions {
        let dev = device_by_ip.get(&ip);
        let vendors: Vec<String> = macs
            .iter()
            .map(|m| lookup_vendor(m).unwrap_or("Desconhecido").to_string())
            .collect();

        let dev_name = dev.map(|d| d.name.clone());
        let dev_id = dev.map(|d| d.id);

        let vendor_summary = macs
            .iter()
            .zip(vendors.iter())
            .map(|(m, v)| format!("{m} ({v})"))
            .collect::<Vec<_>>()
            .join(", ");

        let description = format!(
            "Conflito de IP: O endereço {ip} está respondendo para múltiplos MACs distintos: {vendor_summary}."
        );

        conflicts.push(NetworkConflict {
            id: format!("ip_collision_{ip}"),
            conflict_type: ConflictType::IpCollision,
            ip_address: ip,
            mac_addresses: macs,
            affected_device_id: dev_id,
            affected_device_name: dev_name,
            vendors,
            severity: "critical".into(),
            description,
            detected_at: now_str.clone(),
        });
    }

    // 3. DETECÇÃO DE MAC DUPLICADO / CLONE DE MAC (Mesmo MAC ativo em múltiplos IPs)
    let mac_duplicates = neighbor_cache::find_mac_duplicates(neighbors);
    for (mac, ips) in mac_duplicates {
        let dev_id = device_by_mac.get(&mac).copied();
        let dev = dev_id.and_then(|id| device_by_id.get(&id));
        let vendor = lookup_vendor(&mac).unwrap_or("Desconhecido").to_string();

        let description = format!(
            "MAC Duplicado/Clonado: O endereço MAC {mac} ({vendor}) está ativo simultaneamente em múltiplos IPs: {}.",
            ips.join(", ")
        );

        conflicts.push(NetworkConflict {
            id: format!("mac_duplicate_{mac}"),
            conflict_type: ConflictType::MacDuplicated,
            ip_address: ips.first().cloned().unwrap_or_default(),
            mac_addresses: vec![mac],
            affected_device_id: dev_id,
            affected_device_name: dev.map(|d| d.name.clone()),
            vendors: vec![vendor],
            severity: "warning".into(),
            description,
            detected_at: now_str.clone(),
        });
    }

    // 4. DETECÇÃO DE DIVERGÊNCIA DE CADASTRO (IP do dispositivo assumido por outro MAC)
    let neighbor_by_ip: BTreeMap<String, String> = neighbors
        .iter()
        .map(|n| (n.ip_address.clone(), n.mac_address.clone()))
        .collect();

    for (dev_id, registered_macs) in &macs_by_device {
        if let Some(dev) = device_by_id.get(dev_id) {
            if let Some(ref ip) = dev.ip_address {
                if let Some(observed_mac) = neighbor_by_ip.get(ip) {
                    if !registered_macs.contains(observed_mac) {
                        let observed_vendor = lookup_vendor(observed_mac)
                            .unwrap_or("Desconhecido")
                            .to_string();

                        let desc = format!(
                            "Divergência de Equipamento: O dispositivo '{}' (IP {ip}) está cadastrado com MAC {}, mas na rede física está respondendo com MAC {observed_mac} ({observed_vendor}).",
                            dev.name,
                            registered_macs.join(", ")
                        );

                        conflicts.push(NetworkConflict {
                            id: format!("mismatch_{}_{ip}", dev.id),
                            conflict_type: ConflictType::DeviceMacMismatch,
                            ip_address: ip.clone(),
                            mac_addresses: vec![observed_mac.clone()],
                            affected_device_id: Some(dev.id),
                            affected_device_name: Some(dev.name.clone()),
                            vendors: vec![observed_vendor],
                            severity: "critical".into(),
                            description: desc,
                            detected_at: now_str.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(conflicts)
}

/// Alerta o sistema gravando eventos e publicando via SSE quando conflitos são detectados.
pub async fn alert_on_conflicts(ctx: &AppContext, conflicts: &[NetworkConflict]) -> AppResult<()> {
    let now = Utc::now();
    let current_ids: std::collections::HashSet<&str> =
        conflicts.iter().map(|c| c.id.as_str()).collect();

    // 1. Auto-resolve alertas de conflitos anteriores que deixaram de existir
    if let Ok(active_events) = alert_events::Entity::find()
        .filter(alert_events::Column::ScopeKey.is_not_null())
        .filter(alert_events::Column::Status.eq("active"))
        .all(&ctx.db)
        .await
    {
        for evt in active_events {
            if let Some(ref scope) = evt.scope_key {
                if (scope.starts_with("ip_collision_")
                    || scope.starts_with("mac_duplicate_")
                    || scope.starts_with("mismatch_"))
                    && !current_ids.contains(scope.as_str())
                {
                    let mut act: alert_events::ActiveModel = evt.into();
                    act.status = Set("resolved".into());
                    act.resolved_at = Set(Some(now.into()));
                    act.updated_at = Set(now.into());
                    let _ = act.update(&ctx.db).await;
                }
            }
        }
    }

    // 2. Registra novos alertas para os conflitos atuais
    for conflict in conflicts {
        // Evita flood de eventos idênticos gravando apenas se não houver alerta recente para o mesmo ID
        let existing = alert_events::Entity::find()
            .filter(alert_events::Column::ScopeKey.eq(&conflict.id))
            .filter(alert_events::Column::Status.eq("active"))
            .one(&ctx.db)
            .await?;

        if existing.is_none() {
            let _ = alert_events::ActiveModel {
                device_id: Set(conflict.affected_device_id),
                scope_key: Set(Some(conflict.id.clone())),
                status: Set("active".into()),
                severity: Set(conflict.severity.clone()),
                started_at: Set(now.into()),
                message: Set(Some(conflict.description.clone())),
                data: Set(Some(serde_json::to_value(conflict).unwrap_or_default())),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await;
        }
    }

    // 3. Notifica a aplicação em tempo real via SSE
    if let Ok(events) = EventBus::from_context(ctx) {
        let _ = events
            .publish(
                &ctx.db,
                "discovery:conflicts_detected",
                serde_json::json!({
                    "count": conflicts.len(),
                    "conflicts": conflicts
                }),
            )
            .await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializacao_de_conflict_type() {
        let json = serde_json::to_string(&ConflictType::IpCollision).unwrap();
        assert_eq!(json, "\"ipCollision\"");
        let json_mac = serde_json::to_string(&ConflictType::MacDuplicated).unwrap();
        assert_eq!(json_mac, "\"macDuplicated\"");
    }
}
