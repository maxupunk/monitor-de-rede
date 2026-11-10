//! Manifests: a imagem de uma plataforma, o índice multiplataforma e as
//! attestations que o BuildKit pendura no índice (SBOM e proveniência).
//!
//! Aceita os tipos OCI e os Docker v2 (`schema2` e manifest list), que é o
//! que `docker push` e `buildx` mandam.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::digest::Digest;

pub const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
pub const OCI_INDEX: &str = "application/vnd.oci.image.index.v1+json";
pub const DOCKER_MANIFEST: &str = "application/vnd.docker.distribution.manifest.v2+json";
pub const DOCKER_LIST: &str = "application/vnd.docker.distribution.manifest.list.v2+json";
/// Teto de um manifest (recomendação da especificação).
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

/// Anotação do BuildKit que marca uma attestation dentro do índice.
const REFERENCE_TYPE: &str = "vnd.docker.reference.type";
const REFERENCE_DIGEST: &str = "vnd.docker.reference.digest";
/// Tipo do predicado in-toto de cada camada de attestation.
pub const PREDICATE_TYPE: &str = "in-toto.io/predicate-type";
pub const SPDX_PREDICATE: &str = "https://spdx.dev/Document";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    pub architecture: String,
    pub os: String,
    #[serde(default)]
    pub variant: Option<String>,
}

impl Platform {
    #[must_use]
    pub fn label(&self) -> String {
        match &self.variant {
            Some(variant) => format!("{}/{}/{variant}", self.os, self.architecture),
            None => format!("{}/{}", self.os, self.architecture),
        }
    }

    fn is_unknown(&self) -> bool {
        self.os == "unknown" || self.architecture == "unknown"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDescriptor {
    media_type: String,
    digest: String,
    size: i64,
    #[serde(default)]
    platform: Option<Platform>,
    #[serde(default)]
    annotations: Option<Map<String, Value>>,
    #[serde(default)]
    artifact_type: Option<String>,
}

/// Um descritor já validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    pub media_type: String,
    pub digest: Digest,
    pub size: i64,
    pub platform: Option<Platform>,
    pub annotations: Map<String, Value>,
}

impl Descriptor {
    fn annotation(&self, key: &str) -> Option<&str> {
        self.annotations.get(key).and_then(Value::as_str)
    }

    /// É uma attestation do BuildKit (e de qual manifest).
    #[must_use]
    pub fn attests(&self) -> Option<Digest> {
        (self.annotation(REFERENCE_TYPE) == Some("attestation-manifest"))
            .then(|| self.annotation(REFERENCE_DIGEST)?.parse().ok())
            .flatten()
    }

    /// O predicado in-toto de uma camada de attestation.
    #[must_use]
    pub fn predicate(&self) -> Option<&str> {
        self.annotation(PREDICATE_TYPE)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    #[serde(default)]
    schema_version: Option<i64>,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    artifact_type: Option<String>,
    #[serde(default)]
    config: Option<RawDescriptor>,
    #[serde(default)]
    layers: Option<Vec<RawDescriptor>>,
    #[serde(default)]
    manifests: Option<Vec<RawDescriptor>>,
    #[serde(default)]
    subject: Option<RawDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Imagem de uma plataforma (ou artefato): config + camadas.
    Image {
        config: Descriptor,
        layers: Vec<Descriptor>,
    },
    /// Índice: outros manifests.
    Index { manifests: Vec<Descriptor> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub media_type: String,
    pub artifact_type: Option<String>,
    pub subject: Option<Digest>,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest grande demais")]
    TooLarge,
    #[error("manifest inválido: {0}")]
    Invalid(String),
    #[error("tipo de manifest não suportado: {0}")]
    Unsupported(String),
}

fn descriptor(raw: RawDescriptor) -> Result<Descriptor, ManifestError> {
    let digest = raw
        .digest
        .parse()
        .map_err(|_| ManifestError::Invalid(format!("digest inválido: {}", raw.digest)))?;
    if raw.size < 0 {
        return Err(ManifestError::Invalid("tamanho negativo".into()));
    }
    Ok(Descriptor {
        media_type: raw.media_type,
        digest,
        size: raw.size,
        platform: raw.platform,
        annotations: raw.annotations.unwrap_or_default(),
    })
}

/// Lê um manifest. `declared` é o `Content-Type` do envio: quando presente,
/// manda; sem ele, vale o `mediaType` do próprio documento.
///
/// # Errors
///
/// [`ManifestError`] com o motivo.
pub fn parse(bytes: &[u8], declared: Option<&str>) -> Result<Manifest, ManifestError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge);
    }
    let raw: Raw =
        serde_json::from_slice(bytes).map_err(|error| ManifestError::Invalid(error.to_string()))?;
    if raw.schema_version.is_some_and(|version| version != 2) {
        return Err(ManifestError::Unsupported(
            "schemaVersion diferente de 2".into(),
        ));
    }
    let declared = declared
        .map(|value| value.split(';').next().unwrap_or(value).trim())
        .filter(|value| !value.is_empty() && *value != "application/json");
    let media_type = match (declared, raw.media_type.as_deref()) {
        (Some(declared), Some(inner)) if declared != inner => {
            return Err(ManifestError::Invalid(
                "Content-Type diferente do mediaType do manifest".into(),
            ))
        }
        (Some(value), _) | (None, Some(value)) => value.to_string(),
        (None, None) if raw.manifests.is_some() => OCI_INDEX.to_string(),
        (None, None) => OCI_MANIFEST.to_string(),
    };
    let subject = raw.subject.map(descriptor).transpose()?.map(|d| d.digest);
    let kind = match media_type.as_str() {
        OCI_MANIFEST | DOCKER_MANIFEST => Kind::Image {
            config: descriptor(
                raw.config
                    .ok_or_else(|| ManifestError::Invalid("manifest sem config".into()))?,
            )?,
            layers: raw
                .layers
                .unwrap_or_default()
                .into_iter()
                .map(descriptor)
                .collect::<Result<_, _>>()?,
        },
        OCI_INDEX | DOCKER_LIST => Kind::Index {
            manifests: raw
                .manifests
                .unwrap_or_default()
                .into_iter()
                .map(descriptor)
                .collect::<Result<_, _>>()?,
        },
        other => return Err(ManifestError::Unsupported(other.to_string())),
    };
    Ok(Manifest {
        media_type,
        artifact_type: raw.artifact_type,
        subject,
        kind,
    })
}

impl Manifest {
    /// Os blobs de que o manifest depende (config e camadas).
    #[must_use]
    pub fn blobs(&self) -> Vec<&Descriptor> {
        match &self.kind {
            Kind::Image { config, layers } => std::iter::once(config).chain(layers).collect(),
            Kind::Index { .. } => Vec::new(),
        }
    }

    /// Os manifests filhos (índice).
    #[must_use]
    pub fn children(&self) -> &[Descriptor] {
        match &self.kind {
            Kind::Index { manifests } => manifests,
            Kind::Image { .. } => &[],
        }
    }

    /// Soma dos blobs (imagem).
    #[must_use]
    pub fn blobs_size(&self) -> i64 {
        self.blobs().iter().map(|blob| blob.size).sum()
    }

    /// As imagens de verdade de um índice — sem as attestations nem as
    /// plataformas `unknown`.
    #[must_use]
    pub fn images(&self) -> Vec<&Descriptor> {
        self.children()
            .iter()
            .filter(|child| {
                child.attests().is_none()
                    && child
                        .platform
                        .as_ref()
                        .is_none_or(|platform| !platform.is_unknown())
            })
            .collect()
    }

    /// A imagem de uma arquitetura (`amd64`, `arm64`).
    #[must_use]
    pub fn for_architecture(&self, architecture: &str) -> Option<&Descriptor> {
        let images = self.images();
        images
            .iter()
            .find(|child| {
                child.platform.as_ref().is_some_and(|platform| {
                    platform.os == "linux" && platform.architecture == architecture
                })
            })
            // Um índice sem plataforma declarada serve a quem pedir.
            .or_else(|| (images.len() == 1 && images[0].platform.is_none()).then(|| &images[0]))
            .copied()
    }

    /// A attestation de um manifest de imagem (dentro deste índice).
    #[must_use]
    pub fn attestation_of(&self, image: &Digest) -> Option<&Descriptor> {
        self.children()
            .iter()
            .find(|child| child.attests().as_ref() == Some(image))
    }
}

pub use netmonitor_servers::docker::builds::oci_architecture;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn d(hex: char) -> String {
        format!("sha256:{}", hex.to_string().repeat(64))
    }

    #[test]
    fn indice_do_buildx_com_attestation() {
        let index = json!({
            "schemaVersion": 2,
            "mediaType": OCI_INDEX,
            "manifests": [
                {"mediaType": OCI_MANIFEST, "digest": d('a'), "size": 10,
                 "platform": {"architecture": "amd64", "os": "linux"}},
                {"mediaType": OCI_MANIFEST, "digest": d('b'), "size": 10,
                 "platform": {"architecture": "unknown", "os": "unknown"},
                 "annotations": {"vnd.docker.reference.type": "attestation-manifest",
                                 "vnd.docker.reference.digest": d('a')}}
            ]
        });
        let parsed = parse(index.to_string().as_bytes(), None).unwrap();
        assert_eq!(parsed.images().len(), 1);
        let amd64 = parsed.for_architecture("amd64").unwrap();
        assert_eq!(amd64.digest.as_str(), d('a'));
        let attestation = parsed.attestation_of(&amd64.digest).unwrap();
        assert_eq!(attestation.digest.as_str(), d('b'));
        // A imagem amd64 não serve a um host arm64.
        assert!(parsed.for_architecture("arm64").is_none());
    }

    #[test]
    fn imagem_docker_v2_e_tipo_divergente() {
        let image = json!({
            "schemaVersion": 2,
            "mediaType": DOCKER_MANIFEST,
            "config": {"mediaType": "application/vnd.docker.container.image.v1+json", "digest": d('c'), "size": 5},
            "layers": [{"mediaType": "application/vnd.docker.image.rootfs.diff.tar.gzip", "digest": d('d'), "size": 7}]
        });
        let bytes = image.to_string();
        let parsed = parse(bytes.as_bytes(), Some(DOCKER_MANIFEST)).unwrap();
        assert_eq!(parsed.blobs().len(), 2);
        assert_eq!(parsed.blobs_size(), 12);
        assert!(parse(bytes.as_bytes(), Some(OCI_MANIFEST)).is_err());
    }

    #[test]
    fn digest_ruim_e_tipo_estranho_sao_recusados() {
        let bad = json!({"schemaVersion": 2, "mediaType": OCI_MANIFEST,
            "config": {"mediaType": "x", "digest": "sha256:../../etc", "size": 1}, "layers": []});
        assert!(parse(bad.to_string().as_bytes(), None).is_err());
        let odd = json!({"schemaVersion": 2, "mediaType": "application/x-coisa"});
        assert!(matches!(
            parse(odd.to_string().as_bytes(), None),
            Err(ManifestError::Unsupported(_))
        ));
    }

    #[test]
    fn arquitetura_do_rust_para_oci() {
        assert_eq!(oci_architecture("x86_64"), "amd64");
        assert_eq!(oci_architecture("aarch64"), "arm64");
    }
}
