//! Deploy contínuo de stacks compose no host do agente (ADR 021).
//!
//! O agente escreve **só** dentro de `AGENT_STACKS_DIR` (no container, um
//! volume montado em `/stacks`; nativo, `/srv/netmonitor/stacks`). O CLI
//! compose roda aqui, mas quem resolve os bind mounts relativos
//! (`./docker/nginx`) é a Engine do host: [`host_path`] descobre de onde vem a
//! montagem e o compose trabalha pelo caminho do host.
//!
//! | Peça | Responsabilidade |
//! |---|---|
//! | [`host_path`] | o caminho da raiz no host, visto de dentro do agente |
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
pub mod host_path;
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

use super::{
    source::{DockerEngine, LocalEngine},
    DockerError,
};

/// Variável com a raiz das stacks no host (e no container, mesmo caminho).
pub const STACKS_DIR_ENV: &str = "AGENT_STACKS_DIR";
/// Raiz padrão do agente nativo: o caminho já é o do host.
pub const DEFAULT_STACKS_DIR: &str = "/srv/netmonitor/stacks";
/// Raiz na imagem do agente, onde vai montado o volume das stacks.
pub const CONTAINER_STACKS_DIR: &str = "/stacks";
/// Volume das stacks no `docker run` exibido e no compose da central.
pub const STACKS_VOLUME: &str = "netmonitor-stacks";

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

    /// A raiz pelo caminho do **host**. Fora de container é a própria
    /// `AGENT_STACKS_DIR`; dentro, o agente lê na Engine a origem do que está
    /// montado nela e deixa esse caminho apontando para a montagem
    /// ([`host_path`]).
    ///
    /// # Errors
    ///
    /// O motivo, em português, para a tela.
    pub async fn resolve(engine: &dyn DockerEngine) -> Result<Self, String> {
        let mounted = Self::from_env();
        let mountinfo = tokio::fs::read_to_string("/proc/self/mountinfo")
            .await
            .unwrap_or_default();
        let Some(id) = host_path::own_container_id(&mountinfo) else {
            return Ok(mounted);
        };
        let inspected = engine.inspect_container_raw(&id).await.map_err(|error| {
            format!("o agente não conseguiu ler as próprias montagens na Engine: {error}")
        })?;
        let host = host_path::mount_source(&inspected, mounted.path()).ok_or_else(|| {
            format!(
                "{} não está montado no agente: monte um volume nele ({STACKS_DIR_ENV})",
                mounted.path().display()
            )
        })?;
        let (alias, target) = (host.clone(), mounted.0);
        tokio::task::spawn_blocking(move || host_path::ensure_alias(&alias, &target))
            .await
            .map_err(|error| error.to_string())??;
        Ok(Self(host))
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
                "{} não existe no agente: monte um volume nele ({STACKS_DIR_ENV})",
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
        let root = StacksRoot::resolve(self)
            .await
            .map_err(DockerError::Validation)?;
        deploy::run(self, &root, request, &events).await
    }

    async fn stack_readiness(&self, project: &str) -> Result<StackReadiness, DockerError> {
        readiness::inspect(self, StacksRoot::resolve(self).await, project).await
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
