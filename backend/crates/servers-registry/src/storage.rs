//! Onde os bytes moram (RNF-04): o trait [`BlobStore`] e o disco local.
//!
//! O conteúdo é endereçado pelo digest e gravado uma vez só: duas imagens com
//! a mesma camada de base ocupam o espaço de uma. Todo blob entra por um
//! arquivo temporário já conferido ([`BlobStore::commit`]) — nada vira
//! visível pela metade. Um driver S3 entra implementando o trait, sem mexer
//! na API, no catálogo nem na varredura.

use std::{
    io,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, OnceLock},
};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncSeekExt};

use crate::digest::Digest;

/// Pasta do registry (`REGISTRY_DIR`). No container, `/data/registry`: fora da
/// raiz dos armazenamentos, que o explorador lista e apaga.
pub const DIR_ENV: &str = "REGISTRY_DIR";
const DEFAULT_DIR: &str = "registry";

pub type BlobReader = Pin<Box<dyn AsyncRead + Send>>;

#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Tamanho do blob, quando ele existe.
    async fn size(&self, digest: &Digest) -> io::Result<Option<u64>>;

    /// Lê o blob a partir de `offset` (o pull com `Range`).
    async fn open(&self, digest: &Digest, offset: u64) -> io::Result<BlobReader>;

    /// Uma pasta local para montar arquivos antes do [`commit`](Self::commit)
    /// (envios em andamento, arquivos da ingestão).
    fn staging(&self) -> PathBuf;

    /// Torna `staged` o conteúdo de `digest`. Quem chama já conferiu o
    /// digest. Se o blob já existia, o arquivo temporário só é apagado.
    async fn commit(&self, staged: &Path, digest: &Digest) -> io::Result<()>;

    async fn delete(&self, digest: &Digest) -> io::Result<()>;
}

/// Disco local: `blobs/sha256/ab/abcdef…`, `staging/`.
pub struct LocalDisk {
    root: PathBuf,
}

impl LocalDisk {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, digest: &Digest) -> PathBuf {
        let hex = digest.hex();
        self.root
            .join("blobs")
            .join("sha256")
            .join(&hex[..2])
            .join(hex)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[async_trait]
impl BlobStore for LocalDisk {
    async fn size(&self, digest: &Digest) -> io::Result<Option<u64>> {
        match tokio::fs::metadata(self.path(digest)).await {
            Ok(meta) => Ok(Some(meta.len())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn open(&self, digest: &Digest, offset: u64) -> io::Result<BlobReader> {
        let mut file = tokio::fs::File::open(self.path(digest)).await?;
        if offset > 0 {
            file.seek(io::SeekFrom::Start(offset)).await?;
        }
        Ok(Box::pin(file))
    }

    fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }

    async fn commit(&self, staged: &Path, digest: &Digest) -> io::Result<()> {
        let target = self.path(digest);
        if tokio::fs::try_exists(&target).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(staged).await;
            return Ok(());
        }
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        // O conteúdo vai ao disco antes do nome: uma queda depois do rename
        // não deixa um blob com o digest certo e os bytes errados.
        tokio::fs::OpenOptions::new()
            .write(true)
            .open(staged)
            .await?
            .sync_all()
            .await?;
        tokio::fs::rename(staged, &target).await
    }

    async fn delete(&self, digest: &Digest) -> io::Result<()> {
        match tokio::fs::remove_file(self.path(digest)).await {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

/// O storage do processo (`REGISTRY_DIR`).
#[must_use]
pub fn store() -> Arc<dyn BlobStore> {
    static STORE: OnceLock<Arc<dyn BlobStore>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            let dir = std::env::var(DIR_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_DIR.to_string());
            Arc::new(LocalDisk::new(dir))
        })
        .clone()
}

/// Um arquivo na área de montagem, apagado se ninguém o consumir.
pub struct Staged {
    pub path: PathBuf,
    keep: bool,
}

impl Staged {
    /// Um nome novo na área de montagem do storage.
    ///
    /// # Errors
    ///
    /// Pasta não criada.
    pub async fn new(store: &dyn BlobStore) -> io::Result<Self> {
        let dir = store.staging();
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self {
            path: dir.join(uuid::Uuid::new_v4().simple().to_string()),
            keep: false,
        })
    }

    /// O arquivo de um envio em andamento (`uploads/<id>`), que sobrevive
    /// entre requisições.
    ///
    /// # Errors
    ///
    /// Pasta não criada.
    pub async fn upload(store: &dyn BlobStore, id: &uuid::Uuid) -> io::Result<Self> {
        let dir = store.staging().join("uploads");
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self {
            path: dir.join(id.simple().to_string()),
            keep: true,
        })
    }

    /// O arquivo foi consumido (commit) ou deve ficar.
    pub fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn commit_dedup_e_leitura_com_deslocamento() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalDisk::new(dir.path());
        let digest = Digest::of(b"camada");
        for _ in 0..2 {
            let mut staged = Staged::new(&store).await.unwrap();
            tokio::fs::write(&staged.path, b"camada").await.unwrap();
            store.commit(&staged.path, &digest).await.unwrap();
            staged.keep();
        }
        assert_eq!(store.size(&digest).await.unwrap(), Some(6));
        let mut reader = store.open(&digest, 2).await.unwrap();
        let mut rest = String::new();
        reader.read_to_string(&mut rest).await.unwrap();
        assert_eq!(rest, "mada");
        drop(reader);
        store.delete(&digest).await.unwrap();
        assert_eq!(store.size(&digest).await.unwrap(), None);
        store.delete(&digest).await.unwrap();
    }
}
