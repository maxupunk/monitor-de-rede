//! Operações de manutenção: limpar logs, atualizar imagem e compose.
//!
//! Ficam fora de [`super::source::DockerEngine`] de propósito (segregação de
//! interface): consultas e ciclo de vida são rápidos e sem estado; estas
//! operações são longas, relatam progresso e nem todo host oferece todas —
//! compose, por exemplo, só existe onde há um agente para executar o CLI.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::views::docker::DockerActionResponse;

use super::{log_clear, source::LocalEngine, update, DockerError};

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
}

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

    /// O processo da API nunca executa o CLI `docker` (ADR 010, AGENTS §7).
    /// Compose só existe do lado do agente, que atende o host remoto.
    async fn compose(
        &self,
        _request: &ComposeRequest,
        _progress: Progress,
    ) -> Result<DockerActionResponse, DockerError> {
        Err(DockerError::Unsupported(
            "Ações de compose exigem o agente NetMonitor no servidor de destino".to_string(),
        ))
    }
}
