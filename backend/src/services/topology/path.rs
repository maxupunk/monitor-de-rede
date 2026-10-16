//! Menor caminho entre dois dispositivos no grafo da topologia.
//!
//! Função **pura**: recebe as arestas já carregadas e não sabe de banco nem de
//! HTTP. O endpoint `GET /api/topology/path` e o orquestrador de teste de
//! enlace usam a mesma função, para que "o caminho que a tela desenha" e "o
//! caminho que o teste percorre" nunca divirjam.
//!
//! Regras:
//! - o grafo é **não direcionado** — um enlace LLDP descoberto de um lado vale
//!   nos dois sentidos;
//! - ganha o caminho com **menos saltos**; entre caminhos do mesmo tamanho,
//!   ganha o de maior confiança somada (enlace confirmado/manual tem 100,
//!   inferido tem 60), e o empate restante cai na ordem dos ids — o resultado
//!   é **determinístico**, independente da ordem em que as arestas chegam.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};

use serde::Serialize;
use ts_rs::TS;

/// Aresta de entrada: `(id da aresta, dispositivo a, dispositivo b, confiança 0..=100)`.
///
/// O id segue o contrato do grafo: positivo é linha de `device_links`, negativo
/// é a aresta virtual de hierarquia (`-id do filho`).
pub type PathEdge = (i64, i64, i64, u8);

/// Caminho encontrado: os dispositivos em ordem, de `from` a `to`, e as
/// arestas entre eles (`edge_ids[i]` liga `device_ids[i]` a `device_ids[i + 1]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyPath {
    #[ts(type = "number")]
    pub from: i64,
    #[ts(type = "number")]
    pub to: i64,
    #[ts(type = "number[]")]
    pub device_ids: Vec<i64>,
    #[ts(type = "number[]")]
    pub edge_ids: Vec<i64>,
    /// Quantidade de saltos (`edge_ids.len()`).
    pub hops: usize,
}

/// Custo lexicográfico: primeiro saltos, depois a "desconfiança" acumulada.
type Cost = (usize, u32);

/// Menor caminho entre `from` e `to`, ou `None` quando não há ligação conhecida.
///
/// `from == to` devolve o caminho trivial de zero saltos.
#[must_use]
pub fn shortest_path(edges: &[PathEdge], from: i64, to: i64) -> Option<TopologyPath> {
    if from == to {
        return Some(TopologyPath {
            from,
            to,
            device_ids: vec![from],
            edge_ids: Vec::new(),
            hops: 0,
        });
    }

    // Adjacência ordenada: vizinho, confiança decrescente, id da aresta. É o
    // que torna o resultado independente da ordem de entrada.
    let mut adjacency = BTreeMap::<i64, Vec<(i64, u8, i64)>>::new();
    for &(id, a, b, confidence) in edges {
        if a == b {
            continue;
        }
        let confidence = confidence.min(100);
        adjacency.entry(a).or_default().push((b, confidence, id));
        adjacency.entry(b).or_default().push((a, confidence, id));
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by_key(|&(neighbor, confidence, id)| (neighbor, Reverse(confidence), id));
    }
    if !adjacency.contains_key(&from) || !adjacency.contains_key(&to) {
        return None;
    }

    let mut best = BTreeMap::<i64, Cost>::from([(from, (0, 0))]);
    let mut previous = BTreeMap::<i64, (i64, i64)>::new();
    let mut queue = BinaryHeap::from([Reverse(((0usize, 0u32), from))]);

    while let Some(Reverse((cost, device))) = queue.pop() {
        if best.get(&device).is_some_and(|known| *known < cost) {
            continue;
        }
        if device == to {
            break;
        }
        for &(neighbor, confidence, edge_id) in adjacency.get(&device).into_iter().flatten() {
            let next = (cost.0 + 1, cost.1 + u32::from(100 - confidence));
            if best.get(&neighbor).is_none_or(|known| next < *known) {
                best.insert(neighbor, next);
                previous.insert(neighbor, (device, edge_id));
                queue.push(Reverse((next, neighbor)));
            }
        }
    }

    previous.contains_key(&to).then(|| {
        let mut device_ids = vec![to];
        let mut edge_ids = Vec::new();
        let mut cursor = to;
        while let Some(&(prev, edge_id)) = previous.get(&cursor) {
            edge_ids.push(edge_id);
            device_ids.push(prev);
            cursor = prev;
            if cursor == from {
                break;
            }
        }
        device_ids.reverse();
        edge_ids.reverse();
        TopologyPath {
            from,
            to,
            hops: edge_ids.len(),
            device_ids,
            edge_ids,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesmo_dispositivo_e_caminho_trivial() {
        let caminho = shortest_path(&[], 7, 7).expect("trivial");
        assert_eq!(caminho.device_ids, vec![7]);
        assert!(caminho.edge_ids.is_empty());
        assert_eq!(caminho.hops, 0);
    }

    #[test]
    fn sem_ligacao_nao_ha_caminho() {
        assert_eq!(shortest_path(&[(1, 1, 2, 100)], 1, 3), None);
        assert_eq!(shortest_path(&[(1, 1, 2, 100), (2, 3, 4, 100)], 1, 4), None);
    }

    #[test]
    fn grafo_e_nao_direcionado() {
        // Enlace gravado 2 → 1 serve para ir de 1 a 2.
        let caminho = shortest_path(&[(10, 2, 1, 95)], 1, 2).expect("caminho");
        assert_eq!(caminho.device_ids, vec![1, 2]);
        assert_eq!(caminho.edge_ids, vec![10]);
    }

    #[test]
    fn menos_saltos_vence_mesmo_com_confianca_menor() {
        // 1 - 2 - 3 - 4 confirmado, contra 1 - 4 inferido direto.
        let arestas = [
            (1, 1, 2, 100),
            (2, 2, 3, 100),
            (3, 3, 4, 100),
            (4, 1, 4, 60),
        ];
        let caminho = shortest_path(&arestas, 1, 4).expect("caminho");
        assert_eq!(caminho.device_ids, vec![1, 4]);
        assert_eq!(caminho.edge_ids, vec![4]);
        assert_eq!(caminho.hops, 1);
    }

    #[test]
    fn no_empate_de_saltos_vence_a_maior_confianca() {
        // Dois caminhos de 2 saltos de 1 a 4: por 2 (inferido) e por 3 (LLDP).
        let arestas = [(1, 1, 2, 60), (2, 2, 4, 60), (3, 1, 3, 95), (4, 3, 4, 100)];
        let caminho = shortest_path(&arestas, 1, 4).expect("caminho");
        assert_eq!(caminho.device_ids, vec![1, 3, 4]);
        assert_eq!(caminho.edge_ids, vec![3, 4]);
    }

    #[test]
    fn arestas_paralelas_escolhem_a_mais_confiavel() {
        let arestas = [(5, 1, 2, 60), (9, 2, 1, 100)];
        let caminho = shortest_path(&arestas, 1, 2).expect("caminho");
        assert_eq!(caminho.edge_ids, vec![9]);
    }

    #[test]
    fn empate_total_e_deterministico_independente_da_ordem() {
        // Losango 1 → {2, 3} → 4, tudo igual: vence o menor id de vizinho.
        let arestas = vec![
            (1, 1, 2, 100),
            (2, 2, 4, 100),
            (3, 1, 3, 100),
            (4, 3, 4, 100),
        ];
        let esperado = shortest_path(&arestas, 1, 4).expect("caminho");
        assert_eq!(esperado.device_ids, vec![1, 2, 4]);

        let mut invertidas = arestas.clone();
        invertidas.reverse();
        assert_eq!(shortest_path(&invertidas, 1, 4), Some(esperado));
    }

    #[test]
    fn aresta_virtual_de_hierarquia_entra_no_caminho() {
        // Servidor (1) → roteador (2) por enlace real, roteador → AP (3) pela
        // hierarquia declarada (id negativo = -filho).
        let arestas = [(11, 1, 2, 95), (-3, 2, 3, 100)];
        let caminho = shortest_path(&arestas, 1, 3).expect("caminho");
        assert_eq!(caminho.device_ids, vec![1, 2, 3]);
        assert_eq!(caminho.edge_ids, vec![11, -3]);
    }

    #[test]
    fn ciclos_nao_prendem_a_busca() {
        let arestas = [
            (1, 1, 2, 100),
            (2, 2, 3, 100),
            (3, 3, 1, 100),
            (4, 3, 5, 100),
        ];
        let caminho = shortest_path(&arestas, 2, 5).expect("caminho");
        assert_eq!(caminho.device_ids, vec![2, 3, 5]);
    }

    #[test]
    fn laco_no_mesmo_dispositivo_e_ignorado() {
        assert_eq!(shortest_path(&[(1, 1, 1, 100)], 1, 2), None);
    }
}
