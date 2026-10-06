//! Os providers que um armazenamento pode ter.
//!
//! O provider é o que o operador escolhe na tela. Vários deles falam o mesmo
//! protocolo — AWS S3, MinIO, Cloudflare R2 e "outro S3-compatível" usam todos
//! a [`S3Config`](super::config::S3Config) —, mas a distinção importa: o R2
//! assina com a região `auto`, o MinIO exige endereçamento por caminho, e a
//! tela mostra ajuda diferente para cada um.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::config::StorageConfig;
use crate::services::shared::errors::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum StorageProvider {
    Local,
    AwsS3,
    Minio,
    CloudflareR2,
    S3Compatible,
    GoogleGcs,
    AzureBlob,
    Sftp,
}

impl StorageProvider {
    pub const ALL: [Self; 8] = [
        Self::Local,
        Self::AwsS3,
        Self::Minio,
        Self::CloudflareR2,
        Self::S3Compatible,
        Self::GoogleGcs,
        Self::AzureBlob,
        Self::Sftp,
    ];

    /// Valor gravado na coluna `storage_destinations.provider`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::AwsS3 => "aws_s3",
            Self::Minio => "minio",
            Self::CloudflareR2 => "cloudflare_r2",
            Self::S3Compatible => "s3_compatible",
            Self::GoogleGcs => "google_gcs",
            Self::AzureBlob => "azure_blob",
            Self::Sftp => "sftp",
        }
    }

    /// Lê o valor da coluna.
    ///
    /// # Errors
    ///
    /// Valor que não é de nenhum provider conhecido.
    pub fn parse(value: &str) -> AppResult<Self> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.as_str() == value)
            .ok_or_else(|| AppError::validation(format!("Tipo de armazenamento inválido: {value}")))
    }

    /// Nome exibido nas mensagens do backend (auditoria, erros).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "Pasta local",
            Self::AwsS3 => "Amazon S3",
            Self::Minio => "MinIO",
            Self::CloudflareR2 => "Cloudflare R2",
            Self::S3Compatible => "S3-compatível",
            Self::GoogleGcs => "Google Cloud Storage",
            Self::AzureBlob => "Azure Blob Storage",
            Self::Sftp => "SFTP",
        }
    }

    /// A config tem o formato que este provider espera?
    ///
    /// Uma credencial de SFTP gravada num destino marcado como S3 não teria
    /// adapter que a lesse — a recusa acontece no cadastro, e não no primeiro
    /// backup agendado, de madrugada.
    #[must_use]
    pub const fn accepts(self, config: &StorageConfig) -> bool {
        matches!(
            (self, config),
            (Self::Local, StorageConfig::Local(_))
                | (
                    Self::AwsS3 | Self::Minio | Self::CloudflareR2 | Self::S3Compatible,
                    StorageConfig::S3(_)
                )
                | (Self::GoogleGcs, StorageConfig::Gcs(_))
                | (Self::AzureBlob, StorageConfig::AzureBlob(_))
                | (Self::Sftp, StorageConfig::Sftp(_))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::storage::config::{LocalConfig, S3Config, SftpConfig};

    #[test]
    fn o_valor_da_coluna_volta_ao_mesmo_provider() {
        for provider in StorageProvider::ALL {
            assert_eq!(StorageProvider::parse(provider.as_str()).unwrap(), provider);
        }
    }

    #[test]
    fn valor_desconhecido_e_recusado() {
        assert!(StorageProvider::parse("dropbox").is_err());
    }

    #[test]
    fn a_familia_s3_aceita_a_mesma_config() {
        let s3 = StorageConfig::S3(S3Config::default());
        for provider in [
            StorageProvider::AwsS3,
            StorageProvider::Minio,
            StorageProvider::CloudflareR2,
            StorageProvider::S3Compatible,
        ] {
            assert!(provider.accepts(&s3), "{provider:?}");
        }
        assert!(!StorageProvider::Sftp.accepts(&s3));
    }

    #[test]
    fn provider_e_config_de_familias_diferentes_nao_casam() {
        assert!(!StorageProvider::Local.accepts(&StorageConfig::Sftp(SftpConfig::default())));
        assert!(!StorageProvider::Sftp.accepts(&StorageConfig::Local(LocalConfig::default())));
    }
}
