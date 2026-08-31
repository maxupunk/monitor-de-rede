//! Persistência do layout do mapa de topologia.
//!
//! O layout é guardado em `system_settings` como JSON, usando uma chave por
//! `site_id` (ou uma chave global quando nenhum site é informado). Essa escolha
//! evita nova migration: o par já existe e é replicado nos backups.

use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::{
    models::system_settings,
    services::shared::errors::{AppError, AppResult},
};

const STORAGE_KEY: &str = "topology_layout";
const MAX_NODES: usize = 10_000;

fn storage_key(site_id: Option<i64>) -> String {
    match site_id {
        Some(id) => format!("{STORAGE_KEY}:{id}"),
        None => STORAGE_KEY.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyLayoutNode {
    pub device_id: i64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyLayout {
    pub nodes: Vec<TopologyLayoutNode>,
}

/// Carrega o layout gravado para o site (ou o layout global).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn load_layout(
    db: &DatabaseConnection,
    site_id: Option<i64>,
) -> AppResult<TopologyLayout> {
    let setting = system_settings::Model::get(db, &storage_key(site_id)).await?;
    let layout = setting
        .and_then(|row| row.value)
        .and_then(|value| serde_json::from_str::<TopologyLayout>(&value).ok())
        .unwrap_or_default();
    Ok(layout)
}

/// Grava o layout para o site (ou o layout global).
///
/// # Errors
///
/// Propaga erro do banco ou rejeita payloads absurdamente grandes.
pub async fn save_layout(
    db: &DatabaseConnection,
    site_id: Option<i64>,
    layout: TopologyLayout,
) -> AppResult<TopologyLayout> {
    if layout.nodes.len() > MAX_NODES {
        return Err(AppError::validation(format!(
            "Layout excede o limite de {MAX_NODES} nós"
        )));
    }

    let value = serde_json::to_string(&layout)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, &storage_key(site_id), Some(value)).await?;
    Ok(layout)
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
}
