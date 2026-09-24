//! Operações e normalização do contrato da Docker Engine.
//!
//! Cada operação recebe a fonte dos dados crus ([`DockerEngine`]) e aplica o
//! mesmo mapeamento e a mesma redação de segredos, seja o host local ou um
//! servidor remoto atendido por agente.

use std::collections::HashMap;

use serde_json::Value;

use crate::views::docker::{
    DockerActionResponse, DockerContainerConfig, DockerContainerDetail, DockerContainerHostConfig,
    DockerContainerPort, DockerContainerState, DockerContainerSummary, DockerImageDetail,
    DockerImageSummary, DockerIpamConfig, DockerLogEntry, DockerMount, DockerNetworkContainer,
    DockerNetworkDetail, DockerNetworkEndpoint, DockerNetworkSummary, DockerPruneResponse,
    DockerRestartPolicy, DockerStatusResponse, DockerVolumeDetail, DockerVolumeSummary,
};

use super::{
    source::{DockerEngine, LogChunk},
    DockerError, DISABLED_REASON, UNAVAILABLE_REASON,
};

const MAX_LOG_LINES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Remove { force: bool },
}

impl ContainerAction {
    fn success_message(self) -> &'static str {
        match self {
            Self::Start => "Container iniciado com sucesso.",
            Self::Stop => "Container parado com sucesso.",
            Self::Restart => "Container reiniciado com sucesso.",
            Self::Remove { .. } => "Container removido com sucesso.",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilters {
    pub tail: String,
    pub since: i64,
    pub until: i64,
    pub timestamps: bool,
}

pub async fn status(engine: &dyn DockerEngine) -> DockerStatusResponse {
    let raw = match engine.status_raw().await {
        Ok(raw) => raw,
        Err(DockerError::Disabled) => return unavailable_status(DISABLED_REASON),
        Err(_) => return unavailable_status(UNAVAILABLE_REASON),
    };
    let (version, info) = (&raw.version, &raw.info);

    DockerStatusResponse {
        available: true,
        reason: None,
        engine_version: optional_string(version, &["Version", "version"]),
        api_version: optional_string(version, &["ApiVersion", "apiVersion"]),
        name: optional_string(info, &["Name", "name"]),
        operating_system: optional_string(info, &["OperatingSystem", "operatingSystem"]),
        architecture: optional_string(info, &["Architecture", "architecture"]),
        cpus: optional_i64(info, &["NCPU", "nCpu"]),
        memory_total_bytes: optional_i64(info, &["MemTotal", "memTotal"]),
        containers: optional_i64(info, &["Containers", "containers"]),
        containers_running: optional_i64(info, &["ContainersRunning", "containersRunning"]),
        containers_stopped: optional_i64(info, &["ContainersStopped", "containersStopped"]),
        images: optional_i64(info, &["Images", "images"]),
    }
}

fn unavailable_status(reason: &str) -> DockerStatusResponse {
    DockerStatusResponse {
        available: false,
        reason: Some(reason.to_string()),
        engine_version: None,
        api_version: None,
        name: None,
        operating_system: None,
        architecture: None,
        cpus: None,
        memory_total_bytes: None,
        containers: None,
        containers_running: None,
        containers_stopped: None,
        images: None,
    }
}

pub async fn list_containers(
    engine: &dyn DockerEngine,
) -> Result<Vec<DockerContainerSummary>, DockerError> {
    let mut output = engine
        .list_containers_raw()
        .await?
        .iter()
        .map(container_summary)
        .collect::<Vec<_>>();
    output.sort_by_key(container_name);
    Ok(output)
}

pub async fn inspect_container(
    engine: &dyn DockerEngine,
    id: &str,
) -> Result<DockerContainerDetail, DockerError> {
    Ok(container_detail(&engine.inspect_container_raw(id).await?))
}

pub async fn container_action(
    engine: &dyn DockerEngine,
    id: &str,
    action: ContainerAction,
) -> Result<DockerActionResponse, DockerError> {
    engine.container_action(id, action).await?;
    Ok(DockerActionResponse {
        success: true,
        message: action.success_message().to_string(),
    })
}

pub async fn container_logs(
    engine: &dyn DockerEngine,
    id: &str,
    filters: LogFilters,
) -> Result<Vec<DockerLogEntry>, DockerError> {
    let chunks = engine
        .container_logs_raw(id, &filters, MAX_LOG_LINES)
        .await?;
    Ok(log_entries(&chunks, filters.timestamps))
}

pub(crate) fn log_entries(chunks: &[LogChunk], timestamps: bool) -> Vec<DockerLogEntry> {
    let mut entries = Vec::new();
    for chunk in chunks {
        for raw_line in chunk.text.lines() {
            let (timestamp, message) = split_timestamp(raw_line, timestamps);
            if !message.is_empty() {
                entries.push(DockerLogEntry {
                    timestamp,
                    stream: chunk.stream.clone(),
                    message,
                });
            }
            if entries.len() >= MAX_LOG_LINES {
                return entries;
            }
        }
    }
    entries
}

pub async fn list_volumes(
    engine: &dyn DockerEngine,
) -> Result<Vec<DockerVolumeSummary>, DockerError> {
    let raw = engine.list_volumes_raw().await?;
    let mut volumes: Vec<DockerVolumeSummary> = field(&raw, &["Volumes", "volumes"])
        .and_then(Value::as_array)
        .map(|items| items.iter().map(volume_summary).collect())
        .unwrap_or_default();
    volumes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(volumes)
}

pub async fn inspect_volume(
    engine: &dyn DockerEngine,
    name: &str,
) -> Result<DockerVolumeDetail, DockerError> {
    let raw = engine.inspect_volume_raw(name).await?;
    let summary = volume_summary(&raw);
    Ok(DockerVolumeDetail {
        name: summary.name,
        driver: summary.driver,
        mountpoint: summary.mountpoint,
        labels: summary.labels,
        scope: summary.scope,
        created_at: summary.created_at,
        options: string_map(field(&raw, &["Options", "options"])),
    })
}

pub async fn remove_volume(
    engine: &dyn DockerEngine,
    name: &str,
    force: bool,
) -> Result<DockerActionResponse, DockerError> {
    engine.remove_volume(name, force).await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Volume removido com sucesso.".to_string(),
    })
}

/// Redes com a contagem de containers conectados.
///
/// A listagem `GET /networks` da Engine devolve `Containers` vazio — só a
/// inspeção de cada rede o preenche. Em vez de uma inspeção por rede, a conta
/// sai da lista de containers, que já traz as redes de cada um.
pub async fn list_networks(
    engine: &dyn DockerEngine,
) -> Result<Vec<DockerNetworkSummary>, DockerError> {
    let (networks, containers) =
        tokio::try_join!(engine.list_networks_raw(), engine.list_containers_raw())?;
    let attached = containers_per_network(&containers);
    let mut output = networks
        .iter()
        .map(network_summary)
        .map(|mut network| {
            let counted = attached
                .get(&network.id)
                .or_else(|| attached.get(&network.name))
                .copied()
                .unwrap_or(0);
            network.connected_containers = network.connected_containers.max(counted);
            network
        })
        .collect::<Vec<_>>();
    output.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(output)
}

/// Containers por rede, contados pelo `NetworkSettings.Networks` de cada
/// container. A chave é o id da rede e, quando a Engine não o informa, o nome.
fn containers_per_network(containers: &[Value]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for container in containers {
        let Some(networks) = field(container, &["NetworkSettings", "networkSettings"])
            .and_then(|settings| field(settings, &["Networks", "networks"]))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (name, network) in networks {
            let id = string(network, &["NetworkID", "networkId", "networkID"]);
            let key = if id.is_empty() { name.clone() } else { id };
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    counts
}

pub async fn inspect_network(
    engine: &dyn DockerEngine,
    id: &str,
) -> Result<DockerNetworkDetail, DockerError> {
    let raw = engine.inspect_network_raw(id).await?;
    let summary = network_summary(&raw);
    let containers = field(&raw, &["Containers", "containers"])
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .map(|(id, item)| DockerNetworkContainer {
                    container_id: id.clone(),
                    name: string(item, &["Name", "name"]),
                    mac_address: string(item, &["MacAddress", "macAddress"]),
                    ipv4_address: string(item, &["IPv4Address", "ipv4Address"]),
                    ipv6_address: string(item, &["IPv6Address", "ipv6Address"]),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(DockerNetworkDetail {
        id: summary.id,
        name: summary.name,
        driver: summary.driver,
        scope: summary.scope,
        ipam_driver: summary.ipam_driver,
        ipam_config: summary.ipam_config,
        internal: summary.internal,
        connected_containers: summary.connected_containers,
        labels: summary.labels,
        created: summary.created,
        containers,
        options: string_map(field(&raw, &["Options", "options"])),
    })
}

pub async fn create_network(
    engine: &dyn DockerEngine,
    name: String,
    driver: String,
) -> Result<DockerActionResponse, DockerError> {
    engine.create_network(&name, &driver).await?;
    Ok(DockerActionResponse {
        success: true,
        message: format!("Rede \"{name}\" criada com sucesso."),
    })
}

pub async fn remove_network(
    engine: &dyn DockerEngine,
    id: &str,
) -> Result<DockerActionResponse, DockerError> {
    engine.remove_network(id).await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Rede removida com sucesso.".to_string(),
    })
}

pub async fn connect_network(
    engine: &dyn DockerEngine,
    network: &str,
    container: String,
) -> Result<DockerActionResponse, DockerError> {
    engine.connect_network(network, &container).await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Container conectado à rede com sucesso.".to_string(),
    })
}

pub async fn disconnect_network(
    engine: &dyn DockerEngine,
    network: &str,
    container: String,
    force: bool,
) -> Result<DockerActionResponse, DockerError> {
    let inspected = engine.inspect_container_raw(&container).await?;
    if attached_network_count(&inspected) <= 1 {
        return Err(DockerError::Validation(
            "O container precisa permanecer conectado a pelo menos uma rede".to_string(),
        ));
    }
    engine
        .disconnect_network(network, &container, force)
        .await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Container desconectado da rede com sucesso.".to_string(),
    })
}

pub async fn list_images(
    engine: &dyn DockerEngine,
) -> Result<Vec<DockerImageSummary>, DockerError> {
    let mut output = engine
        .list_images_raw()
        .await?
        .iter()
        .map(image_summary)
        .collect::<Vec<_>>();
    output.sort_by_key(|image| std::cmp::Reverse(image.created));
    Ok(output)
}

pub async fn inspect_image(
    engine: &dyn DockerEngine,
    id: &str,
) -> Result<DockerImageDetail, DockerError> {
    let raw = engine.inspect_image_raw(id).await?;
    let config = field(&raw, &["Config", "config"]).unwrap_or(&Value::Null);
    let root_fs = field(&raw, &["RootFS", "rootFs"]).unwrap_or(&Value::Null);
    Ok(DockerImageDetail {
        id: string(&raw, &["Id", "id"]),
        repo_tags: string_vec(field(&raw, &["RepoTags", "repoTags"])),
        created: string(&raw, &["Created", "created"]),
        size: i64_value(&raw, &["Size", "size"]),
        environment: redact_environment(string_vec(field(config, &["Env", "env"]))),
        command: string_vec(field(config, &["Cmd", "cmd"])),
        entrypoint: string_vec(field(config, &["Entrypoint", "entrypoint"])),
        labels: string_map(field(config, &["Labels", "labels"])),
        working_dir: string(config, &["WorkingDir", "workingDir"]),
        user: string(config, &["User", "user"]),
        root_fs_type: string(root_fs, &["Type", "type"]),
        layers: string_vec(field(root_fs, &["Layers", "layers"])),
    })
}

pub async fn remove_image(
    engine: &dyn DockerEngine,
    id: &str,
    force: bool,
) -> Result<DockerActionResponse, DockerError> {
    engine.remove_image(id, force).await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Imagem removida com sucesso.".to_string(),
    })
}

pub async fn prune_images(engine: &dyn DockerEngine) -> Result<DockerPruneResponse, DockerError> {
    let raw = engine.prune_images_raw().await?;
    Ok(DockerPruneResponse {
        images_deleted: field(&raw, &["ImagesDeleted", "imagesDeleted"])
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        space_reclaimed: field(&raw, &["SpaceReclaimed", "spaceReclaimed"])
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    })
}

fn container_summary(raw: &Value) -> DockerContainerSummary {
    let labels = string_map(field(raw, &["Labels", "labels"]));
    DockerContainerSummary {
        id: string(raw, &["Id", "id"]),
        names: string_vec(field(raw, &["Names", "names"])),
        image: string(raw, &["Image", "image"]),
        image_id: string(raw, &["ImageID", "imageId"]),
        state: string(raw, &["State", "state"]),
        status: string(raw, &["Status", "status"]),
        project_name: project_name(&labels),
        labels,
        ports: field(raw, &["Ports", "ports"])
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|port| DockerContainerPort {
                        ip: optional_string(port, &["IP", "ip"]),
                        private_port: u16_value(port, &["PrivatePort", "privatePort"]),
                        public_port: optional_u16(port, &["PublicPort", "publicPort"]),
                        protocol: string(port, &["Type", "typ", "type"]),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        created: i64_value(raw, &["Created", "created"]),
    }
}

fn container_detail(raw: &Value) -> DockerContainerDetail {
    let state = field(raw, &["State", "state"]).unwrap_or(&Value::Null);
    let config = field(raw, &["Config", "config"]).unwrap_or(&Value::Null);
    let host = field(raw, &["HostConfig", "hostConfig"]).unwrap_or(&Value::Null);
    let restart = field(host, &["RestartPolicy", "restartPolicy"]).unwrap_or(&Value::Null);
    let networks = field(raw, &["NetworkSettings", "networkSettings"])
        .and_then(|settings| field(settings, &["Networks", "networks"]))
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .map(|(name, endpoint)| DockerNetworkEndpoint {
                    network_id: string(endpoint, &["NetworkID", "networkId"]),
                    network_name: name.clone(),
                    ip_address: string(endpoint, &["IPAddress", "ipAddress"]),
                    gateway: string(endpoint, &["Gateway", "gateway"]),
                    aliases: string_vec(field(endpoint, &["Aliases", "aliases"])),
                })
                .collect()
        })
        .unwrap_or_default();
    let mounts = field(raw, &["Mounts", "mounts"])
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|mount| DockerMount {
                    mount_type: string(mount, &["Type", "typ", "type"]),
                    name: optional_string(mount, &["Name", "name"]),
                    source: string(mount, &["Source", "source"]),
                    destination: string(mount, &["Destination", "destination"]),
                    mode: string(mount, &["Mode", "mode"]),
                    read_write: bool_value(mount, &["RW", "rw"]),
                })
                .collect()
        })
        .unwrap_or_default();
    DockerContainerDetail {
        id: string(raw, &["Id", "id"]),
        name: string(raw, &["Name", "name"])
            .trim_start_matches('/')
            .to_string(),
        image: string(config, &["Image", "image"]),
        image_id: string(raw, &["Image", "imageId"]),
        created: string(raw, &["Created", "created"]),
        state: DockerContainerState {
            status: string(state, &["Status", "status"]),
            running: bool_value(state, &["Running", "running"]),
            paused: bool_value(state, &["Paused", "paused"]),
            restarting: bool_value(state, &["Restarting", "restarting"]),
            pid: i64_value(state, &["Pid", "pid"]),
            started_at: string(state, &["StartedAt", "startedAt"]),
            finished_at: string(state, &["FinishedAt", "finishedAt"]),
            exit_code: i64_value(state, &["ExitCode", "exitCode"]),
        },
        config: DockerContainerConfig {
            hostname: string(config, &["Hostname", "hostname"]),
            environment: redact_environment(string_vec(field(config, &["Env", "env"]))),
            command: string_vec(field(config, &["Cmd", "cmd"])),
            entrypoint: string_vec(field(config, &["Entrypoint", "entrypoint"])),
            labels: string_map(field(config, &["Labels", "labels"])),
            working_dir: string(config, &["WorkingDir", "workingDir"]),
            user: string(config, &["User", "user"]),
        },
        host_config: DockerContainerHostConfig {
            restart_policy: DockerRestartPolicy {
                name: string(restart, &["Name", "name"]),
                maximum_retry_count: i64_value(
                    restart,
                    &["MaximumRetryCount", "maximumRetryCount"],
                ),
            },
            network_mode: string(host, &["NetworkMode", "networkMode"]),
        },
        mounts,
        networks,
    }
}

fn attached_network_count(raw: &Value) -> usize {
    field(raw, &["NetworkSettings", "networkSettings"])
        .and_then(|settings| field(settings, &["Networks", "networks"]))
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len)
}

fn volume_summary(raw: &Value) -> DockerVolumeSummary {
    DockerVolumeSummary {
        name: string(raw, &["Name", "name"]),
        driver: string(raw, &["Driver", "driver"]),
        mountpoint: string(raw, &["Mountpoint", "mountpoint"]),
        labels: string_map(field(raw, &["Labels", "labels"])),
        scope: string(raw, &["Scope", "scope"]),
        created_at: optional_string(raw, &["CreatedAt", "createdAt"]),
    }
}

fn network_summary(raw: &Value) -> DockerNetworkSummary {
    let ipam = field(raw, &["IPAM", "ipam"]).unwrap_or(&Value::Null);
    let connected_containers = field(raw, &["Containers", "containers"])
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    DockerNetworkSummary {
        id: string(raw, &["Id", "id"]),
        name: string(raw, &["Name", "name"]),
        driver: string(raw, &["Driver", "driver"]),
        scope: string(raw, &["Scope", "scope"]),
        ipam_driver: string(ipam, &["Driver", "driver"]),
        ipam_config: field(ipam, &["Config", "config"])
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| DockerIpamConfig {
                        subnet: optional_string(item, &["Subnet", "subnet"]),
                        gateway: optional_string(item, &["Gateway", "gateway"]),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        internal: bool_value(raw, &["Internal", "internal"]),
        connected_containers,
        labels: string_map(field(raw, &["Labels", "labels"])),
        created: string(raw, &["Created", "created"]),
    }
}

fn image_summary(raw: &Value) -> DockerImageSummary {
    DockerImageSummary {
        id: string(raw, &["Id", "id"]),
        parent_id: string(raw, &["ParentId", "parentId"]),
        repo_tags: string_vec(field(raw, &["RepoTags", "repoTags"])),
        repo_digests: string_vec(field(raw, &["RepoDigests", "repoDigests"])),
        created: i64_value(raw, &["Created", "created"]),
        size: i64_value(raw, &["Size", "size"]),
        shared_size: i64_value(raw, &["SharedSize", "sharedSize"]),
        labels: string_map(field(raw, &["Labels", "labels"])),
        containers: i64_value(raw, &["Containers", "containers"]),
    }
}

fn field<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|name| value.get(*name))
}

fn string(value: &Value, names: &[&str]) -> String {
    optional_string(value, names).unwrap_or_default()
}

fn optional_string(value: &Value, names: &[&str]) -> Option<String> {
    field(value, names)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn i64_value(value: &Value, names: &[&str]) -> i64 {
    optional_i64(value, names).unwrap_or_default()
}

fn optional_i64(value: &Value, names: &[&str]) -> Option<i64> {
    field(value, names).and_then(Value::as_i64)
}

fn u16_value(value: &Value, names: &[&str]) -> u16 {
    optional_u16(value, names).unwrap_or_default()
}

fn optional_u16(value: &Value, names: &[&str]) -> Option<u16> {
    field(value, names)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
}

fn bool_value(value: &Value, names: &[&str]) -> bool {
    field(value, names)
        .and_then(Value::as_bool)
        .unwrap_or_default()
}

fn string_vec(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| {
                        let value = if is_sensitive_name(key) {
                            "********".to_string()
                        } else {
                            value.to_string()
                        };
                        (key.clone(), value)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn project_name(labels: &HashMap<String, String>) -> Option<String> {
    [
        "com.docker.compose.project",
        "io.podman.compose.project",
        "project.name",
    ]
    .iter()
    .find_map(|key| labels.get(*key))
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
}

fn container_name(container: &DockerContainerSummary) -> String {
    container
        .names
        .first()
        .map(|name| name.trim_start_matches('/').to_ascii_lowercase())
        .unwrap_or_else(|| container.id.to_ascii_lowercase())
}

fn split_timestamp(line: &str, enabled: bool) -> (String, String) {
    if enabled {
        if let Some((timestamp, message)) = line.split_once(' ') {
            return (timestamp.to_string(), message.trim_end().to_string());
        }
    }
    (String::new(), line.trim_end().to_string())
}

fn redact_environment(entries: Vec<String>) -> Vec<String> {
    entries
        .into_iter()
        .map(|entry| {
            let Some((name, value)) = entry.split_once('=') else {
                return entry;
            };
            if is_sensitive_name(name) || contains_url_credentials(value) {
                format!("{name}=********")
            } else {
                entry
            }
        })
        .collect()
}

fn is_sensitive_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "PASSWORD",
        "PASSWD",
        "PASS",
        "TOKEN",
        "SECRET",
        "PRIVATE",
        "CREDENTIAL",
        "API_KEY",
        "ACCESS_KEY",
        "DATABASE_URL",
        "DB_URL",
        "DSN",
        "AUTH",
        "COOKIE",
        "SESSION",
        "CONNECTION_STRING",
        "SIGNING_KEY",
        "ENCRYPTION_KEY",
    ]
    .iter()
    .any(|marker| upper.contains(marker))
}

fn contains_url_credentials(value: &str) -> bool {
    value.split_once("://").is_some_and(|(_, remainder)| {
        remainder
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|authority| {
                authority
                    .split_once('@')
                    .is_some_and(|(userinfo, _)| userinfo.contains(':'))
            })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use serial_test::serial;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn status_explica_quando_a_integracao_foi_desativada() {
        let previous = std::env::var_os("DOCKER_ENABLED");
        std::env::set_var("DOCKER_ENABLED", "false");
        let response = status(&crate::services::docker::source::LocalEngine).await;
        match previous {
            Some(value) => std::env::set_var("DOCKER_ENABLED", value),
            None => std::env::remove_var("DOCKER_ENABLED"),
        }

        assert!(!response.available);
        assert_eq!(response.reason.as_deref(), Some(DISABLED_REASON));
    }

    #[test]
    fn normaliza_container_e_projeto_compose() {
        let value = container_summary(&json!({
            "Id": "abc",
            "Names": ["/web"],
            "Image": "nginx:latest",
            "ImageID": "sha256:def",
            "State": "running",
            "Status": "Up 1 minute",
            "Labels": { "com.docker.compose.project": "portal" },
            "Ports": [{ "PrivatePort": 80, "PublicPort": 8080, "Type": "tcp" }],
            "Created": 10
        }));
        assert_eq!(value.project_name.as_deref(), Some("portal"));
        assert_eq!(value.ports[0].public_port, Some(8080));
    }

    #[test]
    fn oculta_segredos_do_ambiente_sem_apagar_campos_comuns() {
        let values = redact_environment(vec![
            "POSTGRES_PASSWORD=segredo".to_string(),
            "APP_PORT=3333".to_string(),
            "API_KEY=valor".to_string(),
            "DATABASE_URL=postgres://user:senha@db/app".to_string(),
            "REDIS_URL=redis://:senha@cache/0".to_string(),
        ]);
        assert_eq!(values[0], "POSTGRES_PASSWORD=********");
        assert_eq!(values[1], "APP_PORT=3333");
        assert_eq!(values[2], "API_KEY=********");
        assert_eq!(values[3], "DATABASE_URL=********");
        assert_eq!(values[4], "REDIS_URL=********");
    }

    #[test]
    fn oculta_valores_sensiveis_de_labels_e_opcoes() {
        let values = string_map(Some(&json!({
            "com.docker.compose.project": "portal",
            "traefik.http.middlewares.admin.basicauth.users": "admin:hash",
            "database_password": "segredo"
        })));
        assert_eq!(values["com.docker.compose.project"], "portal");
        assert_eq!(
            values["traefik.http.middlewares.admin.basicauth.users"],
            "********"
        );
        assert_eq!(values["database_password"], "********");
    }

    #[test]
    fn conta_redes_anexadas_antes_de_permitir_desconexao() {
        assert_eq!(
            attached_network_count(&json!({
                "NetworkSettings": {
                    "Networks": { "frontend": {}, "backend": {} }
                }
            })),
            2
        );
        assert_eq!(
            attached_network_count(&json!({
                "NetworkSettings": { "Networks": { "default": {} } }
            })),
            1
        );
    }

    #[test]
    fn conta_containers_por_rede_pela_lista_de_containers() {
        let containers = vec![
            json!({ "NetworkSettings": { "Networks": {
                "app_net": { "NetworkID": "n1" },
                "bridge": { "NetworkID": "n0" }
            } } }),
            json!({ "NetworkSettings": { "Networks": { "app_net": { "NetworkID": "n1" } } } }),
            json!({ "NetworkSettings": { "Networks": { "legado": {} } } }),
            json!({ "Names": ["/sem-rede"] }),
        ];
        let contagem = containers_per_network(&containers);
        assert_eq!(contagem.get("n1"), Some(&2));
        assert_eq!(contagem.get("n0"), Some(&1));
        assert_eq!(contagem.get("legado"), Some(&1), "sem id, conta pelo nome");
        assert_eq!(contagem.len(), 3);
    }

    /// Fonte falsa: prova que mapeamento e regras não dependem do `bollard`,
    /// que é o que permite a mesma camada servir o host de um agente remoto.
    #[derive(Default)]
    struct FakeEngine {
        status: Option<DockerError>,
        containers: Vec<Value>,
        inspected: Value,
        disconnected: std::sync::Mutex<bool>,
    }

    #[async_trait::async_trait]
    impl DockerEngine for FakeEngine {
        async fn status_raw(&self) -> Result<super::super::source::EngineStatusRaw, DockerError> {
            match &self.status {
                Some(DockerError::Disabled) => Err(DockerError::Disabled),
                Some(_) => Err(DockerError::Unavailable),
                None => Ok(super::super::source::EngineStatusRaw {
                    version: json!({ "Version": "27.0.1" }),
                    info: json!({ "NCPU": 4, "Name": "srv-remoto" }),
                }),
            }
        }
        async fn list_containers_raw(&self) -> Result<Vec<Value>, DockerError> {
            Ok(self.containers.clone())
        }
        async fn inspect_container_raw(&self, _: &str) -> Result<Value, DockerError> {
            Ok(self.inspected.clone())
        }
        async fn container_action(&self, _: &str, _: ContainerAction) -> Result<(), DockerError> {
            Ok(())
        }
        async fn container_logs_raw(
            &self,
            _: &str,
            _: &LogFilters,
            _: usize,
        ) -> Result<Vec<LogChunk>, DockerError> {
            Ok(Vec::new())
        }
        async fn list_volumes_raw(&self) -> Result<Value, DockerError> {
            Ok(json!({ "Volumes": [] }))
        }
        async fn inspect_volume_raw(&self, _: &str) -> Result<Value, DockerError> {
            Err(DockerError::NotFound)
        }
        async fn remove_volume(&self, _: &str, _: bool) -> Result<(), DockerError> {
            Ok(())
        }
        async fn list_networks_raw(&self) -> Result<Vec<Value>, DockerError> {
            Ok(Vec::new())
        }
        async fn inspect_network_raw(&self, _: &str) -> Result<Value, DockerError> {
            Err(DockerError::NotFound)
        }
        async fn create_network(&self, _: &str, _: &str) -> Result<(), DockerError> {
            Ok(())
        }
        async fn remove_network(&self, _: &str) -> Result<(), DockerError> {
            Ok(())
        }
        async fn connect_network(&self, _: &str, _: &str) -> Result<(), DockerError> {
            Ok(())
        }
        async fn disconnect_network(&self, _: &str, _: &str, _: bool) -> Result<(), DockerError> {
            *self.disconnected.lock().expect("mutex") = true;
            Ok(())
        }
        async fn list_images_raw(&self) -> Result<Vec<Value>, DockerError> {
            Ok(Vec::new())
        }
        async fn inspect_image_raw(&self, _: &str) -> Result<Value, DockerError> {
            Err(DockerError::NotFound)
        }
        async fn remove_image(&self, _: &str, _: bool) -> Result<(), DockerError> {
            Ok(())
        }
        async fn prune_images_raw(&self) -> Result<Value, DockerError> {
            Ok(json!({ "ImagesDeleted": [{}, {}], "SpaceReclaimed": 42 }))
        }
        async fn metrics(&self) -> crate::views::docker::DockerMetricsResponse {
            crate::services::docker::metrics::unavailable("fonte falsa")
        }
        async fn follow_logs(
            &self,
            _: &str,
            _: &str,
            _: tokio::sync::mpsc::UnboundedSender<LogChunk>,
            _: tokio_util::sync::CancellationToken,
        ) -> Result<(), DockerError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn status_de_fonte_qualquer_segue_o_mesmo_mapeamento() {
        let ok = status(&FakeEngine::default()).await;
        assert!(ok.available);
        assert_eq!(ok.engine_version.as_deref(), Some("27.0.1"));
        assert_eq!(ok.cpus, Some(4));

        let disabled = status(&FakeEngine {
            status: Some(DockerError::Disabled),
            ..Default::default()
        })
        .await;
        assert_eq!(disabled.reason.as_deref(), Some(DISABLED_REASON));

        let down = status(&FakeEngine {
            status: Some(DockerError::Unavailable),
            ..Default::default()
        })
        .await;
        assert_eq!(down.reason.as_deref(), Some(UNAVAILABLE_REASON));
    }

    #[tokio::test]
    async fn lista_de_fonte_qualquer_e_ordenada_e_normalizada() {
        let engine = FakeEngine {
            containers: vec![
                json!({ "Id": "2", "Names": ["/zeta"], "Labels": {} }),
                json!({ "Id": "1", "Names": ["/Alfa"], "Labels": { "com.docker.compose.project": "app" } }),
            ],
            ..Default::default()
        };
        let items = list_containers(&engine).await.expect("lista");
        assert_eq!(items[0].names, vec!["/Alfa"]);
        assert_eq!(items[0].project_name.as_deref(), Some("app"));
        assert_eq!(items[1].id, "2");

        let pruned = prune_images(&engine).await.expect("prune");
        assert_eq!(pruned.images_deleted, 2);
        assert_eq!(pruned.space_reclaimed, 42);
    }

    #[tokio::test]
    async fn desconexao_da_ultima_rede_e_barrada_antes_de_chegar_a_fonte() {
        let engine = FakeEngine {
            inspected: json!({ "NetworkSettings": { "Networks": { "default": {} } } }),
            ..Default::default()
        };
        let error = disconnect_network(&engine, "default", "web".into(), false)
            .await
            .expect_err("última rede");
        assert!(matches!(error, DockerError::Validation(_)));
        assert!(!*engine.disconnected.lock().expect("mutex"));
    }

    #[test]
    fn blocos_de_log_viram_linhas_com_fluxo_e_timestamp() {
        let chunks = [
            LogChunk {
                stream: "stdout".into(),
                text: "2024-01-01T00:00:00Z iniciado

"
                .into(),
            },
            LogChunk {
                stream: "stderr".into(),
                text: "2024-01-01T00:00:01Z falhou
"
                .into(),
            },
        ];
        let entries = log_entries(&chunks, true);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].timestamp, "2024-01-01T00:00:00Z");
        assert_eq!(entries[0].message, "iniciado");
        assert_eq!(entries[1].stream, "stderr");
    }
}
