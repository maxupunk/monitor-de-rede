//! Equipamentos **transparentes** para o teste de enlace (ADR 015 §5.2).
//!
//! Um equipamento sem IP — o switch não gerenciável, o hub — não tem como
//! medir nada: não fala SSH, não roda plugin, não recebe conexão. Mas o
//! tráfego passa por ele. Por isso ele nunca é ponta de uma perna: o teste
//! mede de um equipamento com IP ao próximo com IP, **passando** pelos
//! transparentes do meio — a medida em L2 dos cabos entre os dois.
//!
//! Funções puras: a regra (`is_transparent`), o colapso do caminho
//! (`collapse`) e o recorte do grafo em que só transparentes ficam no meio
//! (`segment_edges`). O plano só carrega os dados e chama.

use crate::{
    models::devices,
    services::{
        devices::system_device::NETMONITOR_KEY, shared::errors::AppError, topology::path::PathEdge,
    },
};

/// Sem IP o equipamento é transparente. O próprio NetMonitor nunca é: ele é
/// a central, com os motores nativos.
#[must_use]
pub fn is_transparent(device: &devices::Model) -> bool {
    device.system_key.as_deref() != Some(NETMONITOR_KEY)
        && device
            .ip_address
            .as_deref()
            .is_none_or(|ip| ip.trim().is_empty())
}

/// O erro de quando um transparente é pedido como ponta.
#[must_use]
pub fn not_an_endpoint(name: &str) -> AppError {
    AppError::validation(format!(
        "{name} não tem IP — escolha um equipamento depois dele: o teste passa por ele, não \
         para nele"
    ))
}

/// Uma perna entre dois equipamentos com IP: as arestas atravessadas, em
/// ordem de `a` a `b`, e os transparentes do meio, na mesma ordem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    pub a: i64,
    pub b: i64,
    pub via_device_ids: Vec<i64>,
    pub edge_ids: Vec<i64>,
}

/// A aresta da perna quando ela é uma só (o `edgeId` do contrato); `None`
/// com mais de uma ou nenhuma.
#[must_use]
pub fn single_edge(edge_ids: &[i64]) -> Option<i64> {
    match edge_ids {
        [only] => Some(*only),
        _ => None,
    }
}

/// Junta cada sequência de transparentes do caminho numa perna só entre os
/// equipamentos com IP de cada lado.
///
/// `edge_ids[i]` liga `device_ids[i]` a `device_ids[i + 1]` (o formato de
/// [`TopologyPath`](crate::services::topology::path::TopologyPath)).
///
/// # Errors
///
/// O id da ponta (primeira ou última) que é transparente: ela não mede.
pub fn collapse(
    device_ids: &[i64],
    edge_ids: &[i64],
    is_transparent: impl Fn(i64) -> bool,
) -> Result<Vec<Hop>, i64> {
    let (Some(&first), Some(&last)) = (device_ids.first(), device_ids.last()) else {
        return Ok(Vec::new());
    };
    for end in [first, last] {
        if is_transparent(end) {
            return Err(end);
        }
    }
    let mut hops = Vec::new();
    let mut current = Hop {
        a: first,
        b: first,
        via_device_ids: Vec::new(),
        edge_ids: Vec::new(),
    };
    for (index, &device) in device_ids.iter().enumerate().skip(1) {
        if let Some(&edge) = edge_ids.get(index - 1) {
            current.edge_ids.push(edge);
        }
        if index + 1 < device_ids.len() && is_transparent(device) {
            current.via_device_ids.push(device);
            continue;
        }
        current.b = device;
        let next = Hop {
            a: device,
            b: device,
            via_device_ids: Vec::new(),
            edge_ids: Vec::new(),
        };
        hops.push(std::mem::replace(&mut current, next));
    }
    Ok(hops)
}

/// O recorte do grafo em que `from` e `to` só se ligam por transparentes:
/// fica a aresta cujas duas pontas são `from`, `to` ou um transparente. O
/// menor caminho nele é "o enlace entre os dois", com switches no meio.
#[must_use]
pub fn segment_edges(
    edges: &[PathEdge],
    from: i64,
    to: i64,
    is_transparent: impl Fn(i64) -> bool,
) -> Vec<PathEdge> {
    let allowed = |device: i64| device == from || device == to || is_transparent(device);
    edges
        .iter()
        .copied()
        .filter(|&(_, a, b, _)| allowed(a) && allowed(b))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::topology::path::shortest_path;

    fn hop(a: i64, b: i64, via: &[i64], edges: &[i64]) -> Hop {
        Hop {
            a,
            b,
            via_device_ids: via.to_vec(),
            edge_ids: edges.to_vec(),
        }
    }

    #[test]
    fn sem_transparente_cada_aresta_e_uma_perna() {
        let hops = collapse(&[1, 2, 3], &[10, 20], |_| false).unwrap();
        assert_eq!(hops, [hop(1, 2, &[], &[10]), hop(2, 3, &[], &[20])]);
        assert_eq!(single_edge(&hops[0].edge_ids), Some(10));
    }

    #[test]
    fn um_switch_no_meio_vira_uma_perna_so() {
        // AP-terraço (1) — switch-assistencia (2) — BORDA (3).
        let hops = collapse(&[1, 2, 3], &[10, 20], |id| id == 2).unwrap();
        assert_eq!(hops, [hop(1, 3, &[2], &[10, 20])]);
        assert_eq!(single_edge(&hops[0].edge_ids), None, "duas arestas");
    }

    #[test]
    fn dois_switches_seguidos_e_um_ip_depois() {
        let hops = collapse(&[1, 2, 3, 4, 5], &[10, 20, 30, 40], |id| id == 2 || id == 3).unwrap();
        assert_eq!(
            hops,
            [hop(1, 4, &[2, 3], &[10, 20, 30]), hop(4, 5, &[], &[40])]
        );
    }

    #[test]
    fn transparente_na_ponta_e_recusado() {
        assert_eq!(collapse(&[2, 1], &[10], |id| id == 2), Err(2));
        assert_eq!(collapse(&[1, 3, 2], &[10, 20], |id| id >= 2), Err(2));
        assert_eq!(collapse(&[], &[], |_| true), Ok(Vec::new()));
    }

    fn device(ip: Option<&str>, system: bool) -> devices::Model {
        let now = chrono::Utc::now().into();
        devices::Model {
            id: 1,
            site_id: None,
            network_id: None,
            parent_id: None,
            ip_address: ip.map(str::to_owned),
            name: "D".into(),
            r#type: "unmanaged_switch".into(),
            vendor: None,
            model: None,
            serial_number: None,
            description: None,
            is_monitored: false,
            snmp_enabled: false,
            snmp_community: None,
            snmp_version: None,
            snmp_poll_interval_seconds: 60,
            access_mode: None,
            operating_system: None,
            syslog_server_address: None,
            system_key: system.then(|| NETMONITOR_KEY.to_owned()),
            link_interface_id: None,
            link_interface_name: None,
            firmware_version: None,
            observed_os: None,
            observed_os_source: None,
            observed_os_reason: None,
            status: "unknown".into(),
            last_seen_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn sem_ip_e_transparente_e_o_netmonitor_nunca_e() {
        assert!(is_transparent(&device(None, false)));
        assert!(is_transparent(&device(Some("  "), false)));
        assert!(!is_transparent(&device(Some("10.0.0.1"), false)));
        assert!(
            !is_transparent(&device(None, true)),
            "a central sempre mede"
        );
    }

    #[test]
    fn recorte_so_deixa_transparentes_no_meio() {
        // 1 — 2(switch) — 3, e 1 — 4(com IP) — 3.
        let edges: Vec<PathEdge> = vec![
            (10, 1, 2, 100),
            (20, 2, 3, 100),
            (30, 1, 4, 100),
            (40, 4, 3, 100),
        ];
        let recorte = segment_edges(&edges, 1, 3, |id| id == 2);
        let path = shortest_path(&recorte, 1, 3).unwrap();
        assert_eq!(path.device_ids, [1, 2, 3]);
        assert_eq!(path.edge_ids, [10, 20]);
        // Sem switch, só há caminho por um equipamento com IP: nada.
        let sem_switch = segment_edges(&edges[2..], 1, 3, |id| id == 2);
        assert!(shortest_path(&sem_switch, 1, 3).is_none());
    }
}
