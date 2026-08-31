//! Contratos de saída do gerenciador Docker.

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerStatusResponse {
    pub available: bool,
    pub reason: Option<String>,
    pub engine_version: Option<String>,
    pub api_version: Option<String>,
    pub name: Option<String>,
    pub operating_system: Option<String>,
    pub architecture: Option<String>,
    #[ts(type = "number | null")]
    pub cpus: Option<i64>,
    #[ts(type = "number | null")]
    pub memory_total_bytes: Option<i64>,
    #[ts(type = "number | null")]
    pub containers: Option<i64>,
    #[ts(type = "number | null")]
    pub containers_running: Option<i64>,
    #[ts(type = "number | null")]
    pub containers_stopped: Option<i64>,
    #[ts(type = "number | null")]
    pub images: Option<i64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerContainerPort {
    pub ip: Option<String>,
    pub private_port: u16,
    pub public_port: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerContainerSummary {
    pub id: String,
    pub names: Vec<String>,
    pub image: String,
    pub image_id: String,
    pub state: String,
    pub status: String,
    pub labels: HashMap<String, String>,
    pub ports: Vec<DockerContainerPort>,
    #[ts(type = "number")]
    pub created: i64,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerContainerState {
    pub status: String,
    pub running: bool,
    pub paused: bool,
    pub restarting: bool,
    #[ts(type = "number")]
    pub pid: i64,
    pub started_at: String,
    pub finished_at: String,
    #[ts(type = "number")]
    pub exit_code: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerContainerConfig {
    pub hostname: String,
    pub environment: Vec<String>,
    pub command: Vec<String>,
    pub entrypoint: Vec<String>,
    pub labels: HashMap<String, String>,
    pub working_dir: String,
    pub user: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerRestartPolicy {
    pub name: String,
    #[ts(type = "number")]
    pub maximum_retry_count: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerContainerHostConfig {
    pub restart_policy: DockerRestartPolicy,
    pub network_mode: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerMount {
    pub mount_type: String,
    pub name: Option<String>,
    pub source: String,
    pub destination: String,
    pub mode: String,
    pub read_write: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerNetworkEndpoint {
    pub network_id: String,
    pub network_name: String,
    pub ip_address: String,
    pub gateway: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerContainerDetail {
    pub id: String,
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub created: String,
    pub state: DockerContainerState,
    pub config: DockerContainerConfig,
    pub host_config: DockerContainerHostConfig,
    pub mounts: Vec<DockerMount>,
    pub networks: Vec<DockerNetworkEndpoint>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerVolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub labels: HashMap<String, String>,
    pub scope: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerVolumeDetail {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub labels: HashMap<String, String>,
    pub scope: String,
    pub created_at: Option<String>,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerIpamConfig {
    pub subnet: Option<String>,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerNetworkContainer {
    pub container_id: String,
    pub name: String,
    pub mac_address: String,
    pub ipv4_address: String,
    pub ipv6_address: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerNetworkSummary {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub ipam_driver: String,
    pub ipam_config: Vec<DockerIpamConfig>,
    pub internal: bool,
    pub connected_containers: usize,
    pub labels: HashMap<String, String>,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerNetworkDetail {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub ipam_driver: String,
    pub ipam_config: Vec<DockerIpamConfig>,
    pub internal: bool,
    pub connected_containers: usize,
    pub labels: HashMap<String, String>,
    pub created: String,
    pub containers: Vec<DockerNetworkContainer>,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerImageSummary {
    pub id: String,
    pub parent_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    #[ts(type = "number")]
    pub created: i64,
    #[ts(type = "number")]
    pub size: i64,
    #[ts(type = "number")]
    pub shared_size: i64,
    pub labels: HashMap<String, String>,
    #[ts(type = "number")]
    pub containers: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerImageDetail {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub created: String,
    #[ts(type = "number")]
    pub size: i64,
    pub environment: Vec<String>,
    pub command: Vec<String>,
    pub entrypoint: Vec<String>,
    pub labels: HashMap<String, String>,
    pub working_dir: String,
    pub user: String,
    pub root_fs_type: String,
    pub layers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerLogEntry {
    pub timestamp: String,
    pub stream: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerActionResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerPruneResponse {
    pub images_deleted: usize,
    #[ts(type = "number")]
    pub space_reclaimed: u64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerCpuMetrics {
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerMemoryMetrics {
    #[ts(type = "number")]
    pub usage_bytes: u64,
    #[ts(type = "number")]
    pub limit_bytes: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerIoMetrics {
    #[ts(type = "number")]
    pub read_bytes: u64,
    #[ts(type = "number")]
    pub write_bytes: u64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerNetworkMetrics {
    #[ts(type = "number")]
    pub received_bytes: u64,
    #[ts(type = "number")]
    pub transmitted_bytes: u64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerContainerMetrics {
    pub container_id: String,
    pub container_name: String,
    pub project_name: Option<String>,
    pub image_name: String,
    pub status: String,
    pub cpu: DockerCpuMetrics,
    pub memory: DockerMemoryMetrics,
    pub network: DockerNetworkMetrics,
    pub block_io: DockerIoMetrics,
    #[ts(type = "number | null")]
    pub pids: Option<u64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DockerMetricsResponse {
    pub docker_available: bool,
    pub unavailable_reason: Option<String>,
    pub collected_at: String,
    pub containers: Vec<DockerContainerMetrics>,
}
