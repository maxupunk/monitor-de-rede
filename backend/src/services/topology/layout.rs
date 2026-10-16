//! Persistência do layout do mapa de topologia.
//!
//! O layout é guardado em `system_settings` como JSON, usando uma chave por
//! `site_id` (ou uma chave global quando nenhum site é informado). Essa escolha
//! evita nova migration: o par já existe e é replicado nos backups.

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::system_settings,
    services::shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

const STORAGE_KEY: &str = "topology_layout";
const MAX_NODES: usize = 10_000;

fn storage_key(site_id: Option<i64>) -> String {
    match site_id {
        Some(id) => format!("{STORAGE_KEY}:{id}"),
        None => STORAGE_KEY.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyLayoutNode {
    #[ts(type = "number")]
    pub device_id: i64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyLayout {
    pub nodes: Vec<TopologyLayoutNode>,
}

/// Carrega o layout gravado para o site (ou o layout global).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn load_layout<C: ConnectionTrait>(
    db: &C,
    site_id: Option<i64>,
) -> AppResult<TopologyLayout> {
    let setting = system_settings::Model::get(db, &storage_key(site_id)).await?;
    let layout = setting
        .and_then(|row| row.value)
        .and_then(|value| serde_json::from_str::<TopologyLayout>(&value).ok())
        .unwrap_or_default();
    Ok(layout)
}

fn validate(layout: &TopologyLayout) -> AppResult<()> {
    if layout.nodes.len() > MAX_NODES {
        return Err(AppError::validation(format!(
            "Layout excede o limite de {MAX_NODES} nós"
        )));
    }
    if layout
        .nodes
        .iter()
        .any(|node| !node.x.is_finite() || !node.y.is_finite())
    {
        return Err(AppError::validation("Posição de nó inválida no layout"));
    }
    Ok(())
}

async fn store<C: ConnectionTrait>(
    db: &C,
    site_id: Option<i64>,
    layout: &TopologyLayout,
) -> AppResult<()> {
    let value = serde_json::to_string(layout)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, &storage_key(site_id), Some(value)).await?;
    Ok(())
}

/// Grava o layout inteiro para o site (ou o layout global), substituindo o anterior.
///
/// # Errors
///
/// Propaga erro do banco ou rejeita payloads absurdamente grandes.
pub async fn save_layout(
    db: &DatabaseConnection,
    site_id: Option<i64>,
    layout: TopologyLayout,
) -> AppResult<TopologyLayout> {
    validate(&layout)?;
    store(db, site_id, &layout).await?;
    Ok(layout)
}

/// Funde posições parciais no layout gravado: nó já presente muda de lugar,
/// nó novo entra no fim, o resto fica como estava. Se o mesmo nó vier duas
/// vezes no lote, vale a última posição.
#[must_use]
pub fn merge_layout(stored: TopologyLayout, changes: Vec<TopologyLayoutNode>) -> TopologyLayout {
    let mut latest = BTreeMap::new();
    let mut order = Vec::new();
    for node in changes {
        if latest.insert(node.device_id, node.clone()).is_none() {
            order.push(node.device_id);
        }
    }
    let mut nodes: Vec<TopologyLayoutNode> = stored
        .nodes
        .into_iter()
        .map(|node| latest.remove(&node.device_id).unwrap_or(node))
        .collect();
    nodes.extend(order.into_iter().filter_map(|id| latest.remove(&id)));
    TopologyLayout { nodes }
}

/// Grava só as posições que mudaram (a tela arrasta um nó e manda só ele).
///
/// Ler, fundir e gravar ficam na mesma transação de escrita: duas abas
/// arrastando nós diferentes ao mesmo tempo não apagam a posição uma da outra.
///
/// # Errors
///
/// Propaga erro do banco ou rejeita um layout resultante grande demais.
pub async fn patch_layout(
    db: &DatabaseConnection,
    site_id: Option<i64>,
    changes: Vec<TopologyLayoutNode>,
) -> AppResult<TopologyLayout> {
    let txn = begin_write(db).await?;
    let merged = merge_layout(load_layout(&txn, site_id).await?, changes);
    validate(&merged)?;
    store(&txn, site_id, &merged).await?;
    txn.commit().await?;
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};

    async fn banco() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    #[tokio::test]
    async fn layout_inexistente_devolve_documento_vazio() {
        let db = banco().await;
        let layout = load_layout(&db, None).await.expect("carregar");
        assert!(layout.nodes.is_empty());
    }

    #[tokio::test]
    async fn layout_e_gravado_e_lido_de_volta() {
        let db = banco().await;
        let layout = TopologyLayout {
            nodes: vec![
                TopologyLayoutNode {
                    device_id: 1,
                    x: 10.5,
                    y: 20.0,
                },
                TopologyLayoutNode {
                    device_id: 7,
                    x: 100.0,
                    y: 200.0,
                },
            ],
        };
        save_layout(&db, None, layout.clone())
            .await
            .expect("gravar");

        let carregado = load_layout(&db, None).await.expect("carregar");
        assert_eq!(carregado.nodes.len(), 2);
        assert_eq!(carregado.nodes[0].device_id, 1);
        assert!((carregado.nodes[0].x - 10.5).abs() < f64::EPSILON);
        assert!((carregado.nodes[0].y - 20.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn layouts_sao_independentes_por_site() {
        let db = banco().await;
        let global = TopologyLayout {
            nodes: vec![TopologyLayoutNode {
                device_id: 1,
                x: 1.0,
                y: 1.0,
            }],
        };
        let site = TopologyLayout {
            nodes: vec![TopologyLayoutNode {
                device_id: 2,
                x: 2.0,
                y: 2.0,
            }],
        };

        save_layout(&db, None, global).await.expect("gravar global");
        save_layout(&db, Some(42), site).await.expect("gravar site");

        let global_carregado = load_layout(&db, None).await.expect("carregar global");
        let site_carregado = load_layout(&db, Some(42)).await.expect("carregar site");

        assert_eq!(global_carregado.nodes.len(), 1);
        assert_eq!(global_carregado.nodes[0].device_id, 1);
        assert_eq!(site_carregado.nodes.len(), 1);
        assert_eq!(site_carregado.nodes[0].device_id, 2);
    }

    #[tokio::test]
    async fn payload_muito_grande_e_recusado() {
        let db = banco().await;
        let layout = TopologyLayout {
            nodes: (0..10_001)
                .map(|i| TopologyLayoutNode {
                    device_id: i as i64,
                    x: 0.0,
                    y: 0.0,
                })
                .collect(),
        };
        let erro = save_layout(&db, None, layout)
            .await
            .expect_err("devia recusar");
        assert!(format!("{erro:?}").contains("limite"));
    }

    fn no(device_id: i64, x: f64) -> TopologyLayoutNode {
        TopologyLayoutNode { device_id, x, y: x }
    }

    #[test]
    fn fusao_move_existentes_acrescenta_novos_e_preserva_o_resto() {
        let gravado = TopologyLayout {
            nodes: vec![no(1, 1.0), no(2, 2.0), no(3, 3.0)],
        };
        let fundido = merge_layout(gravado, vec![no(4, 40.0), no(2, 20.0), no(4, 44.0)]);
        let ids: Vec<_> = fundido.nodes.iter().map(|n| n.device_id).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);
        assert!((fundido.nodes[1].x - 20.0).abs() < f64::EPSILON);
        assert!((fundido.nodes[0].x - 1.0).abs() < f64::EPSILON);
        // Repetido no lote: vale a última posição.
        assert!((fundido.nodes[3].x - 44.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn patch_grava_so_o_que_mudou() {
        let db = banco().await;
        save_layout(
            &db,
            Some(5),
            TopologyLayout {
                nodes: vec![no(1, 1.0), no(2, 2.0)],
            },
        )
        .await
        .expect("gravar");

        patch_layout(&db, Some(5), vec![no(2, 9.0)])
            .await
            .expect("patch");
        let carregado = load_layout(&db, Some(5)).await.expect("carregar");
        assert_eq!(carregado.nodes.len(), 2);
        assert!((carregado.nodes[0].x - 1.0).abs() < f64::EPSILON);
        assert!((carregado.nodes[1].x - 9.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn posicao_nao_finita_e_recusada() {
        let db = banco().await;
        let erro = patch_layout(&db, None, vec![no(1, f64::NAN)])
            .await
            .expect_err("devia recusar");
        assert!(format!("{erro:?}").contains("inválida"));
    }
}
