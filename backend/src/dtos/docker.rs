//! Entradas HTTP do gerenciador Docker.

use serde::Deserialize;
use ts_rs::TS;

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerForceQuery {
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerLogsQuery {
    pub tail: Option<String>,
    #[ts(type = "number | null")]
    pub since: Option<i64>,
    #[ts(type = "number | null")]
    pub until: Option<i64>,
    pub timestamps: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerNetworkCreateInput {
    pub name: String,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerNetworkConnectionInput {
    pub container_id: String,
    pub force: Option<bool>,
}
