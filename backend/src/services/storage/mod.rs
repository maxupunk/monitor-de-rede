//! Armazenamentos: os lugares para onde vão as cópias do backup.
//!
//! Um trait, três implementações e oito providers mapeados sobre elas:
//!
//! | Provider | Implementação |
//! |---|---|
//! | `local` | [`local::LocalExplorer`] |
//! | `aws_s3`, `minio`, `cloudflare_r2`, `s3_compatible` | [`cloud::CloudExplorer`] |
//! | `google_gcs`, `azure_blob` | [`cloud::CloudExplorer`] |
//! | `sftp` | [`sftp::SftpExplorer`] |
//!
//! O cadastro e a credencial cifrada moram em [`service`]; o backup que usa os
//! destinos, em [`backups`]; o ciclo que o dispara sozinho, em [`schedule`].
//!
//! ## O adapter nasce da config, não a recebe a cada chamada
//!
//! A config é consumida na construção ([`explorer_for`]) e o compilador garante
//! o resto: não existe caminho em que o adapter de S3 receba credencial de
//! SFTP.
//!
//! ## Por que `opendal`
//!
//! Ele já está na árvore como base do `ctx.storage` do Loco — bastou ligar os
//! services de S3, GCS e Azure. O SFTP fica com o `russh`, que o projeto já usa
//! para falar com roteadores (ver o cabeçalho de [`sftp`]).

pub mod backups;
pub mod cloud;
pub mod config;
pub mod local;
pub mod provider;
pub mod schedule;
pub mod service;
pub mod sftp;

use std::path::{Path, PathBuf};
use std::pin::Pin;

use async_trait::async_trait;

pub use config::{normalize_path, StorageConfig};
pub use provider::StorageProvider;

use crate::services::shared::errors::AppError;

/// Quantos objetos uma listagem devolve quando o cliente não pede outro número.
pub const DEFAULT_LIST_LIMIT: usize = 100;

/// Teto de objetos por página.
pub const MAX_LIST_LIMIT: usize = 1000;

/// Variável com a pasta sob a qual os armazenamentos locais podem gravar.
pub const LOCAL_ROOT_ENV: &str = "STORAGE_LOCAL_ROOT";

/// Raiz dos armazenamentos locais quando a variável não está definida.
const DEFAULT_LOCAL_ROOT: &str = "storage";

/// Pasta sob a qual todo armazenamento local fica.
///
/// Um destino local não escolhe um caminho qualquer do servidor: ele escolhe
/// uma subpasta daqui. Sem esse teto, o explorador — que lista e **apaga** —
/// alcançaria o próprio banco do sistema com um `basePath` de `/data`. Disco
/// externo ou compartilhamento de rede entra montado debaixo desta raiz.
#[must_use]
pub fn local_root() -> PathBuf {
    std::env::var(LOCAL_ROOT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or_else(|| PathBuf::from(DEFAULT_LOCAL_ROOT), PathBuf::from)
}

/// Um item da listagem de um destino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketObject {
    /// Caminho relativo à raiz do destino, sempre com `/`.
    pub key: String,
    /// Último segmento do `key`, que é o que a interface exibe.
    pub name: String,
    /// `None` em diretório — pasta não tem tamanho próprio, e `0` faria a
    /// interface exibir "0 B".
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub is_directory: bool,
    pub etag: Option<String>,
}

impl BucketObject {
    /// Item de arquivo.
    #[must_use]
    pub fn file(key: impl Into<String>, size: i64, last_modified: Option<String>) -> Self {
        let key = normalize_path(&key.into());

        Self {
            name: leaf_name(&key),
            key,
            size: Some(size),
            last_modified,
            is_directory: false,
            etag: None,
        }
    }

    /// Item de diretório.
    #[must_use]
    pub fn directory(key: impl Into<String>) -> Self {
        let key = normalize_path(&key.into());

        Self {
            name: leaf_name(&key),
            key,
            size: None,
            last_modified: None,
            is_directory: true,
            etag: None,
        }
    }
}

/// Último segmento de uma chave.
#[must_use]
pub fn leaf_name(key: &str) -> String {
    key.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(key)
        .to_string()
}

/// O que o cliente pediu numa listagem.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Continuação opaca. Onde o provider não tem token nativo, é a última
    /// chave devolvida — a listagem recomeça **depois** dela.
    pub cursor: Option<String>,
    pub limit: Option<usize>,
    /// Filtro adicional aplicado dentro de `path`.
    pub prefix: Option<String>,
}

impl ListOptions {
    #[must_use]
    pub fn effective_limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_LIST_LIMIT)
            .clamp(1, MAX_LIST_LIMIT)
    }
}

/// Uma página de objetos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListPage {
    pub objects: Vec<BucketObject>,
    pub next_cursor: Option<String>,
    pub is_truncated: bool,
}

/// Metadados de um objeto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMetadata {
    pub key: String,
    pub size: i64,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
}

/// Leitor de um objeto, já como stream.
pub type ObjectReader = Pin<Box<dyn tokio::io::AsyncRead + Send>>;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Configuração do armazenamento inválida ou incompleta")]
    InvalidConfig,
    #[error("Caminho fora do armazenamento")]
    PathTraversal,
    #[error("Não é permitido excluir a raiz do armazenamento")]
    RootDeletion,
    #[error("{0} não encontrado")]
    NotFound(String),
    #[error("{0}")]
    Backend(String),
    #[error("{operation} não é suportado por este tipo de armazenamento")]
    Unsupported { operation: &'static str },
}

impl StorageError {
    /// Texto que vai para o campo `message` da resposta.
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Erro de backend a partir de qualquer coisa que saiba se exibir.
    pub fn backend(error: impl std::fmt::Display) -> Self {
        Self::Backend(error.to_string())
    }
}

/// O destino que não responde é um problema **dele**, não deste servidor: a
/// mensagem vai inteira para a tela (`400`), porque é ela que diz ao operador
/// se a chave está errada, o bucket não existe ou o host recusou a conexão.
impl From<StorageError> for AppError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::InvalidConfig | StorageError::PathTraversal => {
                Self::validation(error.message())
            }
            StorageError::NotFound(_) => Self::not_found(error.message()),
            StorageError::RootDeletion
            | StorageError::Backend(_)
            | StorageError::Unsupported { .. } => Self::business_rule(error.message()),
        }
    }
}

/// Operações comuns a todos os destinos.
///
/// O trait é pequeno de propósito (segregação de interfaces): são as operações
/// que o backup e o explorador precisam, e nada além.
#[async_trait]
pub trait StorageExplorer: Send + Sync {
    /// Lista o conteúdo de `path`, um nível apenas.
    async fn list_objects(
        &self,
        path: &str,
        options: &ListOptions,
    ) -> Result<ListPage, StorageError>;

    async fn object_metadata(&self, key: &str) -> Result<ObjectMetadata, StorageError>;

    /// Remove um objeto. Com `is_directory`, remove o conteúdo recursivamente.
    async fn delete_object(&self, key: &str, is_directory: bool) -> Result<(), StorageError>;

    /// Confere que o destino responde e que a credencial serve.
    async fn test_connection(&self) -> Result<(), StorageError>;

    /// Envia um arquivo local para o destino.
    async fn put_file(&self, key: &str, source: &Path) -> Result<(), StorageError>;

    /// Abre um objeto para leitura.
    async fn read_object(&self, key: &str) -> Result<ObjectReader, StorageError>;

    /// Identidade do servidor vista na última conexão, onde o protocolo tem
    /// uma (a impressão digital da chave SSH, no SFTP).
    fn observed_identity(&self) -> Option<String> {
        None
    }
}

/// Constrói o adapter de uma config.
///
/// Sem cache: o adapter **carrega** a credencial, e cachear por provider
/// entregaria a credencial de um destino a outro.
///
/// # Errors
///
/// Config incompleta ou que o `opendal` recusa.
pub fn explorer_for(
    config: &StorageConfig,
    provider: StorageProvider,
    local_root: &Path,
) -> Result<Box<dyn StorageExplorer>, StorageError> {
    if !provider.accepts(config) {
        return Err(StorageError::InvalidConfig);
    }
    Ok(match config {
        StorageConfig::Local(local) => Box::new(local::LocalExplorer::new(local, local_root)?),
        StorageConfig::S3(_) | StorageConfig::Gcs(_) | StorageConfig::AzureBlob(_) => {
            Box::new(cloud::CloudExplorer::new(config, Some(provider))?)
        }
        StorageConfig::Sftp(sftp) => Box::new(sftp::SftpExplorer::new(sftp)),
    })
}

/// Recusa a remoção da raiz do destino.
///
/// Um `DELETE` com `key: ""` ou `key: "/"` apagaria o bucket inteiro, e a
/// interface envia a chave que o usuário selecionou — um clique na linha
/// errada não pode ter esse alcance.
///
/// # Errors
///
/// [`StorageError::RootDeletion`] para qualquer forma da raiz.
pub fn assert_deletable(key: &str) -> Result<String, StorageError> {
    let trimmed = key.trim();

    if trimmed.is_empty() || trimmed == "/" || trimmed == "." || normalize_path(trimmed).is_empty()
    {
        return Err(StorageError::RootDeletion);
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_leaf_of_a_key() {
        assert_eq!(leaf_name("12/vendas.json"), "vendas.json");
        assert_eq!(leaf_name("vendas.json"), "vendas.json");
        // Chave de diretório termina em barra em alguns providers.
        assert_eq!(leaf_name("12/subpasta/"), "subpasta");
        assert_eq!(leaf_name(""), "");
    }

    #[test]
    fn a_directory_has_no_size() {
        let directory = BucketObject::directory("12/");
        assert_eq!(directory.size, None);
        assert!(directory.is_directory);
        assert_eq!(directory.name, "12");
    }

    #[test]
    fn a_file_carries_its_size_and_normalized_key() {
        let file = BucketObject::file("12\\vendas.json", 2048, None);

        assert_eq!(file.key, "12/vendas.json");
        assert_eq!(file.size, Some(2048));
        assert!(!file.is_directory);
    }

    #[test]
    fn the_page_size_is_clamped() {
        assert_eq!(ListOptions::default().effective_limit(), DEFAULT_LIST_LIMIT);
        assert_eq!(
            ListOptions {
                limit: Some(50_000),
                ..ListOptions::default()
            }
            .effective_limit(),
            MAX_LIST_LIMIT
        );
        // Zero produziria uma página vazia para sempre.
        assert_eq!(
            ListOptions {
                limit: Some(0),
                ..ListOptions::default()
            }
            .effective_limit(),
            1
        );
    }

    #[test]
    fn refuses_to_delete_the_root_of_a_destination() {
        for root in ["", " ", "/", ".", "//", "\\"] {
            assert!(
                matches!(assert_deletable(root), Err(StorageError::RootDeletion)),
                "aceitou {root:?}"
            );
        }
    }

    #[test]
    fn accepts_a_real_key() {
        assert_eq!(
            assert_deletable(" 12/vendas.json ").unwrap(),
            "12/vendas.json"
        );
    }

    #[test]
    fn a_provider_with_the_wrong_config_has_no_adapter() {
        let config = StorageConfig::Local(config::LocalConfig::default());
        assert!(matches!(
            explorer_for(&config, StorageProvider::AwsS3, Path::new("storage")),
            Err(StorageError::InvalidConfig)
        ));
    }

    #[test]
    fn the_remote_failure_reaches_the_screen_with_its_message() {
        let error: AppError = StorageError::Backend("S3: AccessDenied".into()).into();
        assert_eq!(error.status(), axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(error.to_string(), "S3: AccessDenied");
    }
}
