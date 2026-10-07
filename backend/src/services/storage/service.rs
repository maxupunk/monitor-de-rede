//! Cadastro dos armazenamentos.
//!
//! A credencial vive cifrada em `config_encrypted` (XChaCha20, a mesma
//! `ENCRYPTION_KEY` das chaves da VPN) e só é decifrada para montar o adapter.
//! A tela nunca a recebe de volta: ver [`StorageConfig::redacted`].

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use super::{
    config::{S3Config, StorageConfig},
    explorer_for, local_root, StorageExplorer, StorageProvider,
};
use crate::{
    models::storage_destinations,
    services::shared::{
        crypto,
        errors::{AppError, AppResult},
    },
};

/// O que o formulário envia para criar ou editar um destino.
#[derive(Debug, Clone)]
pub struct StorageInput {
    pub name: String,
    pub provider: StorageProvider,
    pub config: StorageConfig,
}

/// Um armazenamento com a config já decifrada.
pub struct Destination {
    pub row: storage_destinations::Model,
    pub provider: StorageProvider,
    pub config: StorageConfig,
}

impl Destination {
    /// Adapter que fala com este destino.
    ///
    /// # Errors
    ///
    /// Config que o adapter recusa.
    pub fn explorer(&self) -> AppResult<Box<dyn StorageExplorer>> {
        Ok(explorer_for(&self.config, self.provider, &local_root())?)
    }
}

/// Lê e decifra uma linha.
///
/// # Errors
///
/// Provider desconhecido, ou config que não decifra (a `ENCRYPTION_KEY` mudou)
/// ou não casa com o formato esperado.
pub fn open(row: storage_destinations::Model) -> AppResult<Destination> {
    let provider = StorageProvider::parse(&row.provider)?;
    let plain = crypto::decrypt(&row.config_encrypted).map_err(|_| {
        AppError::business_rule(format!(
            "Não foi possível ler a credencial de \"{}\" — a ENCRYPTION_KEY mudou desde o cadastro? \
             Edite o armazenamento e informe a credencial de novo.",
            row.name
        ))
    })?;
    let config: StorageConfig = serde_json::from_str(&plain)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("config de armazenamento: {err}")))?;
    Ok(Destination {
        row,
        provider,
        config,
    })
}

/// Todos os armazenamentos, por nome.
///
/// # Errors
///
/// Erro do banco.
pub async fn list<C: ConnectionTrait>(db: &C) -> AppResult<Vec<storage_destinations::Model>> {
    Ok(storage_destinations::Entity::find()
        .order_by_asc(storage_destinations::Column::Name)
        .all(db)
        .await?)
}

/// Um armazenamento pelo id.
///
/// # Errors
///
/// `404` quando não existe.
pub async fn find<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<storage_destinations::Model> {
    storage_destinations::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Armazenamento não encontrado"))
}

/// Um armazenamento pelo id, já decifrado.
///
/// # Errors
///
/// `404` quando não existe; ver [`open`].
pub async fn load<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<Destination> {
    open(find(db, id).await?)
}

/// Cadastra um armazenamento.
///
/// # Errors
///
/// Validação do formulário ou nome repetido.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    input: StorageInput,
) -> AppResult<storage_destinations::Model> {
    let input = normalize(input)?;
    ensure_unique_name(db, &input.name, None).await?;

    let row = storage_destinations::ActiveModel {
        name: Set(input.name),
        provider: Set(input.provider.as_str().to_string()),
        config_encrypted: Set(seal(&input.config)?),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(row)
}

/// Edita um armazenamento. Segredo em branco mantém o gravado.
///
/// # Errors
///
/// `404`, validação do formulário ou nome repetido.
pub async fn update<C: ConnectionTrait>(
    db: &C,
    id: i64,
    mut input: StorageInput,
) -> AppResult<storage_destinations::Model> {
    let current = load(db, id).await?;
    input.config.keep_secrets_from(&current.config);
    let input = normalize(input)?;
    ensure_unique_name(db, &input.name, Some(id)).await?;

    let mut row: storage_destinations::ActiveModel = current.row.into();
    row.name = Set(input.name);
    row.provider = Set(input.provider.as_str().to_string());
    row.config_encrypted = Set(seal(&input.config)?);
    Ok(row.update(db).await?)
}

/// Remove o cadastro. Os arquivos que estão no destino ficam onde estão.
///
/// # Errors
///
/// `404` ou erro do banco.
pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<storage_destinations::Model> {
    let row = find(db, id).await?;
    storage_destinations::Entity::delete_by_id(id)
        .exec(db)
        .await?;
    Ok(row)
}

/// Resultado de um teste de conexão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutcome {
    pub ok: bool,
    pub message: String,
    /// Identidade do servidor (SFTP), para a tela mostrar o que vai gravar.
    pub host_key_fingerprint: Option<String>,
}

/// Testa uma config que ainda está no formulário.
///
/// Com `id`, os segredos em branco vêm do cadastro — é o "Testar" da edição,
/// em que o operador não redigitou a senha.
///
/// # Errors
///
/// Só falhas **daqui** (cadastro inexistente, credencial que não decifra). O
/// destino que não responde é um resultado do teste, com `ok: false`.
pub async fn test_draft<C: ConnectionTrait>(
    db: &C,
    id: Option<i64>,
    provider: StorageProvider,
    mut config: StorageConfig,
) -> AppResult<TestOutcome> {
    if let Some(id) = id {
        config.keep_secrets_from(&load(db, id).await?.config);
    }
    if let Err(error) = validate_config(provider, &config) {
        return Ok(TestOutcome {
            ok: false,
            message: error.to_string(),
            host_key_fingerprint: None,
        });
    }
    let explorer = match explorer_for(&config, provider, &local_root()) {
        Ok(explorer) => explorer,
        Err(error) => {
            return Ok(TestOutcome {
                ok: false,
                message: error.message(),
                host_key_fingerprint: None,
            })
        }
    };
    Ok(run_test(explorer.as_ref()).await)
}

/// Testa um armazenamento já cadastrado e, no SFTP, grava a identidade do
/// servidor se ainda não houver uma.
///
/// # Errors
///
/// `404` ou credencial que não decifra.
pub async fn test_saved<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<TestOutcome> {
    let destination = load(db, id).await?;
    let explorer = destination.explorer()?;
    let outcome = run_test(explorer.as_ref()).await;
    if outcome.ok {
        remember_identity(db, &destination, explorer.as_ref()).await?;
    }
    Ok(outcome)
}

async fn run_test(explorer: &dyn StorageExplorer) -> TestOutcome {
    match explorer.test_connection().await {
        Ok(()) => TestOutcome {
            ok: true,
            message: "Conexão estabelecida e pasta acessível.".to_string(),
            host_key_fingerprint: explorer.observed_identity(),
        },
        Err(error) => TestOutcome {
            ok: false,
            message: error.message(),
            host_key_fingerprint: explorer.observed_identity(),
        },
    }
}

/// Grava a identidade do servidor SFTP vista numa conexão bem-sucedida, se o
/// destino ainda não tinha uma (primeiro uso).
///
/// # Errors
///
/// Erro do banco ou da cifra.
pub async fn remember_identity<C: ConnectionTrait>(
    db: &C,
    destination: &Destination,
    explorer: &dyn StorageExplorer,
) -> AppResult<()> {
    let StorageConfig::Sftp(sftp) = &destination.config else {
        return Ok(());
    };
    if sftp
        .host_key_fingerprint
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(());
    }
    let Some(observed) = explorer.observed_identity() else {
        return Ok(());
    };
    let mut sftp = sftp.clone();
    sftp.host_key_fingerprint = Some(observed);
    let mut row: storage_destinations::ActiveModel = destination.row.clone().into();
    row.config_encrypted = Set(seal(&StorageConfig::Sftp(sftp))?);
    row.update(db).await?;
    Ok(())
}

/// Avisa as telas abertas que a lista mudou (cadastro ou resultado de backup).
pub async fn publish_updated(ctx: &loco_rs::app::AppContext) {
    if let Ok(bus) = crate::services::events::EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(&ctx.db, "storages:updated", serde_json::json!({}))
            .await
        {
            tracing::warn!(%error, "falha ao publicar storages:updated");
        }
    }
}

fn seal(config: &StorageConfig) -> AppResult<String> {
    let plain = serde_json::to_string(config)
        .map_err(|err| AppError::Internal(anyhow::anyhow!("serializar config: {err}")))?;
    crypto::encrypt(&plain)
}

async fn ensure_unique_name<C: ConnectionTrait>(
    db: &C,
    name: &str,
    except: Option<i64>,
) -> AppResult<()> {
    let mut query =
        storage_destinations::Entity::find().filter(storage_destinations::Column::Name.eq(name));
    if let Some(id) = except {
        query = query.filter(storage_destinations::Column::Id.ne(id));
    }
    if query.count(db).await? > 0 {
        return Err(AppError::conflict(format!(
            "Já existe um armazenamento chamado \"{name}\""
        )));
    }
    Ok(())
}

/// Apara o formulário e confere as regras que não dependem do banco.
fn normalize(mut input: StorageInput) -> AppResult<StorageInput> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err(AppError::validation("Informe um nome para o armazenamento"));
    }
    if input.name.chars().count() > 120 {
        return Err(AppError::validation(
            "O nome pode ter no máximo 120 caracteres",
        ));
    }
    validate_config(input.provider, &input.config)?;
    Ok(input)
}

/// Campos obrigatórios de cada provider.
///
/// Conferidos aqui, e não só no primeiro backup: um destino sem bucket
/// cadastrado em silêncio só se revelaria de madrugada, no agendamento.
///
/// # Errors
///
/// O primeiro campo obrigatório ausente, nomeado como a tela o rotula.
pub fn validate_config(provider: StorageProvider, config: &StorageConfig) -> AppResult<()> {
    if !provider.accepts(config) {
        return Err(AppError::validation(format!(
            "Os campos enviados não são de um armazenamento {}",
            provider.label()
        )));
    }
    let missing = |label: &str| {
        Err(AppError::validation(format!(
            "Informe {label} do armazenamento {}",
            provider.label()
        )))
    };
    let blank = |value: Option<&str>| value.is_none_or(|value| value.trim().is_empty());

    match config {
        StorageConfig::Local(local) => {
            super::local::LocalExplorer::new(local, &local_root()).map_err(|_| {
                AppError::validation(
                    "A pasta precisa ficar dentro da raiz dos armazenamentos locais \
                     (sem \"..\" nem caminho absoluto)",
                )
            })?;
        }
        StorageConfig::S3(s3) => {
            if s3.bucket.trim().is_empty() {
                return missing("o bucket");
            }
            if needs_endpoint(provider, s3) {
                return missing("o endpoint");
            }
            if s3.access_key_id.trim().is_empty() {
                return missing("o Access Key ID");
            }
            if blank(s3.secret_access_key.as_deref()) {
                return missing("a Secret Access Key");
            }
        }
        StorageConfig::Gcs(gcs) => {
            if gcs.bucket.trim().is_empty() {
                return missing("o bucket");
            }
            if blank(gcs.credentials_json.as_deref()) {
                return missing("o JSON da conta de serviço");
            }
        }
        StorageConfig::AzureBlob(azure) => {
            if azure.container.trim().is_empty() {
                return missing("o container");
            }
            if blank(azure.connection_string.as_deref()) {
                return missing("a connection string");
            }
        }
        StorageConfig::Sftp(sftp) => {
            if sftp.host.trim().is_empty() {
                return missing("o servidor");
            }
            if sftp.username.trim().is_empty() {
                return missing("o usuário");
            }
            if blank(sftp.password.as_deref()) && blank(sftp.private_key.as_deref()) {
                return missing("a senha ou a chave privada");
            }
        }
    }
    Ok(())
}

/// MinIO, R2 e "outro S3" não têm endereço implícito como a AWS.
fn needs_endpoint(provider: StorageProvider, s3: &S3Config) -> bool {
    provider != StorageProvider::AwsS3
        && s3
            .endpoint
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::storage::config::{LocalConfig, SftpConfig};

    fn input(provider: StorageProvider, config: StorageConfig) -> StorageInput {
        StorageInput {
            name: "  NAS do escritório  ".into(),
            provider,
            config,
        }
    }

    #[test]
    fn o_nome_e_aparado() {
        let ok = normalize(input(
            StorageProvider::Local,
            StorageConfig::Local(LocalConfig::default()),
        ))
        .unwrap();
        assert_eq!(ok.name, "NAS do escritório");
    }

    #[test]
    fn minio_sem_endpoint_e_recusado_e_aws_nao_precisa() {
        let s3 = S3Config {
            bucket: "backups".into(),
            access_key_id: "AKIA".into(),
            secret_access_key: Some("segredo".into()),
            ..S3Config::default()
        };
        let config = StorageConfig::S3(s3);
        assert!(validate_config(StorageProvider::AwsS3, &config).is_ok());
        let error = validate_config(StorageProvider::Minio, &config).unwrap_err();
        assert!(error.to_string().contains("endpoint"), "{error}");
    }

    #[test]
    fn sftp_precisa_de_senha_ou_chave() {
        let config = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "backup".into(),
            ..SftpConfig::default()
        });
        assert!(validate_config(StorageProvider::Sftp, &config).is_err());

        let with_key = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "backup".into(),
            private_key: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            ..SftpConfig::default()
        });
        assert!(validate_config(StorageProvider::Sftp, &with_key).is_ok());
    }

    #[test]
    fn pasta_local_fora_da_raiz_e_recusada_no_cadastro() {
        let config = StorageConfig::Local(LocalConfig {
            base_path: Some("../data".into()),
        });
        assert!(validate_config(StorageProvider::Local, &config).is_err());
        let ok = StorageConfig::Local(LocalConfig {
            base_path: Some("nas/copias".into()),
        });
        assert!(validate_config(StorageProvider::Local, &ok).is_ok());
    }

    #[test]
    fn provider_que_nao_casa_com_a_config_e_recusado() {
        let config = StorageConfig::Local(LocalConfig::default());
        assert!(validate_config(StorageProvider::Sftp, &config).is_err());
    }
}
