//! Entradas da topologia: enlaces manuais, switch não gerenciado e layout.

use serde::Deserialize;
use ts_rs::TS;

/// `POST /api/topology/links` — enlace manual entre dois dispositivos.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyLinkInput {
    #[ts(type = "number")]
    pub source_device_id: i64,
    #[ts(type = "number")]
    pub target_device_id: i64,
    #[ts(optional, type = "number | null")]
    pub source_interface_id: Option<i64>,
    #[ts(optional, type = "number | null")]
    pub target_interface_id: Option<i64>,
    #[ts(optional, type = "string | null")]
    pub link_type: Option<String>,
}

/// `PUT /api/topology/links/{id}` — portas e tipo de um enlace existente.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyLinkUpdateInput {
    #[ts(optional, type = "number | null")]
    pub source_interface_id: Option<i64>,
    #[ts(optional, type = "number | null")]
    pub target_interface_id: Option<i64>,
    #[ts(optional, type = "string | null")]
    pub link_type: Option<String>,
}

/// `POST /api/topology/unmanaged-switch`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct UnmanagedSwitchInput {
    pub name: String,
    #[ts(optional, type = "string | null")]
    pub vendor: Option<String>,
    #[ts(optional, type = "string | null")]
    pub model: Option<String>,
    #[serde(default = "default_unmanaged_port_count")]
    #[ts(as = "Option<u32>", optional)]
    pub port_count: u32,
    #[ts(optional, type = "number | null")]
    pub site_id: Option<i64>,
    #[ts(optional, type = "number | null")]
    pub network_id: Option<i64>,
}

fn default_unmanaged_port_count() -> u32 {
    8
}

/// Posição de um nó no mapa.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyLayoutNodeInput {
    #[ts(type = "number")]
    pub device_id: i64,
    pub x: f64,
    pub y: f64,
}

/// `PUT /api/topology/layout` (layout inteiro) e `PATCH` (só os nós movidos).
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyLayoutInput {
    pub nodes: Vec<TopologyLayoutNodeInput>,
}
