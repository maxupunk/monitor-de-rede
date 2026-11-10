//! Do registry ao `docker load` do host (ADR 022 RF-10/11).
//!
//! O tar tem as duas leituras que o Docker conhece: o `manifest.json` do
//! formato clássico (Engine sem containerd) e o layout OCI (`index.json`,
//! `oci-layout`) — o mesmo arranjo do `docker save` desde o Engine 25. Só vai
//! a imagem da arquitetura do host, e ela chega com o nome que o `image:` do
//! compose espera (`RepoTags`), então o `up` não precisa de registry.

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWrite};

use netmonitor_core::shared::errors::{AppError, AppResult};
use sea_orm::DatabaseConnection;

use crate::{
    catalog,
    digest::Digest,
    manifest::{self, Kind, OCI_MANIFEST},
    storage::BlobStore,
    tarstream,
};

fn blob_path(digest: &Digest) -> String {
    format!("blobs/sha256/{}", digest.hex())
}

fn broken(error: std::io::Error) -> AppError {
    AppError::service_unavailable(format!("A entrega da imagem foi interrompida: {error}"))
}

/// O nome como o image store do containerd o guarda: totalmente qualificado
/// (`api:1` → `docker.io/library/api:1`, `acme/api:1` → `docker.io/acme/api:1`).
/// Com o nome curto, `docker images` mostra a imagem mas `docker run` não a
/// acha (o `docker save` do Engine grava assim).
#[must_use]
pub fn qualified(reference: &str) -> String {
    let first = reference.split('/').next().unwrap_or(reference);
    let has_registry = reference.contains('/')
        && (first.contains('.') || first.contains(':') || first == "localhost");
    if has_registry {
        reference.to_string()
    } else if reference.contains('/') {
        format!("docker.io/{reference}")
    } else {
        format!("docker.io/library/{reference}")
    }
}

/// A imagem (manifest de uma plataforma) de `reference` para a arquitetura.
///
/// # Errors
///
/// `NotFound` sem a imagem ou sem a arquitetura.
pub async fn image_for(
    db: &DatabaseConnection,
    repository_id: i64,
    reference: &str,
    architecture: &str,
) -> AppResult<(Digest, String)> {
    let top = catalog::resolve(db, repository_id, reference)
        .await?
        .ok_or_else(|| AppError::not_found(format!("A imagem {reference} não está no registry")))?;
    let parsed = manifest::parse(top.content.as_bytes(), Some(&top.media_type))
        .map_err(|error| AppError::validation(error.to_string()))?;
    match &parsed.kind {
        Kind::Image { .. } => Ok((
            top.digest
                .parse()
                .map_err(|_| AppError::validation("digest inválido no catálogo"))?,
            top.content,
        )),
        Kind::Index { .. } => {
            let child = parsed.for_architecture(architecture).ok_or_else(|| {
                AppError::business_rule(format!(
                    "A imagem {reference} não tem versão para {architecture} (a arquitetura do host)"
                ))
            })?;
            let found = catalog::manifest_by_digest(db, repository_id, child.digest.as_str())
                .await?
                .ok_or_else(|| AppError::not_found("Manifest da plataforma ausente"))?;
            Ok((child.digest.clone(), found.content))
        }
    }
}

/// Escreve o tar do `docker load` em `sink`.
///
/// # Errors
///
/// Imagem ausente, blob faltando no storage ou ponte caída.
pub async fn write<W: AsyncWrite + Unpin>(
    db: &DatabaseConnection,
    store: &dyn BlobStore,
    repository_id: i64,
    reference: &str,
    architecture: &str,
    repo_tag: &str,
    sink: W,
) -> AppResult<W> {
    let (digest, content) = image_for(db, repository_id, reference, architecture).await?;
    let parsed = manifest::parse(content.as_bytes(), None)
        .map_err(|error| AppError::validation(error.to_string()))?;
    let Kind::Image { config, layers } = &parsed.kind else {
        return Err(AppError::validation("Esperava a imagem de uma plataforma"));
    };

    let mut tar = tarstream::Writer::new(sink);
    let mut buffer = vec![0_u8; 256 * 1024];
    for blob in std::iter::once(config).chain(layers) {
        let size = store
            .size(&blob.digest)
            .await
            .map_err(broken)?
            .ok_or_else(|| {
                AppError::not_found(format!("O blob {} sumiu do storage", blob.digest))
            })?;
        tar.begin_file(&blob_path(&blob.digest), size)
            .await
            .map_err(broken)?;
        let mut reader = store.open(&blob.digest, 0).await.map_err(broken)?;
        loop {
            let count = reader.read(&mut buffer).await.map_err(broken)?;
            if count == 0 {
                break;
            }
            tar.write_data(&buffer[..count]).await.map_err(broken)?;
        }
        tar.end_file(size).await.map_err(broken)?;
    }
    tar.file(&blob_path(&digest), content.as_bytes())
        .await
        .map_err(broken)?;

    let legacy = json!([{
        "Config": blob_path(&config.digest),
        "RepoTags": [repo_tag],
        "Layers": layers.iter().map(|layer| blob_path(&layer.digest)).collect::<Vec<_>>(),
    }]);
    let index = json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": [{
            "mediaType": if parsed.media_type.is_empty() { OCI_MANIFEST } else { parsed.media_type.as_str() },
            "digest": digest.as_str(),
            "size": content.len(),
            "annotations": {
                "io.containerd.image.name": qualified(repo_tag),
                "org.opencontainers.image.ref.name": repo_tag.rsplit(':').next().unwrap_or(repo_tag),
            }
        }]
    });
    tar.file("manifest.json", legacy.to_string().as_bytes())
        .await
        .map_err(broken)?;
    tar.file("index.json", index.to_string().as_bytes())
        .await
        .map_err(broken)?;
    tar.file("oci-layout", br#"{"imageLayoutVersion":"1.0.0"}"#)
        .await
        .map_err(broken)?;
    tar.finish().await.map_err(broken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_qualificado_como_o_docker_save() {
        assert_eq!(qualified("api:1"), "docker.io/library/api:1");
        assert_eq!(qualified("acme/api:1"), "docker.io/acme/api:1");
        assert_eq!(qualified("ghcr.io/acme/api:1"), "ghcr.io/acme/api:1");
        assert_eq!(qualified("localhost/api:1"), "localhost/api:1");
        assert_eq!(
            qualified("10.8.0.1:3333/equipe/api:1"),
            "10.8.0.1:3333/equipe/api:1"
        );
    }
}
