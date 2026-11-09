//! Deploy contínuo de stacks compose no host do agente (ADR 021).
//!
//! O agente escreve **só** dentro de `AGENT_STACKS_DIR` (padrão
//! `/srv/netmonitor/stacks`), montado no container no **mesmo caminho** do
//! host: o CLI compose roda aqui, mas quem resolve os bind mounts relativos
//! (`./docker/nginx`) é a Engine do host — um caminho diferente montaria
//! diretórios vazios.
//!
//! | Peça | Responsabilidade |
//! |---|---|
//! | [`git`] | o repositório no commit pedido, com o token só no ambiente |
//! | [`env_file`] | o `.env` gravado de forma atômica, `0600` |
//! | [`registry`] | o login de uso único no registry (`DOCKER_CONFIG` temporário) |
//! | [`images`] | quais tags antigas apagar, mantendo as últimas N |
//! | [`deploy`] | a sequência: sincronizar, gravar, baixar, subir, limpar |
//! | [`readiness`] | o que falta no host, sem mudar nada |
//!
//! A central fala com isto pelo trait [`StackHost`]: no agente, o
//! [`LocalEngine`](super::source::LocalEngine) executa; na central, o
//! [`AgentEngine`](super::remote_engine::AgentEngine) leva o pedido pelo canal.

pub mod deploy;
pub mod env_file;
pub mod git;
pub mod images;
pub mod readiness;
pub mod registry;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

pub use agent_protocol::docker::stacks::{
    StackChunk, StackDeployOutcome, StackDeployRequest, StackReadiness, StackServiceState,
    StackStage,
};

use super::{source::LocalEngine, DockerError};

/// Variável com a raiz das stacks no host (e no container, mesmo caminho).
pub const STACKS_DIR_ENV: &str = "AGENT_STACKS_DIR";
/// Raiz padrão, a mesma do compose do agente.
pub const DEFAULT_STACKS_DIR: &str = "/srv/netmonitor/stacks";

/// Para onde vão as etapas e as linhas de um deploy.
pub type StackEvents = UnboundedSender<StackChunk>;

/// Deploy e prontidão de stacks num host.
#[async_trait]
pub trait StackHost: Send + Sync {
    async fn deploy_stack(
        &self,
        request: &StackDeployRequest,
        events: StackEvents,
    ) -> Result<StackDeployOutcome, DockerError>;

    async fn stack_readiness(&self, project: &str) -> Result<StackReadiness, DockerError>;
}

/// A raiz das stacks deste host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StacksRoot(PathBuf);

impl StacksRoot {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// `AGENT_STACKS_DIR`, ou a raiz padrão.
    #[must_use]
    pub fn from_env() -> Self {
        Self::new(
            std::env::var(STACKS_DIR_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_STACKS_DIR.to_string()),
        )
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// O diretório de um projeto já validado
    /// ([`agent_protocol::docker::stacks::validate_project`]).
    #[must_use]
    pub fn project_dir(&self, project: &str) -> PathBuf {
        self.0.join(project)
    }

    /// A raiz existe e aceita escrita? `Err` com o motivo, para a tela.
    ///
    /// # Errors
    ///
    /// O motivo, em português.
    pub async fn check(&self) -> Result<(), String> {
        let metadata = tokio::fs::metadata(&self.0).await.map_err(|_| {
            format!(
                "{} não existe no agente: monte o diretório do host no mesmo caminho \
                 ({STACKS_DIR_ENV})",
                self.0.display()
            )
        })?;
        if !metadata.is_dir() {
            return Err(format!("{} não é um diretório", self.0.display()));
        }
        let probe = self
            .0
            .join(format!(".netmonitor-write-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&probe, b"ok")
            .await
            .map_err(|_| format!("{} está montado sem permissão de escrita", self.0.display()))?;
        let _ = tokio::fs::remove_file(&probe).await;
        Ok(())
    }
}

#[async_trait]
impl StackHost for LocalEngine {
    async fn deploy_stack(
        &self,
        request: &StackDeployRequest,
        events: StackEvents,
    ) -> Result<StackDeployOutcome, DockerError> {
        deploy::run(self, &StacksRoot::from_env(), request, &events).await
    }

    async fn stack_readiness(&self, project: &str) -> Result<StackReadiness, DockerError> {
        readiness::inspect(self, &StacksRoot::from_env(), project).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn raiz_inexistente_explica_a_montagem() {
        let root = StacksRoot::new("/caminho/que/nao/existe/netmonitor");
        let reason = root.check().await.expect_err("sem raiz");
        assert!(reason.contains(STACKS_DIR_ENV));
    }

    #[tokio::test]
    async fn raiz_gravavel_passa() {
        let dir = tempfile::tempdir().expect("tmp");
        assert!(StacksRoot::new(dir.path()).check().await.is_ok());
    }

    #[test]
    fn diretorio_do_projeto_fica_na_raiz() {
        let root = StacksRoot::new("/srv/netmonitor/stacks");
        assert_eq!(
            root.project_dir("sgc"),
            PathBuf::from("/srv/netmonitor/stacks/sgc")
        );
    }
}
