//! Operações e normalização do contrato da Docker Engine.

use std::collections::HashMap;
use std::time::Duration;

use bollard::{
    container::{ListContainersOptions, LogsOptions, RemoveContainerOptions},
    image::{ListImagesOptions, RemoveImageOptions},
    network::{
        ConnectNetworkOptions, CreateNetworkOptions, DisconnectNetworkOptions, ListNetworksOptions,
    },
    volume::{ListVolumesOptions, RemoveVolumeOptions},
};
use futures::StreamExt;
use serde::Serialize;
use serde_json::Value;

use crate::views::docker::{
    DockerActionResponse, DockerContainerConfig, DockerContainerDetail, DockerContainerHostConfig,
    DockerContainerPort, DockerContainerState, DockerContainerSummary, DockerImageDetail,
    DockerImageSummary, DockerIpamConfig, DockerLogEntry, DockerMount, DockerNetworkContainer,
    DockerNetworkDetail, DockerNetworkEndpoint, DockerNetworkSummary, DockerPruneResponse,
    DockerRestartPolicy, DockerStatusResponse, DockerVolumeDetail, DockerVolumeSummary,
};

use super::{call, client, DockerError, DISABLED_REASON, UNAVAILABLE_REASON};

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_LOG_LINES: usize = 10_000;

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Default)]
pub struct LogFilters {
    pub tail: String,
    pub since: i64,
    pub until: i64,
    pub timestamps: bool,
}

pub async fn status() -> DockerStatusResponse {
    let client = match client() {
        Ok(client) => client,
        Err(DockerError::Disabled) => return unavailable_status(DISABLED_REASON),
        Err(_) => return unavailable_status(UNAVAILABLE_REASON),
    };
    if !matches!(
        tokio::time::timeout(STATUS_TIMEOUT, client.ping()).await,
        Ok(Ok(_))
    ) {
        return unavailable_status(UNAVAILABLE_REASON);
    }

    let version = tokio::time::timeout(STATUS_TIMEOUT, client.version())
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(|value| serde_json::to_value(value).ok())
        .unwrap_or(Value::Null);
    let info = tokio::time::timeout(STATUS_TIMEOUT, client.info())
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(|value| serde_json::to_value(value).ok())
        .unwrap_or(Value::Null);

    DockerStatusResponse {
        available: true,
        reason: None,
        engine_version: optional_string(&version, &["Version", "version"]),
        api_version: optional_string(&version, &["ApiVersion", "apiVersion"]),
        name: optional_string(&info, &["Name", "name"]),
        operating_system: optional_string(&info, &["OperatingSystem", "operatingSystem"]),
        architecture: optional_string(&info, &["Architecture", "architecture"]),
        cpus: optional_i64(&info, &["NCPU", "nCpu"]),
        memory_total_bytes: optional_i64(&info, &["MemTotal", "memTotal"]),
        containers: optional_i64(&info, &["Containers", "containers"]),
        containers_running: optional_i64(&info, &["ContainersRunning", "containersRunning"]),
        containers_stopped: optional_i64(&info, &["ContainersStopped", "containersStopped"]),
        images: optional_i64(&info, &["Images", "images"]),
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

pub async fn list_containers() -> Result<Vec<DockerContainerSummary>, DockerError> {
    let items = call(
        client()?.list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        })),
    )
    .await?;
    let mut output = items
        .into_iter()
        .map(to_value)
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .map(container_summary)
        .collect::<Vec<_>>();
    output.sort_by_key(container_name);
    Ok(output)
}

pub async fn inspect_container(id: &str) -> Result<DockerContainerDetail, DockerError> {
    let raw = to_value(call(client()?.inspect_container(id, None)).await?)?;
    Ok(container_detail(&raw))
}

pub async fn container_action(
    id: &str,
    action: ContainerAction,
) -> Result<DockerActionResponse, DockerError> {
    let client = client()?;
    match action {
        ContainerAction::Start => call(client.start_container::<String>(id, None)).await?,
        ContainerAction::Stop => call(client.stop_container(id, None)).await?,
        ContainerAction::Restart => call(client.restart_container(id, None)).await?,
        ContainerAction::Remove { force } => {
            call(client.remove_container(
                id,
                Some(RemoveContainerOptions {
                    force,
                    ..Default::default()
                }),
            ))
            .await?;
        }
    }
    Ok(DockerActionResponse {
        success: true,
        message: action.success_message().to_string(),
    })
}

pub async fn container_logs(
    id: &str,
    filters: LogFilters,
) -> Result<Vec<DockerLogEntry>, DockerError> {
    let mut stream = client()?.logs(
        id,
        Some(LogsOptions {
            follow: false,
            stdout: true,
            stderr: true,
            since: filters.since,
            until: filters.until,
            timestamps: filters.timestamps,
            tail: filters.tail,
        }),
    );
    let mut entries = Vec::new();
    while entries.len() < MAX_LOG_LINES {
        let next = tokio::time::timeout(STATUS_TIMEOUT, stream.next())
            .await
            .map_err(|_| DockerError::Unavailable)?;
        let Some(output) = next else { break };
        let output = output.map_err(|_| DockerError::Engine)?;
        let (stream_name, bytes) = match output {
            bollard::container::LogOutput::StdOut { message } => ("stdout", message),
            bollard::container::LogOutput::StdErr { message } => ("stderr", message),
            _ => continue,
        };
        for raw_line in String::from_utf8_lossy(&bytes).lines() {
            let (timestamp, message) = split_timestamp(raw_line, filters.timestamps);
            if !message.is_empty() {
                entries.push(DockerLogEntry {
                    timestamp,
                    stream: stream_name.to_string(),
                    message,
                });
            }
            if entries.len() >= MAX_LOG_LINES {
                break;
            }
        }
    }
    Ok(entries)
}

pub async fn list_volumes() -> Result<Vec<DockerVolumeSummary>, DockerError> {
    let raw = to_value(
        call(client()?.list_volumes(Some(ListVolumesOptions::<String>::default()))).await?,
    )?;
    let mut volumes: Vec<DockerVolumeSummary> = field(&raw, &["Volumes", "volumes"])
        .and_then(Value::as_array)
        .map(|items| items.iter().map(volume_summary).collect())
        .unwrap_or_default();
    volumes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(volumes)
}

pub async fn inspect_volume(name: &str) -> Result<DockerVolumeDetail, DockerError> {
    let raw = to_value(call(client()?.inspect_volume(name)).await?)?;
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

pub async fn remove_volume(name: &str, force: bool) -> Result<DockerActionResponse, DockerError> {
    call(client()?.remove_volume(name, Some(RemoveVolumeOptions { force }))).await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Volume removido com sucesso.".to_string(),
    })
}

pub async fn list_networks() -> Result<Vec<DockerNetworkSummary>, DockerError> {
    let items =
        call(client()?.list_networks(Some(ListNetworksOptions::<String>::default()))).await?;
    let mut output = items
        .into_iter()
        .map(to_value)
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .map(network_summary)
        .collect::<Vec<_>>();
    output.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(output)
}

pub async fn inspect_network(id: &str) -> Result<DockerNetworkDetail, DockerError> {
    let raw = to_value(call(client()?.inspect_network::<String>(id, None)).await?)?;
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
    name: String,
    driver: String,
) -> Result<DockerActionResponse, DockerError> {
    call(client()?.create_network(CreateNetworkOptions {
        name: name.clone(),
        driver,
        check_duplicate: true,
        ..Default::default()
    }))
    .await?;
    Ok(DockerActionResponse {
        success: true,
        message: format!("Rede \"{name}\" criada com sucesso."),
    })
}

pub async fn remove_network(id: &str) -> Result<DockerActionResponse, DockerError> {
    call(client()?.remove_network(id)).await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Rede removida com sucesso.".to_string(),
    })
}

pub async fn connect_network(
    network: &str,
    container: String,
) -> Result<DockerActionResponse, DockerError> {
    call(client()?.connect_network(
        network,
        ConnectNetworkOptions {
            container,
            ..Default::default()
        },
    ))
    .await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Container conectado à rede com sucesso.".to_string(),
    })
}

pub async fn disconnect_network(
    network: &str,
    container: String,
    force: bool,
) -> Result<DockerActionResponse, DockerError> {
    call(client()?.disconnect_network(network, DisconnectNetworkOptions { container, force }))
        .await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Container desconectado da rede com sucesso.".to_string(),
    })
}

pub async fn list_images() -> Result<Vec<DockerImageSummary>, DockerError> {
    let items = call(client()?.list_images(Some(ListImagesOptions::<String>::default()))).await?;
    let mut output = items
        .into_iter()
        .map(to_value)
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .map(image_summary)
        .collect::<Vec<_>>();
    output.sort_by_key(|image| std::cmp::Reverse(image.created));
    Ok(output)
}

pub async fn inspect_image(id: &str) -> Result<DockerImageDetail, DockerError> {
    let raw = to_value(call(client()?.inspect_image(id)).await?)?;
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

pub async fn remove_image(id: &str, force: bool) -> Result<DockerActionResponse, DockerError> {
    call(client()?.remove_image(
        id,
        Some(RemoveImageOptions {
            force,
            ..Default::default()
        }),
        None,
    ))
    .await?;
    Ok(DockerActionResponse {
        success: true,
        message: "Imagem removida com sucesso.".to_string(),
    })
}

pub async fn prune_images() -> Result<DockerPruneResponse, DockerError> {
    let raw = to_value(call(client()?.prune_images::<String>(None)).await?)?;
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

fn to_value<T: Serialize>(value: T) -> Result<Value, DockerError> {
    serde_json::to_value(value).map_err(|_| DockerError::Engine)
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
        let response = status().await;
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
}
