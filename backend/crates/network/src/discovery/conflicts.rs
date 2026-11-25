//! Auditoria de Camada 2: Detecção de IPs clonados, MACs duplicados e divergências de rede.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::vendors;
use netmonitor_core::{
    events::EventBus,
    models::_entities::{
        alert_events, device_interfaces, devices, discovery_results, discovery_runs,
    },
    network_tools::{
        nat::can_inspect_l2,
        neighbor_cache::{self, NeighborEntry},
    },
    shared::{errors::AppResult, transaction::begin_write},
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
    vendors::service::ensure_loaded(db).await;

    let mut neighbors = neighbor_cache::read_system_neighbors().await;

    // Complementa só com a última varredura concluída de cada rede: leases
    // DHCP históricos não viram falso conflito, e uma rede varrida menos de
    // uma vez por dia não alterna o alerta entre resolvido e recriado (o que
    // um corte absoluto de 24h fazia).
    if let Ok(recent) = latest_run_results_with_mac(db).await {
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

/// Os resultados com MAC da última run concluída de cada rede.
async fn latest_run_results_with_mac(
    db: &DatabaseConnection,
) -> AppResult<Vec<discovery_results::Model>> {
    let latest_runs: Vec<i64> = discovery_runs::Entity::find()
        .select_only()
        .column_as(discovery_runs::Column::Id.max(), "id")
        .filter(discovery_runs::Column::Status.eq("completed"))
        .group_by(discovery_runs::Column::NetworkId)
        .into_tuple::<Option<i64>>()
        .all(db)
        .await?
        .into_iter()
        .flatten()
        .collect();
    if latest_runs.is_empty() {
        return Ok(Vec::new());
    }
    Ok(discovery_results::Entity::find()
        .filter(discovery_results::Column::MacAddress.is_not_null())
        .filter(discovery_results::Column::DiscoveryRunId.is_in(latest_runs))
        .all(db)
        .await?)
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
            .map(|m| vendors::vendor_name(m).unwrap_or_else(|| "Desconhecido".into()))
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
        let vendor = vendors::vendor_name(&mac).unwrap_or_else(|| "Desconhecido".into());

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
                        let observed_vendor = vendors::vendor_name(observed_mac)
                            .unwrap_or_else(|| "Desconhecido".into());

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

    // 1. Auto-resolve alertas anteriores e registra novos conflitos numa
    // única transação de escrita.
    let txn = begin_write(&ctx.db).await?;
    let active_events = alert_events::Entity::find()
        .filter(alert_events::Column::ScopeKey.is_not_null())
        .filter(alert_events::Column::Status.eq("active"))
        .all(&txn)
        .await?;
    let mut existing_scopes: std::collections::HashSet<String> = std::collections::HashSet::new();
    for evt in active_events {
        if let Some(ref scope) = evt.scope_key {
            if scope.starts_with("ip_collision_")
                || scope.starts_with("mac_duplicate_")
                || scope.starts_with("mismatch_")
            {
                if current_ids.contains(scope.as_str()) {
                    existing_scopes.insert(scope.clone());
                } else {
                    let mut act: alert_events::ActiveModel = evt.into();
                    act.status = Set("resolved".into());
                    act.resolved_at = Set(Some(now.into()));
                    act.updated_at = Set(now.into());
                    // Um erro aborta a transação no PostgreSQL: o COMMIT
                    // viraria rollback silencioso.
                    act.update(&txn).await?;
                }
            }
        }
    }

    // 2. Registra novos alertas para os conflitos atuais
    for conflict in conflicts {
        if !existing_scopes.contains(&conflict.id) {
            alert_events::ActiveModel {
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
            .insert(&txn)
            .await?;
        }
    }
    txn.commit().await?;

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

    use crate::migrations::{MigratorTrait, TestMigrator};
    use netmonitor_core::models::_entities::networks;

    async fn db() -> DatabaseConnection {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = sea_orm::Database::connect(options).await.unwrap();
        TestMigrator::up(&db, None).await.unwrap();
        db
    }

    async fn network(db: &DatabaseConnection, cidr: &str) -> i64 {
        let now = Utc::now().fixed_offset();
        networks::ActiveModel {
            name: Set(cidr.into()),
            cidr: Set(cidr.into()),
            scan_enabled: Set(true),
            scan_interval: Set(7 * 24 * 3600),
            active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap()
        .id
    }

    async fn run_with(db: &DatabaseConnection, network_id: i64, status: &str, mac: &str) {
        // Varredura antiga: o corte absoluto de 24h a descartaria.
        let seen = (Utc::now() - chrono::Duration::days(6)).fixed_offset();
        let run = discovery_runs::ActiveModel {
            network_id: Set(network_id),
            status: Set(status.into()),
            started_at: Set(seen),
            created_at: Set(seen),
            updated_at: Set(seen),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
        discovery_results::ActiveModel {
            discovery_run_id: Set(run.id),
            ip_address: Set("192.168.0.10".into()),
            mac_address: Set(Some(mac.into())),
            confidence: Set(50),
            first_seen_at: Set(seen),
            last_seen_at: Set(seen),
            created_at: Set(seen),
            updated_at: Set(seen),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn conflitos_usam_a_ultima_run_concluida_de_cada_rede() {
        let db = db().await;
        let semanal = network(&db, "192.168.0.0/24").await;
        let outra = network(&db, "10.0.0.0/24").await;
        run_with(&db, semanal, "completed", "aa:aa:aa:aa:aa:01").await;
        run_with(&db, semanal, "completed", "aa:aa:aa:aa:aa:02").await;
        run_with(&db, semanal, "failed", "aa:aa:aa:aa:aa:03").await;
        run_with(&db, outra, "completed", "aa:aa:aa:aa:aa:04").await;

        let mut macs: Vec<String> = latest_run_results_with_mac(&db)
            .await
            .unwrap()
            .into_iter()
            .filter_map(|result| result.mac_address)
            .collect();
        macs.sort();
        assert_eq!(macs, ["aa:aa:aa:aa:aa:02", "aa:aa:aa:aa:aa:04"]);
    }

    #[test]
    fn serializacao_de_conflict_type() {
        let json = serde_json::to_string(&ConflictType::IpCollision).unwrap();
        assert_eq!(json, "\"ipCollision\"");
        let json_mac = serde_json::to_string(&ConflictType::MacDuplicated).unwrap();
        assert_eq!(json_mac, "\"macDuplicated\"");
    }
}
