//! Operações de manutenção: limpar logs, atualizar imagem, compose e
//! exportação de volume.
//!
//! Ficam fora de [`super::source::DockerEngine`] de propósito (segregação de
//! interface): consultas e ciclo de vida são rápidos e sem estado; estas
//! operações são longas, relatam progresso ou transmitem bytes, e nem todo
//! host oferece todas — compose, por exemplo, depende do CLI no agente.

use std::pin::Pin;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncRead, sync::mpsc::UnboundedSender};

use crate::views::docker::DockerActionResponse;

use super::{compose, log_clear, source::LocalEngine, update, volume_export, DockerError};

/// Linhas de progresso de uma operação longa. Envio que falha (ninguém
/// ouvindo) não interrompe a operação.
pub type Progress = UnboundedSender<String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ComposeAction {
    Pull,
    Up,
    Restart,
    Stop,
    Down,
}

impl ComposeAction {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pull => "pull",
            Self::Up => "up",
            Self::Restart => "restart",
            Self::Stop => "stop",
            Self::Down => "down",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeRequest {
    pub project: String,
    pub action: ComposeAction,
    /// Serviço específico; `None` age sobre o projeto inteiro.
    #[serde(default)]
    pub service: Option<String>,
}

/// O tar de um volume a caminho do download. Ler até o fim e soltar o leitor
/// libera o que a origem alocou (o container auxiliar, a ponte do agente).
pub struct VolumeArchive {
    pub file_name: String,
    pub reader: Pin<Box<dyn AsyncRead + Send>>,
}

#[async_trait]
pub trait DockerMaintenance: Send + Sync {
    async fn clear_logs(&self, id: &str) -> Result<DockerActionResponse, DockerError>;
    async fn pull_image(
        &self,
        image: &str,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError>;
    /// Baixa a imagem atual da tag e recria o container com a mesma
    /// configuração, desfazendo tudo se qualquer passo falhar.
    async fn update_container(
        &self,
        id: &str,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError>;
    async fn compose(
        &self,
        request: &ComposeRequest,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError>;
    /// Conteúdo do volume em tar, para download.
    async fn export_volume(&self, name: &str) -> Result<VolumeArchive, DockerError>;
}

/// A Engine deste host. Só o agente a usa (ADR 014): é ele que monta o socket
/// e tem o CLI do compose.
#[async_trait]
impl DockerMaintenance for LocalEngine {
    async fn clear_logs(&self, id: &str) -> Result<DockerActionResponse, DockerError> {
        log_clear::clear(id).await
    }

    async fn pull_image(
        &self,
        image: &str,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        update::pull_image(image, &progress).await
    }

    async fn update_container(
        &self,
        id: &str,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        update::update_container(id, &progress).await
    }

    async fn compose(
        &self,
        request: &ComposeRequest,
        progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        compose::execute(self, request, &progress).await
    }

    async fn export_volume(&self, name: &str) -> Result<VolumeArchive, DockerError> {
        let export = volume_export::export(name).await?;
        Ok(VolumeArchive {
            file_name: export.file_name.clone(),
            reader: Box::pin(export),
        })
    }
}
