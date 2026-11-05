//! Configuração tipada de um destino de armazenamento.
//!
//! A coluna `storage_destinations.config_encrypted` guarda um JSON cifrado cujo
//! formato muda com o provider. Aqui ele vira um enum: cada variante carrega
//! exatamente os campos daquele provider, e o compilador impede que o adapter de
//! S3 receba uma config de SFTP.
//!
//! ## Segredos não voltam para a tela
//!
//! [`StorageConfig::redacted`] devolve a config com cada segredo trocado por
//! vazio e diz quais estavam preenchidos; na edição,
//! [`StorageConfig::keep_secrets_from`] repõe o que veio em branco. É o que
//! permite editar o nome de um destino sem redigitar a chave do S3 — e sem que
//! ela jamais viaje de volta ao navegador.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::provider::StorageProvider;

/// Config de um destino, discriminada por `type`.
///
/// `deny_unknown_fields` fica **de fora** de propósito: o `projectId` do GCS é
/// só informativo, e recusar a config inteira por causa de um campo extra
/// tornaria ilegível o que já está gravado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum StorageConfig {
    Local(LocalConfig),
    S3(S3Config),
    Gcs(GcsConfig),
    AzureBlob(AzureConfig),
    Sftp(SftpConfig),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/", optional_fields)]
pub struct LocalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/", optional_fields)]
pub struct S3Config {
    pub bucket: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_path_style: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/", optional_fields)]
pub struct GcsConfig {
    pub bucket: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/", optional_fields)]
pub struct AzureConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_string: Option<String>,
    pub container: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/", optional_fields)]
pub struct SftpConfig {
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_path: Option<String>,
    /// Impressão digital (SHA-256) da chave do servidor, gravada na primeira
    /// conexão bem-sucedida. Dali em diante uma chave diferente é recusada: é
    /// o que impede um intermediário de receber o backup no lugar do NAS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key_fingerprint: Option<String>,
}

/// Porta default do SSH, usada quando a config não informa outra.
pub const DEFAULT_SFTP_PORT: u16 = 22;

impl StorageConfig {
    /// Prefixo dentro do destino, normalizado sem barras nas pontas.
    ///
    /// Os três providers de objeto chamam isso de `prefix`; o local e o SFTP
    /// chamam de `basePath`. É o mesmo conceito.
    #[must_use]
    pub fn prefix(&self) -> String {
        let raw = match self {
            Self::Local(config) => config.base_path.as_deref(),
            Self::S3(config) => config.prefix.as_deref(),
            Self::Gcs(config) => config.prefix.as_deref(),
            Self::AzureBlob(config) => config.prefix.as_deref(),
            Self::Sftp(config) => config.base_path.as_deref(),
        };

        normalize_path(raw.unwrap_or_default())
    }

    /// O destino guarda os arquivos fora desta máquina?
    #[must_use]
    pub const fn is_remote(&self) -> bool {
        !matches!(self, Self::Local(_))
    }

    /// A config sem os segredos, e o nome de cada segredo que estava
    /// preenchido — a tela mostra "definida" no lugar do valor.
    #[must_use]
    pub fn redacted(&self) -> (Self, Vec<&'static str>) {
        let mut copy = self.clone();
        let mut present = Vec::new();
        for (name, slot) in copy.secret_slots() {
            if slot
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                present.push(name);
            }
            *slot = None;
        }
        (copy, present)
    }

    /// Repõe, a partir da config gravada, cada segredo que chegou em branco.
    ///
    /// Só entre configs do mesmo provider: trocar de S3 para SFTP é um destino
    /// novo, e herdar a senha do anterior seria cruzar credenciais.
    pub fn keep_secrets_from(&mut self, previous: &Self) {
        if std::mem::discriminant(self) != std::mem::discriminant(previous) {
            return;
        }
        let mut previous = previous.clone();
        let old: Vec<(&'static str, Option<String>)> = previous
            .secret_slots()
            .into_iter()
            .map(|(name, slot)| (name, slot.take()))
            .collect();
        let replaced = self.replaced_auth_method();
        for (name, slot) in self.secret_slots() {
            if replaced.contains(&name) {
                continue;
            }
            if slot.as_deref().is_none_or(|value| value.trim().is_empty()) {
                slot.clone_from(
                    &old.iter()
                        .find(|(old_name, _)| *old_name == name)
                        .and_then(|(_, value)| value.clone()),
                );
            }
        }
        // A identidade do servidor vale para aquele endereço. Apontar o
        // destino para outro host exige conhecer a chave dele de novo.
        if let (Self::Sftp(new), Self::Sftp(old)) = (self, &previous) {
            if new.host.trim() != old.host.trim() || new.port != old.port {
                new.host_key_fingerprint = None;
            }
        }
    }

    /// Segredos que **não** devem ser herdados porque o operador trocou o
    /// método de autenticação do SFTP.
    ///
    /// Quem informa só a senha passou a autenticar por senha: herdar a chave
    /// gravada a faria ser tentada primeiro, e o servidor que não a aceita mais
    /// derrubaria o login. O inverso vale para quem informa só a chave.
    fn replaced_auth_method(&self) -> &'static [&'static str] {
        let Self::Sftp(sftp) = self else {
            return &[];
        };
        let filled = |value: Option<&str>| value.is_some_and(|value| !value.trim().is_empty());
        match (
            filled(sftp.password.as_deref()),
            filled(sftp.private_key.as_deref()),
        ) {
            (true, false) => &["privateKey", "passphrase"],
            (false, true) => &["password"],
            _ => &[],
        }
    }

    /// Cada campo secreto, pelo nome que a tela usa.
    fn secret_slots(&mut self) -> Vec<(&'static str, &mut Option<String>)> {
        match self {
            Self::Local(_) => Vec::new(),
            Self::S3(config) => vec![("secretAccessKey", &mut config.secret_access_key)],
            Self::Gcs(config) => vec![("credentialsJson", &mut config.credentials_json)],
            Self::AzureBlob(config) => vec![("connectionString", &mut config.connection_string)],
            Self::Sftp(config) => vec![
                ("password", &mut config.password),
                ("privateKey", &mut config.private_key),
                ("passphrase", &mut config.passphrase),
            ],
        }
    }
}

/// Caminho sem `\`, sem `./` inicial e sem barras nas pontas.
///
/// É o `normalizePath` do `bucket_explorer_service`. Uma chave de objeto nunca
/// tem barra à esquerda, e o migrador traz caminhos gravados no Windows com `\`.
#[must_use]
pub fn normalize_path(value: &str) -> String {
    let replaced = value.replace('\\', "/");
    let trimmed = replaced.trim();
    let without_dot = trimmed.strip_prefix("./").unwrap_or(trimmed);

    without_dot.trim_matches('/').to_string()
}

/// Junta prefixo e chave, pulando o vazio dos dois lados.
#[must_use]
pub fn join_key(prefix: &str, key: &str) -> String {
    let prefix = normalize_path(prefix);
    let key = normalize_path(key);

    match (prefix.is_empty(), key.is_empty()) {
        (true, _) => key,
        (false, true) => prefix,
        (false, false) => format!("{prefix}/{key}"),
    }
}

/// Remove o prefixo do começo de uma chave absoluta do destino.
///
/// É como uma chave vira o `file_path` relativo que `backups` guarda.
#[must_use]
pub fn strip_prefix(prefix: &str, key: &str) -> String {
    let prefix = normalize_path(prefix);
    let key = normalize_path(key);

    if prefix.is_empty() {
        return key;
    }

    key.strip_prefix(&format!("{prefix}/"))
        .map_or(key.clone(), ToString::to_string)
}

/// Região efetiva de um destino S3-compatível.
///
/// Porte do `S3ConfigService.resolveRegion`. O Cloudflare R2 usa `auto`; os
/// demais caem em `us-east-1`. Não é cosmético: o SDK assina a requisição com a
/// região, e assinar com a errada produz um 403 que parece credencial inválida.
#[must_use]
pub fn resolve_s3_region(config: &S3Config, provider: Option<StorageProvider>) -> String {
    if let Some(region) = config.region.as_deref().map(str::trim) {
        if !region.is_empty() {
            return region.to_string();
        }
    }

    let endpoint = config.endpoint.as_deref().unwrap_or_default();
    let is_r2 = provider == Some(StorageProvider::CloudflareR2)
        || endpoint.contains(".r2.cloudflarestorage.com");

    if is_r2 {
        "auto".to_string()
    } else {
        "us-east-1".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_each_provider_into_its_own_variant() {
        let cases = [
            json!({ "type": "local", "basePath": "backups" }),
            json!({ "type": "s3", "bucket": "b", "accessKeyId": "k", "secretAccessKey": "s" }),
            json!({ "type": "gcs", "bucket": "b" }),
            json!({ "type": "azure_blob", "container": "c", "connectionString": "x" }),
            json!({ "type": "sftp", "host": "h", "username": "u" }),
        ];
        let providers = [
            StorageProvider::Local,
            StorageProvider::AwsS3,
            StorageProvider::GoogleGcs,
            StorageProvider::AzureBlob,
            StorageProvider::Sftp,
        ];

        for (raw, provider) in cases.into_iter().zip(providers) {
            let config: StorageConfig = serde_json::from_value(raw.clone())
                .unwrap_or_else(|err| panic!("não parseou {raw}: {err}"));
            assert!(provider.accepts(&config), "{raw}");
        }
    }

    #[test]
    fn the_redacted_config_carries_no_secret() {
        let config = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "backup".into(),
            password: Some("s3nh4".into()),
            private_key: Some("   ".into()),
            ..SftpConfig::default()
        });
        let (redacted, present) = config.redacted();

        assert_eq!(present, vec!["password"]);
        let StorageConfig::Sftp(sftp) = redacted else {
            panic!("mudou de variante");
        };
        assert_eq!(sftp.password, None);
        assert_eq!(sftp.host, "nas");
        assert!(!serde_json::to_string(&sftp).unwrap().contains("s3nh4"));
    }

    #[test]
    fn a_blank_secret_on_edit_keeps_the_stored_one() {
        let stored = StorageConfig::S3(S3Config {
            bucket: "b".into(),
            secret_access_key: Some("segredo".into()),
            ..S3Config::default()
        });
        let mut edited = StorageConfig::S3(S3Config {
            bucket: "outro".into(),
            secret_access_key: Some(String::new()),
            ..S3Config::default()
        });
        edited.keep_secrets_from(&stored);

        let StorageConfig::S3(s3) = edited else {
            panic!("mudou de variante");
        };
        assert_eq!(s3.secret_access_key.as_deref(), Some("segredo"));
        assert_eq!(s3.bucket, "outro");
    }

    #[test]
    fn a_new_secret_on_edit_replaces_the_stored_one() {
        let stored = StorageConfig::AzureBlob(AzureConfig {
            container: "c".into(),
            connection_string: Some("antiga".into()),
            ..AzureConfig::default()
        });
        let mut edited = StorageConfig::AzureBlob(AzureConfig {
            container: "c".into(),
            connection_string: Some("nova".into()),
            ..AzureConfig::default()
        });
        edited.keep_secrets_from(&stored);

        let StorageConfig::AzureBlob(azure) = edited else {
            panic!("mudou de variante");
        };
        assert_eq!(azure.connection_string.as_deref(), Some("nova"));
    }

    #[test]
    fn switching_provider_family_inherits_nothing() {
        let stored = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "u".into(),
            password: Some("s3nh4".into()),
            ..SftpConfig::default()
        });
        let mut edited = StorageConfig::S3(S3Config {
            bucket: "b".into(),
            ..S3Config::default()
        });
        edited.keep_secrets_from(&stored);

        let StorageConfig::S3(s3) = edited else {
            panic!("mudou de variante");
        };
        assert_eq!(s3.secret_access_key, None);
    }

    #[test]
    fn switching_sftp_from_key_to_password_drops_the_key() {
        let stored = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "u".into(),
            private_key: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            passphrase: Some("frase".into()),
            ..SftpConfig::default()
        });
        let mut edited = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "u".into(),
            password: Some("s3nh4".into()),
            ..SftpConfig::default()
        });
        edited.keep_secrets_from(&stored);

        let StorageConfig::Sftp(sftp) = edited else {
            panic!("mudou de variante");
        };
        assert_eq!(sftp.password.as_deref(), Some("s3nh4"));
        assert_eq!(sftp.private_key, None);
        assert_eq!(sftp.passphrase, None);
    }

    #[test]
    fn editing_sftp_without_touching_the_auth_keeps_both() {
        let stored = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "u".into(),
            private_key: Some("chave".into()),
            passphrase: Some("frase".into()),
            ..SftpConfig::default()
        });
        let mut edited = StorageConfig::Sftp(SftpConfig {
            host: "nas".into(),
            username: "outro".into(),
            ..SftpConfig::default()
        });
        edited.keep_secrets_from(&stored);

        let StorageConfig::Sftp(sftp) = edited else {
            panic!("mudou de variante");
        };
        assert_eq!(sftp.private_key.as_deref(), Some("chave"));
        assert_eq!(sftp.passphrase.as_deref(), Some("frase"));
    }

    #[test]
    fn moving_the_sftp_host_forgets_the_server_identity() {
        let stored = StorageConfig::Sftp(SftpConfig {
            host: "nas-antigo".into(),
            username: "u".into(),
            host_key_fingerprint: Some("SHA256:abc".into()),
            ..SftpConfig::default()
        });
        let mut same_host = stored.clone();
        same_host.keep_secrets_from(&stored);
        let StorageConfig::Sftp(kept) = same_host else {
            panic!("mudou de variante");
        };
        assert_eq!(kept.host_key_fingerprint.as_deref(), Some("SHA256:abc"));

        let mut moved = StorageConfig::Sftp(SftpConfig {
            host: "nas-novo".into(),
            username: "u".into(),
            host_key_fingerprint: Some("SHA256:abc".into()),
            ..SftpConfig::default()
        });
        moved.keep_secrets_from(&stored);
        let StorageConfig::Sftp(moved) = moved else {
            panic!("mudou de variante");
        };
        assert_eq!(moved.host_key_fingerprint, None);
    }

    #[test]
    fn keeps_reading_a_config_with_extra_fields() {
        // `projectId` do GCS é só informativo. Recusar a config inteira por
        // causa dele tornaria ilegível o que já está gravado.
        let config: StorageConfig = serde_json::from_value(json!({
            "type": "gcs",
            "bucket": "backups",
            "projectId": "meu-projeto",
            "campoQueNinguemConhece": 42,
        }))
        .expect("config com campo extra continua legível");

        assert!(StorageProvider::GoogleGcs.accepts(&config));
    }

    #[test]
    fn normalizes_windows_separators_and_edges() {
        assert_eq!(normalize_path("12\\vendas.sql.gz"), "12/vendas.sql.gz");
        assert_eq!(normalize_path("/backups/"), "backups");
        assert_eq!(normalize_path("./backups"), "backups");
        assert_eq!(normalize_path("  /a/b/  "), "a/b");
        assert_eq!(normalize_path(""), "");
        assert_eq!(normalize_path("///"), "");
    }

    #[test]
    fn joins_a_prefix_without_leaving_empty_segments() {
        assert_eq!(join_key("dumps", "12/a.gz"), "dumps/12/a.gz");
        assert_eq!(join_key("", "12/a.gz"), "12/a.gz");
        assert_eq!(join_key("dumps", ""), "dumps");
        assert_eq!(join_key("/dumps/", "/12/a.gz"), "dumps/12/a.gz");
        assert_eq!(join_key("", ""), "");
    }

    #[test]
    fn strips_the_prefix_to_get_the_stored_backup_path() {
        assert_eq!(strip_prefix("dumps", "dumps/12/a.gz"), "12/a.gz");
        assert_eq!(strip_prefix("", "12/a.gz"), "12/a.gz");
        // Chave que nao esta' sob o prefixo volta inteira, e nao cortada pelo
        // tamanho — cortar produziria um caminho invalido silenciosamente.
        assert_eq!(strip_prefix("dumps", "outro/12/a.gz"), "outro/12/a.gz");
        // Prefixo que e' so' um pedaco do primeiro segmento nao casa.
        assert_eq!(strip_prefix("dum", "dumps/a.gz"), "dumps/a.gz");
    }

    #[test]
    fn the_prefix_of_each_provider_comes_from_its_own_field() {
        let local = StorageConfig::Local(LocalConfig {
            base_path: Some("/srv/backups/".to_string()),
        });
        assert_eq!(local.prefix(), "srv/backups");

        let s3 = StorageConfig::S3(S3Config {
            bucket: "b".to_string(),
            prefix: Some("dumps/".to_string()),
            ..S3Config::default()
        });
        assert_eq!(s3.prefix(), "dumps");

        let sftp = StorageConfig::Sftp(SftpConfig {
            host: "h".to_string(),
            username: "u".to_string(),
            base_path: Some("/home/tester/backups".to_string()),
            ..SftpConfig::default()
        });
        assert_eq!(sftp.prefix(), "home/tester/backups");
    }

    #[test]
    fn only_local_is_not_remote() {
        assert!(!StorageConfig::Local(LocalConfig::default()).is_remote());
        assert!(StorageConfig::S3(S3Config::default()).is_remote());
        assert!(StorageConfig::Sftp(SftpConfig::default()).is_remote());
    }

    #[test]
    fn cloudflare_r2_defaults_to_the_auto_region() {
        // Assinar com a regiao errada produz um 403 que parece credencial
        // invalida — e' o erro mais caro de diagnosticar deste recurso.
        let by_provider =
            resolve_s3_region(&S3Config::default(), Some(StorageProvider::CloudflareR2));
        assert_eq!(by_provider, "auto");

        let by_endpoint = resolve_s3_region(
            &S3Config {
                endpoint: Some("https://abc.r2.cloudflarestorage.com".to_string()),
                ..S3Config::default()
            },
            None,
        );
        assert_eq!(by_endpoint, "auto");
    }

    #[test]
    fn the_other_providers_default_to_us_east_1() {
        assert_eq!(
            resolve_s3_region(&S3Config::default(), Some(StorageProvider::Minio)),
            "us-east-1"
        );
    }

    #[test]
    fn an_explicit_region_always_wins() {
        assert_eq!(
            resolve_s3_region(
                &S3Config {
                    region: Some(" sa-east-1 ".to_string()),
                    ..S3Config::default()
                },
                Some(StorageProvider::CloudflareR2)
            ),
            "sa-east-1"
        );
        // Regiao em branco conta como ausente.
        assert_eq!(
            resolve_s3_region(
                &S3Config {
                    region: Some("   ".to_string()),
                    ..S3Config::default()
                },
                None
            ),
            "us-east-1"
        );
    }
}
