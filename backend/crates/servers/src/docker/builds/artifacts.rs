//! Onde o agente trabalha numa compilação: o clone descartável de cada uma e
//! os arquivos OCI que esperam a central buscá-los.
//!
//! Tudo mora em `AGENT_STATE_DIR/builds` — dentro do agente, nunca num
//! caminho vindo da central: a central só conhece o `Uuid` do artefato.

use std::{
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, SystemTime},
};

use tokio::io::{AsyncRead, ReadBuf};
use uuid::Uuid;

use super::DockerError;

const STATE_DIR_ENV: &str = "AGENT_STATE_DIR";
const DEFAULT_STATE_DIR: &str = "/var/lib/netmonitor-agent";
/// Artefato não buscado em duas horas é lixo (a central caiu no meio).
const ARTIFACT_TTL: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildsRoot(PathBuf);

impl BuildsRoot {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    #[must_use]
    pub fn from_env() -> Self {
        let state = std::env::var(STATE_DIR_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_STATE_DIR.to_string());
        Self::new(Path::new(&state).join("builds"))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    fn work_root(&self) -> PathBuf {
        self.0.join("work")
    }

    fn artifacts_root(&self) -> PathBuf {
        self.0.join("artifacts")
    }

    /// O clone de uma compilação.
    #[must_use]
    pub fn work_dir(&self, build: Uuid) -> PathBuf {
        self.work_root().join(build.simple().to_string())
    }

    /// O arquivo OCI de um artefato.
    #[must_use]
    pub fn artifact_path(&self, artifact: Uuid) -> PathBuf {
        self.artifacts_root()
            .join(format!("{}.tar", artifact.simple()))
    }

    /// Cria as pastas e apaga o que ficou de compilações anteriores (clones
    /// de uma queda no meio, artefatos que a central não buscou).
    ///
    /// # Errors
    ///
    /// `Validation` quando a pasta não pode ser criada.
    pub async fn prepare(&self) -> Result<(), DockerError> {
        for dir in [self.work_root(), self.artifacts_root()] {
            tokio::fs::create_dir_all(&dir).await.map_err(|error| {
                DockerError::Validation(format!(
                    "Não foi possível preparar {}: {error}",
                    dir.display()
                ))
            })?;
        }
        sweep(&self.work_root(), ARTIFACT_TTL).await;
        sweep(&self.artifacts_root(), ARTIFACT_TTL).await;
        Ok(())
    }

    /// O arquivo de um artefato, para a ponte. Lido até o fim e solto, o
    /// arquivo é apagado: um artefato é entregue uma vez.
    ///
    /// # Errors
    ///
    /// `NotFound` quando o artefato não existe (já entregue ou expirado).
    pub async fn open(&self, artifact: Uuid) -> Result<ArtifactReader, DockerError> {
        let path = self.artifact_path(artifact);
        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|_| DockerError::NotFound)?;
        Ok(ArtifactReader { file, path })
    }
}

/// Apaga o que tem mais de `ttl` em `dir`.
async fn sweep(dir: &Path, ttl: Duration) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    let now = SystemTime::now();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let old = entry
            .metadata()
            .await
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > ttl);
        if !old {
            continue;
        }
        let path = entry.path();
        let removed = if path.is_dir() {
            tokio::fs::remove_dir_all(&path).await
        } else {
            tokio::fs::remove_file(&path).await
        };
        if let Err(error) = removed {
            tracing::debug!(path = %path.display(), %error, "resto de compilação não apagado");
        }
    }
}

/// Apaga o clone quando a compilação termina, dê certo ou não.
pub struct WorkDir(pub PathBuf);

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Leitor de um artefato que se apaga ao ser solto.
pub struct ArtifactReader {
    file: tokio::fs::File,
    path: PathBuf,
}

impl AsyncRead for ArtifactReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.file).poll_read(cx, buf)
    }
}

impl Drop for ArtifactReader {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn artefato_e_entregue_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        let root = BuildsRoot::new(dir.path());
        root.prepare().await.unwrap();
        let artifact = Uuid::new_v4();
        tokio::fs::write(root.artifact_path(artifact), b"oci")
            .await
            .unwrap();
        let mut reader = root.open(artifact).await.unwrap();
        let mut content = String::new();
        reader.read_to_string(&mut content).await.unwrap();
        assert_eq!(content, "oci");
        drop(reader);
        assert!(matches!(
            root.open(artifact).await,
            Err(DockerError::NotFound)
        ));
    }

    #[test]
    fn caminho_sai_do_uuid_e_fica_na_raiz() {
        let root = BuildsRoot::new("/var/lib/netmonitor-agent/builds");
        let artifact = Uuid::nil();
        assert!(root
            .artifact_path(artifact)
            .starts_with("/var/lib/netmonitor-agent/builds/artifacts"));
    }
}
