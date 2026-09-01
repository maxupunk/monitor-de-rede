//! Limpeza real do arquivo de log de containers com driver `json-file`.

use std::path::{Component, Path};

use bollard::{
    container::{Config, CreateContainerOptions, WaitContainerOptions},
    models::{HostConfig, Mount, MountTypeEnum},
};
use futures::StreamExt;

use crate::views::docker::DockerActionResponse;

use super::{call, client, engine, volume_export, DockerError};

const LOG_MOUNT_POINT: &str = "/docker-log/container.log";
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

struct LogTarget {
    path: String,
    running: bool,
}

pub async fn clear(id: &str) -> Result<DockerActionResponse, DockerError> {
    let client = client()?;
    let inspected = call(client.inspect_container(id, None)).await?;
    let target = log_target(&inspected)?;

    // O pull não pode ampliar a indisponibilidade do container alvo.
    volume_export::ensure_image(&client).await?;

    if target.running {
        call(client.stop_container(id, None)).await?;
    }

    let clear_result = truncate_with_helper(&client, &target.path).await;
    let restore_result = if target.running {
        call(client.start_container::<String>(id, None)).await
    } else {
        Ok(())
    };

    if restore_result.is_err() {
        return Err(DockerError::Engine);
    }
    clear_result?;

    Ok(DockerActionResponse {
        success: true,
        message: if target.running {
            "Logs removidos da Docker Engine; o container foi reiniciado.".to_string()
        } else {
            "Logs removidos da Docker Engine.".to_string()
        },
    })
}

fn log_target(
    inspected: &bollard::models::ContainerInspectResponse,
) -> Result<LogTarget, DockerError> {
    let driver = inspected
        .host_config
        .as_ref()
        .and_then(|config| config.log_config.as_ref())
        .and_then(|config| config.typ.as_deref())
        .unwrap_or_default();
    if driver != "json-file" {
        return Err(DockerError::Validation(format!(
            "A limpeza real de logs exige o driver json-file; este container usa {driver}"
        )));
    }
    if inspected
        .state
        .as_ref()
        .and_then(|state| state.paused)
        .unwrap_or(false)
    {
        return Err(DockerError::Validation(
            "Despause o container antes de limpar os logs".to_string(),
        ));
    }
    if inspected
        .host_config
        .as_ref()
        .and_then(|config| config.auto_remove)
        .unwrap_or(false)
    {
        return Err(DockerError::Validation(
            "Containers com remoção automática não podem ser reiniciados para limpar os logs"
                .to_string(),
        ));
    }

    let id = inspected.id.as_deref().ok_or(DockerError::Engine)?;
    if is_current_container(id) {
        return Err(DockerError::Validation(
            "Os logs do container que executa o NetMonitor não podem ser apagados por esta interface"
                .to_string(),
        ));
    }
    let path = inspected.log_path.as_deref().ok_or_else(|| {
        DockerError::Validation("A Docker Engine não informou o arquivo de log".to_string())
    })?;
    validate_log_path(id, path)?;

    Ok(LogTarget {
        path: path.to_string(),
        running: inspected
            .state
            .as_ref()
            .and_then(|state| state.running)
            .unwrap_or(false),
    })
}

fn validate_log_path(id: &str, value: &str) -> Result<(), DockerError> {
    let path = Path::new(value);
    let expected_name = format!("{id}-json.log");
    let valid = value.starts_with('/')
        && !value.chars().any(char::is_control)
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        && path.file_name().and_then(|name| name.to_str()) == Some(expected_name.as_str());
    if !valid {
        return Err(DockerError::Validation(
            "O caminho de logs informado pela Docker Engine não é seguro".to_string(),
        ));
    }
    Ok(())
}

fn is_current_container(id: &str) -> bool {
    std::env::var("HOSTNAME")
        .ok()
        .map(|hostname| hostname.trim().to_ascii_lowercase())
        .filter(|hostname| hostname.len() >= 12)
        .is_some_and(|hostname| id.to_ascii_lowercase().starts_with(&hostname))
}

async fn truncate_with_helper(client: &bollard::Docker, source: &str) -> Result<(), DockerError> {
    let created = call(client.create_container(
        Some(CreateContainerOptions {
            name: format!("netmonitor-log-clear-{}", uuid::Uuid::new_v4()),
            platform: None,
        }),
        Config {
            image: Some(volume_export::EXPORT_IMAGE.to_string()),
            cmd: Some(vec![
                "sh".to_string(),
                "-c".to_string(),
                format!(": > {LOG_MOUNT_POINT}"),
            ]),
            host_config: Some(HostConfig {
                cap_drop: Some(vec!["ALL".to_string()]),
                network_mode: Some("none".to_string()),
                readonly_rootfs: Some(true),
                security_opt: Some(vec!["no-new-privileges:true".to_string()]),
                mounts: Some(vec![Mount {
                    target: Some(LOG_MOUNT_POINT.to_string()),
                    source: Some(source.to_string()),
                    typ: Some(MountTypeEnum::BIND),
                    read_only: Some(false),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        },
    ))
    .await?;
    if created.id.is_empty() {
        return Err(DockerError::Engine);
    }
    if let Err(error) = call(client.start_container::<String>(&created.id, None)).await {
        let _ =
            engine::container_action(&created.id, engine::ContainerAction::Remove { force: true })
                .await;
        return Err(error);
    }

    let mut wait = client.wait_container(&created.id, None::<WaitContainerOptions<String>>);
    let result = tokio::time::timeout(HELPER_TIMEOUT, wait.next())
        .await
        .ok()
        .flatten()
        .and_then(Result::ok);
    let succeeded = result.is_some_and(|result| result.status_code == 0);
    let _ = engine::container_action(&created.id, engine::ContainerAction::Remove { force: true })
        .await;
    if !succeeded {
        return Err(DockerError::Engine);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aceita_somente_o_json_log_do_container_inspecionado() {
        let id = "a".repeat(64);
        assert!(validate_log_path(
            &id,
            &format!("/var/lib/docker/containers/{id}/{id}-json.log")
        )
        .is_ok());
        assert!(validate_log_path(&id, "/etc/passwd").is_err());
        assert!(validate_log_path(&id, &format!("/tmp/../{id}-json.log")).is_err());
    }
}
