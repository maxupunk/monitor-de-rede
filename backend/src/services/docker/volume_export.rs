//! Exporta volumes via Docker API usando um container auxiliar efêmero.

use std::{pin::Pin, task::Poll};

use bollard::{
    container::{Config, CreateContainerOptions, DownloadFromContainerOptions},
    image::CreateImageOptions,
    models::{HostConfig, Mount, MountTypeEnum},
};
use futures::StreamExt;
use tokio::io::{AsyncRead, ReadBuf};

use super::{call, client, engine, DockerError};

pub(crate) const EXPORT_IMAGE: &str = "alpine:latest";
const MOUNT_POINT: &str = "/volume";

pub struct VolumeExport {
    pub file_name: String,
    reader: Pin<Box<dyn AsyncRead + Send>>,
    container_id: String,
}

impl AsyncRead for VolumeExport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.reader.as_mut().poll_read(context, buffer)
    }
}

impl Drop for VolumeExport {
    fn drop(&mut self) {
        let id = self.container_id.clone();
        tokio::spawn(async move {
            let _ = engine::container_action(&id, engine::ContainerAction::Remove { force: true })
                .await;
        });
    }
}

pub async fn export(name: &str) -> Result<VolumeExport, DockerError> {
    engine::inspect_volume(name).await?;
    let client = client()?;
    ensure_image(&client).await?;

    let created = call(client.create_container(
        Some(CreateContainerOptions {
            name: format!("netmonitor-volume-export-{}", uuid::Uuid::new_v4()),
            platform: None,
        }),
        Config {
            image: Some(EXPORT_IMAGE.to_string()),
            cmd: Some(vec!["true".to_string()]),
            host_config: Some(HostConfig {
                mounts: Some(vec![Mount {
                    target: Some(MOUNT_POINT.to_string()),
                    source: Some(name.to_string()),
                    typ: Some(MountTypeEnum::VOLUME),
                    read_only: Some(true),
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

    let stream = client
        .download_from_container(
            &created.id,
            Some(DownloadFromContainerOptions {
                path: MOUNT_POINT.to_string(),
            }),
        )
        .map(|chunk| chunk.map_err(|_| std::io::Error::other("falha ao exportar volume")));
    let reader = tokio_util::io::StreamReader::new(stream);

    Ok(VolumeExport {
        file_name: file_name(name),
        reader: Box::pin(reader),
        container_id: created.id,
    })
}

pub(crate) async fn ensure_image(client: &bollard::Docker) -> Result<(), DockerError> {
    match call(client.inspect_image(EXPORT_IMAGE)).await {
        Ok(_) => return Ok(()),
        Err(DockerError::NotFound) => {}
        Err(error) => return Err(error),
    }
    let mut stream = client.create_image(
        Some(CreateImageOptions {
            from_image: EXPORT_IMAGE,
            ..Default::default()
        }),
        None,
        None,
    );
    loop {
        let next = tokio::time::timeout(std::time::Duration::from_secs(60), stream.next())
            .await
            .map_err(|_| DockerError::Unavailable)?;
        let Some(result) = next else { break };
        result.map_err(|_| DockerError::Engine)?;
    }
    Ok(())
}

fn file_name(volume_name: &str) -> String {
    let safe = volume_name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!(
        "volume-{safe}-{}.tar.gz",
        chrono::Utc::now().format("%Y-%m-%d")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_do_arquivo_nao_aceita_separadores() {
        assert!(file_name("dados/../banco").starts_with("volume-dados____banco-"));
    }
}
