//! Serialização dos armazenamentos.
//!
//! Nenhum tipo aqui carrega segredo: a config sai pela
//! [`StorageConfig::redacted`], e o que estava preenchido vira só o nome do
//! campo em `secretsSet`.

use chrono::Utc;
use serde::Serialize;
use ts_rs::TS;

use crate::{
    models::storage_destinations,
    services::storage::{
        backups::{BackupEntry, RunOutcome},
        config::{join_key, DEFAULT_SFTP_PORT},
        local_root, schedule,
        service::{Destination, TestOutcome},
        BucketObject, ListPage, StorageConfig, StorageProvider,
    },
};

/// Uma linha da lista.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageDestinationResponse {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub provider: StorageProvider,
    /// Para onde vão os arquivos, num texto só: `s3://bucket/prefixo`,
    /// `backup@nas:22/srv/copias`. Nulo quando a credencial não decifra.
    pub target: Option<String>,
    pub backup_enabled: bool,
    pub backup_interval_hours: i32,
    pub backup_retention: i32,
    pub last_backup_at: Option<String>,
    /// `success` | `failed`.
    pub last_backup_status: Option<String>,
    pub last_backup_error: Option<String>,
    pub next_backup_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl StorageDestinationResponse {
    /// Monta a linha. `config` é `None` quando a credencial não pôde ser lida.
    #[must_use]
    pub fn new(row: &storage_destinations::Model, config: Option<&StorageConfig>) -> Self {
        Self {
            id: row.id,
            name: row.name.clone(),
            provider: StorageProvider::parse(&row.provider).unwrap_or(StorageProvider::Local),
            target: config.map(target_of),
            backup_enabled: row.backup_enabled,
            backup_interval_hours: row.backup_interval_hours,
            backup_retention: row.backup_retention,
            last_backup_at: row.last_backup_at.map(|at| at.to_rfc3339()),
            last_backup_status: row.last_backup_status.clone(),
            last_backup_error: row.last_backup_error.clone(),
            next_backup_at: schedule::next_run(row, Utc::now()).map(|at| at.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

/// O cadastro para o formulário de edição.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageDestinationDetail {
    #[serde(flatten)]
    pub summary: StorageDestinationResponse,
    /// A config sem os segredos.
    pub config: StorageConfig,
    /// Segredos que estão gravados (`password`, `secretAccessKey`…).
    pub secrets_set: Vec<String>,
}

impl From<&Destination> for StorageDestinationDetail {
    fn from(destination: &Destination) -> Self {
        let (config, secrets) = destination.config.redacted();
        Self {
            summary: StorageDestinationResponse::new(&destination.row, Some(&destination.config)),
            config,
            secrets_set: secrets.into_iter().map(ToString::to_string).collect(),
        }
    }
}

/// Informação que o formulário precisa e que não é de nenhum destino.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StoragesMetaResponse {
    /// Pasta do servidor sob a qual ficam os armazenamentos locais.
    pub local_root: String,
}

impl StoragesMetaResponse {
    #[must_use]
    pub fn current() -> Self {
        Self {
            local_root: local_root().display().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageTestResponse {
    pub ok: bool,
    pub message: String,
    pub host_key_fingerprint: Option<String>,
}

impl From<TestOutcome> for StorageTestResponse {
    fn from(outcome: TestOutcome) -> Self {
        Self {
            ok: outcome.ok,
            message: outcome.message,
            host_key_fingerprint: outcome.host_key_fingerprint,
        }
    }
}

/// Um item do explorador.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageObjectResponse {
    pub key: String,
    pub name: String,
    #[ts(type = "number | null")]
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub is_directory: bool,
}

impl From<BucketObject> for StorageObjectResponse {
    fn from(object: BucketObject) -> Self {
        Self {
            key: object.key,
            name: object.name,
            size: object.size,
            last_modified: object.last_modified,
            is_directory: object.is_directory,
        }
    }
}

/// Uma página do explorador.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageBrowseResponse {
    pub path: String,
    pub objects: Vec<StorageObjectResponse>,
    pub next_cursor: Option<String>,
}

impl StorageBrowseResponse {
    #[must_use]
    pub fn new(path: String, page: ListPage) -> Self {
        Self {
            path,
            objects: page.objects.into_iter().map(Into::into).collect(),
            next_cursor: page.next_cursor.filter(|_| page.is_truncated),
        }
    }
}

/// Uma cópia de backup guardada no destino.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageBackupResponse {
    pub key: String,
    pub name: String,
    #[ts(type = "number | null")]
    pub size: Option<i64>,
    pub last_modified: Option<String>,
}

impl From<BackupEntry> for StorageBackupResponse {
    fn from(entry: BackupEntry) -> Self {
        Self {
            key: entry.key,
            name: entry.name,
            size: entry.size,
            last_modified: entry.last_modified,
        }
    }
}

/// Resultado de "Fazer backup agora".
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StorageBackupRunResponse {
    pub backup: StorageBackupResponse,
    /// Cópias antigas apagadas pela retenção.
    #[ts(type = "number")]
    pub pruned: usize,
}

impl From<RunOutcome> for StorageBackupRunResponse {
    fn from(outcome: RunOutcome) -> Self {
        Self {
            backup: outcome.entry.into(),
            pruned: outcome.pruned,
        }
    }
}

/// Para onde vão os arquivos, num texto só.
fn target_of(config: &StorageConfig) -> String {
    let prefix = config.prefix();
    match config {
        StorageConfig::Local(_) => {
            let root = local_root().display().to_string().replace('\\', "/");
            join_key(&root, &prefix)
        }
        StorageConfig::S3(s3) => format!("s3://{}", join_key(s3.bucket.trim(), &prefix)),
        StorageConfig::Gcs(gcs) => format!("gs://{}", join_key(gcs.bucket.trim(), &prefix)),
        StorageConfig::AzureBlob(azure) => join_key(azure.container.trim(), &prefix),
        StorageConfig::Sftp(sftp) => format!(
            "{}@{}:{}/{}",
            sftp.username.trim(),
            sftp.host.trim(),
            sftp.port.unwrap_or(DEFAULT_SFTP_PORT),
            prefix
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::storage::config::{S3Config, SftpConfig};

    #[test]
    fn o_destino_s3_aparece_como_url() {
        let config = StorageConfig::S3(S3Config {
            bucket: "copias".into(),
            prefix: Some("/rede/".into()),
            ..S3Config::default()
        });
        assert_eq!(target_of(&config), "s3://copias/rede");
    }

    #[test]
    fn o_destino_sftp_mostra_usuario_host_e_porta() {
        let config = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "backup".into(),
            base_path: Some("/srv/copias".into()),
            ..SftpConfig::default()
        });
        assert_eq!(target_of(&config), "backup@nas:22/srv/copias");
    }
}
