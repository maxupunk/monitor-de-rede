//! Compilação no servidor e trânsito de imagens (ADR 022 e 023).
//!
//! [`ImageHost`] é o que a central pede a um host — compilar, contar como
//! está o builder, entregar um artefato, receber uma imagem — e tem as duas
//! pontas de sempre: o [`LocalEngine`](super::source::LocalEngine), que faz
//! no host do agente ([`local`]), e o
//! [`AgentEngine`](super::remote_engine::AgentEngine), que pede pelo canal.

pub mod artifacts;
pub mod buildx;
mod local;

pub use local::read_report;

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub use agent_protocol::docker::images::{
    BuildChunk, BuildImageSpec, BuildStage, BuilderReadiness, BuiltArtifact, ImageBuildOutcome,
    ImageBuildRequest, ImageImportRequest,
};
use agent_protocol::tunnel::TunnelStream;

use super::DockerError;

/// A arquitetura no vocabulário OCI: `x86_64` → `amd64`, `aarch64` →
/// `arm64` (o `Hello` do agente usa o nome do Rust; o OCI, o do Go).
#[must_use]
pub fn oci_architecture(arch: &str) -> String {
    match arch {
        "x86_64" | "x86-64" => "amd64",
        "aarch64" => "arm64",
        "armv7" | "arm" => "arm",
        "i686" | "x86" => "386",
        other => other,
    }
    .to_string()
}

/// Andamento de uma compilação (etapas e linhas do `buildx`).
pub type BuildEvents = UnboundedSender<BuildChunk>;

/// Resposta do agente ao fim de um `ImageImport` (a última coisa que a ponte
/// traz de volta).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub loaded: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[async_trait]
pub trait ImageHost: Send + Sync {
    /// Compila as imagens de um commit no builder dedicado.
    ///
    /// # Errors
    ///
    /// `Validation` com as últimas linhas da etapa que falhou; `Forbidden`
    /// sem a permissão `build` no host.
    /// `cancel` interrompe a compilação onde ela estiver (no agente, o
    /// `buildx` em curso é encerrado).
    async fn build_images(
        &self,
        request: &ImageBuildRequest,
        events: BuildEvents,
        cancel: CancellationToken,
    ) -> Result<ImageBuildOutcome, DockerError>;

    /// Como está o builder deste host.
    ///
    /// # Errors
    ///
    /// Host inacessível.
    async fn builder_readiness(&self) -> Result<BuilderReadiness, DockerError>;

    /// O arquivo OCI de um artefato compilado (só leitura; a escrita é
    /// ignorada).
    ///
    /// # Errors
    ///
    /// `NotFound` quando já foi entregue ou expirou.
    async fn open_artifact(&self, artifact: Uuid) -> Result<TunnelStream, DockerError>;

    /// Abre o `docker load` deste host: escreve-se o tar, fecha-se a escrita
    /// e lê-se o [`ImportReport`] em JSON.
    ///
    /// # Errors
    ///
    /// `Validation` com a referência inválida; `Forbidden` sem `deploy`.
    async fn open_import(&self, request: &ImageImportRequest) -> Result<TunnelStream, DockerError>;
}
