//! Inferência de enlaces por sub-rede, para quem não fala LLDP/CDP.
//!
//! Cada dispositivo final de uma rede ganha **um** enlace inferido até o
//! equipamento de infraestrutura mais provável daquela rede — não um enlace
//! para cada switch e roteador (o N×M antigo transformava o mapa num novelo).

use std::collections::{BTreeMap, BTreeSet};

use crate::{models::devices, services::topology::link_resolver::NetworkLink};

/// Confiança de um enlace inferido: abaixo de qualquer observação (LLDP 95,
/// CDP 90) e do manual (100), para que nunca sobrescreva nenhum deles.
pub const INFERRED_CONFIDENCE: i32 = 60;
pub const INFERRED_METHOD: &str = "subnet_inference";

/// Prioridade do equipamento como uplink: o "gateway" (roteador/firewall)
/// antes do switch. `None` = não é infraestrutura.
fn infra_rank(device_type: &str) -> Option<u8> {
    match device_type {
        "router" | "firewall" => Some(0),
        "switch" | "unmanaged_switch" => Some(1),
        _ => None,
    }
}

/// Par sem orientação, para comparar enlaces gravados nos dois sentidos.
#[must_use]
pub const fn pair_key(a: i64, b: i64) -> (i64, i64) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Enlaces inferidos para os dispositivos finais de cada rede.
///
/// Para cada dispositivo não-infraestrutura com `network_id`:
/// - se o `parent_id` aponta para uma infraestrutura da mesma rede, é ela;
/// - se o `parent_id` aponta para outro lugar, o operador já declarou o
///   uplink — nada é inferido;
/// - senão, a infraestrutura de melhor `infra_rank`, desempatando pelo menor id.
///
/// Pares em `taken` (já ligados por qualquer enlace) ficam de fora.
#[must_use]
pub fn infer_uplinks(devices: &[devices::Model], taken: &BTreeSet<(i64, i64)>) -> Vec<NetworkLink> {
    let mut infra_by_network = BTreeMap::<i64, Vec<&devices::Model>>::new();
    for device in devices {
        if let (Some(network), Some(_)) = (device.network_id, infra_rank(&device.r#type)) {
            infra_by_network.entry(network).or_default().push(device);
        }
    }
    let mut links = Vec::new();
    for device in devices {
        let Some(network) = device.network_id else {
            continue;
        };
        if infra_rank(&device.r#type).is_some() {
            continue;
        }
        let mut candidates = infra_by_network
            .get(&network)
            .into_iter()
            .flatten()
            .copied();
        let uplink = match device.parent_id {
            Some(parent) => candidates.find(|candidate| candidate.id == parent),
            None => {
                candidates.min_by_key(|candidate| (infra_rank(&candidate.r#type), candidate.id))
            }
        };
        let Some(uplink) = uplink else {
            continue;
        };
        if taken.contains(&pair_key(uplink.id, device.id)) {
            continue;
        }
        links.push(NetworkLink {
            source_device_id: uplink.id,
            target_device_id: device.id,
            source_interface_id: None,
            target_interface_id: device.link_interface_id,
            link_type: "inferred".into(),
            discovery_method: INFERRED_METHOD.into(),
            confidence: INFERRED_CONFIDENCE,
            confirmed: false,
        });
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispositivo(id: i64, tipo: &str, rede: Option<i64>, pai: Option<i64>) -> devices::Model {
        devices::Model {
            id,
            site_id: None,
            network_id: rede,
            parent_id: pai,
            ip_address: None,
            name: format!("dev-{id}"),
            r#type: tipo.into(),
            vendor: None,
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
            firmware_version: None,
            observed_os: None,
            observed_os_source: None,
            observed_os_reason: None,
            status: "online".into(),
            last_seen_at: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn pares(links: &[NetworkLink]) -> Vec<(i64, i64)> {
        links
            .iter()
            .map(|link| (link.source_device_id, link.target_device_id))
            .collect()
    }

    #[test]
    fn cada_host_ganha_um_unico_uplink_preferindo_o_roteador() {
        let devices = vec![
            dispositivo(1, "switch", Some(10), None),
            dispositivo(2, "router", Some(10), None),
            dispositivo(3, "switch", Some(10), None),
            dispositivo(4, "server", Some(10), None),
            dispositivo(5, "workstation", Some(10), None),
        ];
        let links = infer_uplinks(&devices, &BTreeSet::new());
        // Antes: 3 infra × 2 hosts = 6 enlaces.
        assert_eq!(pares(&links), vec![(2, 4), (2, 5)]);
        assert!(links
            .iter()
            .all(|link| link.confidence == 60 && !link.confirmed));
    }

    #[test]
    fn sem_roteador_vence_o_switch_de_menor_id() {
        let devices = vec![
            dispositivo(8, "switch", Some(10), None),
            dispositivo(3, "unmanaged_switch", Some(10), None),
            dispositivo(4, "server", Some(10), None),
        ];
        assert_eq!(
            pares(&infer_uplinks(&devices, &BTreeSet::new())),
            vec![(3, 4)]
        );
    }

    #[test]
    fn parent_id_da_mesma_rede_e_o_uplink_escolhido() {
        let devices = vec![
            dispositivo(1, "router", Some(10), None),
            dispositivo(2, "switch", Some(10), None),
            dispositivo(4, "server", Some(10), Some(2)),
        ];
        assert_eq!(
            pares(&infer_uplinks(&devices, &BTreeSet::new())),
            vec![(2, 4)]
        );
    }

    #[test]
    fn parent_id_fora_da_rede_dispensa_a_inferencia() {
        let devices = vec![
            dispositivo(1, "router", Some(10), None),
            dispositivo(4, "server", Some(10), Some(99)),
        ];
        assert!(infer_uplinks(&devices, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn par_ja_ligado_nao_e_duplicado_em_nenhum_sentido() {
        let devices = vec![
            dispositivo(1, "router", Some(10), None),
            dispositivo(4, "server", Some(10), None),
        ];
        let ligados = BTreeSet::from([pair_key(4, 1)]);
        assert!(infer_uplinks(&devices, &ligados).is_empty());
    }

    #[test]
    fn redes_diferentes_nao_se_misturam_e_sem_rede_nao_infere() {
        let devices = vec![
            dispositivo(1, "router", Some(10), None),
            dispositivo(2, "router", Some(20), None),
            dispositivo(3, "server", Some(20), None),
            dispositivo(4, "server", None, None),
        ];
        assert_eq!(
            pares(&infer_uplinks(&devices, &BTreeSet::new())),
            vec![(2, 3)]
        );
    }

    #[test]
    fn infraestrutura_nao_recebe_uplink_inferido() {
        let devices = vec![
            dispositivo(1, "router", Some(10), None),
            dispositivo(2, "switch", Some(10), None),
        ];
        assert!(infer_uplinks(&devices, &BTreeSet::new()).is_empty());
    }
}
