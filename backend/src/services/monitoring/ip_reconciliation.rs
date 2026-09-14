//! Reconciliação automática de endereço IP por endereço MAC.
//!
//! Quando o monitor de ping de um equipamento falha e o sistema está rodando com
//! acesso à Camada 2 (modo `host`), inspeciona a tabela ARP para verificar se o
//! MAC do dispositivo migrou para outro endereço IP. Se o novo IP responder ao
//! ping, atualiza o cadastro, registra no histórico de eventos do dispositivo e
//! suprime o alarme de queda.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};

use crate::{
    models::_entities::{alert_events, device_interfaces, devices, discovery_results, monitors},
    services::{
        events::EventBus,
        monitoring::{
            contracts::{CheckResult, MonitorStatus},
            runner::{run_monitor, RunOptions},
        },
        network_tools::neighbor_cache::{self, normalize_mac},
        shared::errors::AppResult,
        syslog::nat::NatDetector,
    },
};

/// Informa se o processo atual possui visão L2 da rede local (modo `host` ou fora do container).
#[must_use]
pub fn can_inspect_l2() -> bool {
    !NatDetector::detect().bridged_container()
}

/// Recupera todos os endereços MAC conhecidos associados a um dispositivo.
pub async fn get_device_mac_addresses(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
) -> AppResult<Vec<String>> {
    let mut macs = Vec::new();

    // 1. Interfaces cadastradas do dispositivo
    let interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(device_id))
        .all(db)
        .await?;

    for intf in interfaces {
        if let Some(raw_mac) = intf.mac_address {
            if let Some(mac) = normalize_mac(&raw_mac) {
                if !macs.contains(&mac) {
                    macs.push(mac);
                }
            }
        }
    }

    // 2. Se não houver MAC nas interfaces, tenta descobrir por cache de discovery pelo IP cadastrado
    if macs.is_empty() {
        if let Some(device) = devices::Entity::find_by_id(device_id).one(db).await? {
            if let Some(ip) = device.ip_address.as_deref().filter(|s| !s.is_empty()) {
                let discovery = discovery_results::Entity::find()
                    .filter(discovery_results::Column::IpAddress.eq(ip))
                    .filter(discovery_results::Column::MacAddress.is_not_null())
                    .one(db)
                    .await?;

                if let Some(disc) = discovery {
                    if let Some(raw_mac) = disc.mac_address {
                        if let Some(mac) = normalize_mac(&raw_mac) {
                            macs.push(mac.clone());
                            // Registra uma interface padrão para guardar o MAC de forma persistente
                            let now = Utc::now();
                            let _ = device_interfaces::ActiveModel {
                                device_id: Set(device_id),
                                name: Set("eth0".into()),
                                mac_address: Set(Some(mac)),
                                created_at: Set(now.into()),
                                updated_at: Set(now.into()),
                                ..Default::default()
                            }
                            .insert(db)
                            .await;
                        }
                    }
                }
            }
        }
    }

    Ok(macs)
}

/// Garante que um MAC conhecido seja gravado como interface de um dispositivo.
pub async fn associate_device_mac(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    mac: &str,
) -> AppResult<()> {
    let Some(normalized) = normalize_mac(mac) else {
        return Ok(());
    };

    let existing = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(device_id))
        .filter(device_interfaces::Column::MacAddress.eq(&normalized))
        .one(db)
        .await?;

    if existing.is_none() {
        let now = Utc::now();
        let _ = device_interfaces::ActiveModel {
            device_id: Set(device_id),
            name: Set("eth0".into()),
            mac_address: Set(Some(normalized)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(())
}

/// Tenta reconciliar o endereço IP de um dispositivo quando a checagem de alcance falha.
///
/// Se o container estiver em modo `host` e o MAC do dispositivo for encontrado respondendo
/// em outro IP que esteja online, o IP do dispositivo e o monitor são atualizados, um evento é
/// adicionado ao histórico do dispositivo e o resultado é retornado como sucesso (`Up`).
pub async fn try_reconcile_device_ip_on_failure(
    ctx: &AppContext,
    monitor: &monitors::Model,
    _failure_result: &CheckResult,
) -> AppResult<Option<CheckResult>> {
    // 1. Apenas aplicável se houver dispositivo associado e o monitor for de alcance
    let Some(device_id) = monitor.device_id else {
        return Ok(None);
    };

    if !monitor.r#type.eq_ignore_ascii_case("ping") {
        return Ok(None);
    }

    // 2. Apenas se tiver visão de camada 2 (modo host)
    if !can_inspect_l2() {
        tracing::debug!(
            device_id,
            "reconciliação de IP ignorada: processo sem acesso L2 (modo bridge)"
        );
        return Ok(None);
    }

    // 3. Carrega o dispositivo
    let Some(device) = devices::Entity::find_by_id(device_id).one(&ctx.db).await? else {
        return Ok(None);
    };

    let current_ip = match device.ip_address.as_deref().filter(|s| !s.is_empty()) {
        Some(ip) => ip.to_string(),
        None => return Ok(None),
    };

    // 4. Carrega os MACs conhecidos do dispositivo
    let known_macs = get_device_mac_addresses(&ctx.db, device_id).await?;
    if known_macs.is_empty() {
        tracing::debug!(
            device_id,
            device_name = %device.name,
            "dispositivo sem MAC cadastrado; reconciliação impossibilitada"
        );
        return Ok(None);
    }

    // 5. Inspeciona a tabela de vizinhos ARP
    let neighbors = neighbor_cache::read_system_neighbors().await;
    let mut candidate_ip = None;
    let mut matched_mac = None;

    for mac in &known_macs {
        let ips = neighbor_cache::find_ips_for_mac(&neighbors, mac);
        for ip in ips {
            if ip != current_ip {
                candidate_ip = Some(ip);
                matched_mac = Some(mac.clone());
                break;
            }
        }
        if candidate_ip.is_some() {
            break;
        }
    }

    // Fallback: se não estiver na tabela ARP ativa, verifica se a última descoberta registrou o MAC
    if candidate_ip.is_none() {
        for mac in &known_macs {
            if let Ok(Some(disc)) = discovery_results::Entity::find()
                .filter(discovery_results::Column::MacAddress.eq(mac))
                .filter(discovery_results::Column::IpAddress.ne(&current_ip))
                .order_by_desc(discovery_results::Column::UpdatedAt)
                .one(&ctx.db)
                .await
            {
                candidate_ip = Some(disc.ip_address);
                matched_mac = Some(mac.clone());
                break;
            }
        }
    }

    let Some(new_ip) = candidate_ip else {
        return Ok(None);
    };

    tracing::info!(
        device_id = device.id,
        device_name = %device.name,
        old_ip = %current_ip,
        new_ip = %new_ip,
        mac = ?matched_mac,
        "potencial alteração de IP detectada via ARP; testando alcance do novo IP"
    );

    // 6. Testa alcance (ping) no novo IP antes de confirmar a alteração
    let ping_config = serde_json::json!({
        "host": new_ip,
        "packetCount": 2,
        "timeoutMs": 3000
    });

    let ping_test = run_monitor(
        ctx,
        "ping",
        &ping_config,
        RunOptions {
            timeout_ms: Some(3000),
        },
    )
    .await;

    let Ok(new_result) = ping_test else {
        return Ok(None);
    };

    if new_result.status != MonitorStatus::Up {
        tracing::warn!(
            device_id = device.id,
            new_ip = %new_ip,
            "novo IP detectado via ARP não respondeu ao ping; mantendo falha"
        );
        return Ok(None);
    }

    // 7. SUCESSO: Atualiza o dispositivo
    let mut dev_active: devices::ActiveModel = device.clone().into();
    dev_active.ip_address = Set(Some(new_ip.clone()));
    dev_active.updated_at = Set(Utc::now().into());
    let _ = dev_active.update(&ctx.db).await?;

    // 8. Registra no histórico de eventos do dispositivo (/devices/[id] > Histórico de Eventos)
    let mac_label = matched_mac.as_deref().unwrap_or("N/A");
    let event_message = format!(
        "Endereço IP alterado automaticamente de {current_ip} para {new_ip} (detectado via MAC {mac_label})"
    );
    let now = Utc::now();
    let _ = alert_events::ActiveModel {
        device_id: Set(Some(device.id)),
        monitor_id: Set(Some(monitor.id)),
        status: Set("info".into()),
        severity: Set("info".into()),
        started_at: Set(now.into()),
        resolved_at: Set(Some(now.into())),
        message: Set(Some(event_message.clone())),
        data: Set(Some(serde_json::json!({
            "oldIp": current_ip,
            "newIp": new_ip,
            "macAddress": matched_mac,
            "reconciliation": "auto_ip_change"
        }))),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await;

    // 9. Sincroniza a configuração do monitor atual com o novo IP
    let mut mon_active: monitors::ActiveModel = monitor.clone().into();
    let mut new_config = monitor.configuration.clone();
    if let serde_json::Value::Object(ref mut map) = new_config {
        map.insert("host".into(), serde_json::Value::String(new_ip.clone()));
    }
    mon_active.configuration = Set(new_config);
    mon_active.updated_at = Set(now.into());
    let _ = mon_active.update(&ctx.db).await;

    // Sincroniza outros monitores associados a este dispositivo
    if let Ok(other_monitors) = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .all(&ctx.db)
        .await
    {
        for mon in other_monitors {
            if mon.id == monitor.id {
                continue;
            }
            let mut mon_config = mon.configuration.clone();
            let mut modified = false;
            if let serde_json::Value::Object(ref mut map) = mon_config {
                if let Some(host_val) = map.get_mut("host") {
                    if host_val.as_str() == Some(&current_ip) {
                        *host_val = serde_json::Value::String(new_ip.clone());
                        modified = true;
                    }
                }
                if let Some(ip_val) = map.get_mut("ip") {
                    if ip_val.as_str() == Some(&current_ip) {
                        *ip_val = serde_json::Value::String(new_ip.clone());
                        modified = true;
                    }
                }
            }
            if modified {
                let mut act: monitors::ActiveModel = mon.into();
                act.configuration = Set(mon_config);
                act.updated_at = Set(now.into());
                let _ = act.update(&ctx.db).await;
            }
        }
    }

    // 10. Notifica a aplicação em tempo real via SSE
    if let Ok(events) = EventBus::from_context(ctx) {
        let _ = events
            .publish(
                &ctx.db,
                "device:updated",
                serde_json::json!({
                    "deviceId": device.id,
                    "ipAddress": new_ip,
                    "previousIpAddress": current_ip,
                    "reconciliation": "ip_changed"
                }),
            )
            .await;
    }

    tracing::info!(
        device_id = device.id,
        device_name = %device.name,
        old_ip = %current_ip,
        new_ip = %new_ip,
        "IP do dispositivo reconciliado com sucesso; alerta de queda suprimido"
    );

    Ok(Some(new_result))
}

/// Aprende e grava o endereço MAC automaticamente a partir da tabela ARP
/// quando o dispositivo responde com sucesso ao ping e ainda não possui MAC cadastrado.
pub async fn auto_learn_device_mac_if_missing(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    ip: &str,
) -> AppResult<()> {
    if !can_inspect_l2() {
        return Ok(());
    }

    let count = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(device_id))
        .filter(device_interfaces::Column::MacAddress.is_not_null())
        .count(db)
        .await?;

    if count > 0 {
        return Ok(());
    }

    let neighbors = neighbor_cache::read_system_neighbors().await;
    for entry in neighbors {
        if entry.ip_address == ip {
            associate_device_mac(db, device_id, &entry.mac_address).await?;
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deteccao_de_modo_host_funciona() {
        // can_inspect_l2 deve retornar boolean determinístico
        let _ = can_inspect_l2();
    }
}
