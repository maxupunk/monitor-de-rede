//! Credenciais SNMP: o perfil reutilizável (`snmp_profiles`) e a resolução
//! única de "com que credencial falar com este equipamento".
//!
//! # Um caminho só
//!
//! A coleta, os monitores, a descoberta, a identificação do equipamento e o
//! script da VPN perguntam a mesma coisa, e todos passam por aqui:
//!
//! 1. o perfil do dispositivo (`devices.snmp_profile_id`);
//! 2. sem ele, o perfil marcado como padrão (`is_default`);
//! 3. sem nenhum, a comunidade `public` em v2c ([`SnmpCredentials::fallback`])
//!    — o comportamento de fábrica de antes dos perfis, explícito aqui.
//!
//! O cliente SNMP (área Rede) converte o [`SnmpCredentials`] no `SnmpConfig`
//! dele; o núcleo não conhece o cliente.
//!
//! # Segredos
//!
//! Comunidade, senha de autenticação e senha de privacidade são gravadas
//! cifradas ([`crate::shared::crypto`], a mesma `ENCRYPTION_KEY` das chaves da
//! VPN) e só são decifradas para montar a consulta. O `Debug` de
//! [`SnmpCredentials`] as mascara: um `tracing` descuidado não as vaza.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;

use crate::{
    models::{devices, monitors, snmp_profiles},
    shared::{
        crypto,
        errors::{AppError, AppResult},
    },
};

/// Comunidade de fábrica — o padrão de quase todo agente SNMP e o que o
/// sistema usava antes de existir perfil.
pub const FALLBACK_COMMUNITY: &str = "public";

/// Nome do perfil padrão criado quando nenhum existe ([`ensure_default_profile`]).
pub const FALLBACK_PROFILE_NAME: &str = "Padrão (public, v2c)";

/// Tipo do monitor SNMP (`monitors.type`). A Rede registra o checker com ele.
pub const SNMP_MONITOR_KIND: &str = "snmp";

/// Protocolos de autenticação aceitos (USM). São os nomes que o cliente SNMP
/// entende; a tela oferece exatamente estes.
pub const AUTH_PROTOCOLS: [&str; 6] = ["md5", "sha", "sha224", "sha256", "sha384", "sha512"];

/// Protocolos de privacidade aceitos (USM).
pub const PRIV_PROTOCOLS: [&str; 5] = ["des", "3des", "aes128", "aes192", "aes256"];

/// Versão do protocolo SNMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnmpVersion {
    V1,
    V2c,
    V3,
}

impl SnmpVersion {
    /// Aceita `v1`/`1`, `v2c`/`2c`/`2` e `v3`/`3`, sem diferenciar caixa.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "v1" | "1" => Some(Self::V1),
            "v2c" | "2c" | "2" => Some(Self::V2c),
            "v3" | "3" => Some(Self::V3),
            _ => None,
        }
    }

    /// O nome gravado no banco e devolvido pela API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2c => "v2c",
            Self::V3 => "v3",
        }
    }
}

/// Nível de segurança do SNMPv3, deduzido do que foi preenchido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    NoAuthNoPriv,
    AuthNoPriv,
    AuthPriv,
}

impl SecurityLevel {
    /// O nível pelo que está configurado: autenticação e privacidade.
    #[must_use]
    pub const fn of(has_auth: bool, has_priv: bool) -> Self {
        match (has_auth, has_priv) {
            (true, true) => Self::AuthPriv,
            (true, false) => Self::AuthNoPriv,
            _ => Self::NoAuthNoPriv,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoAuthNoPriv => "noAuthNoPriv",
            Self::AuthNoPriv => "authNoPriv",
            Self::AuthPriv => "authPriv",
        }
    }
}

/// A credencial em claro, pronta para montar uma consulta.
#[derive(Clone, PartialEq, Eq)]
pub struct SnmpCredentials {
    pub version: SnmpVersion,
    /// Comunidade (v1/v2c). Vazia no v3.
    pub community: String,
    pub username: Option<String>,
    pub auth_protocol: Option<String>,
    pub auth_key: Option<String>,
    pub priv_protocol: Option<String>,
    pub priv_key: Option<String>,
    pub context_name: Option<String>,
}

impl std::fmt::Debug for SnmpCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mask = |value: &Option<String>| value.as_ref().map(|_| "********");
        f.debug_struct("SnmpCredentials")
            .field("version", &self.version)
            .field(
                "community",
                &(!self.community.is_empty()).then_some("********"),
            )
            .field("username", &self.username)
            .field("auth_protocol", &self.auth_protocol)
            .field("auth_key", &mask(&self.auth_key))
            .field("priv_protocol", &self.priv_protocol)
            .field("priv_key", &mask(&self.priv_key))
            .field("context_name", &self.context_name)
            .finish()
    }
}

impl SnmpCredentials {
    /// Comunidade em v1 ou v2c.
    #[must_use]
    pub fn community(version: SnmpVersion, community: impl Into<String>) -> Self {
        Self {
            version,
            community: community.into(),
            username: None,
            auth_protocol: None,
            auth_key: None,
            priv_protocol: None,
            priv_key: None,
            context_name: None,
        }
    }

    /// `public` em v2c: o que vale quando não há perfil algum.
    #[must_use]
    pub fn fallback() -> Self {
        Self::community(SnmpVersion::V2c, FALLBACK_COMMUNITY)
    }

    /// Nível de segurança do v3 (no v1/v2c, `NoAuthNoPriv`).
    #[must_use]
    pub fn security_level(&self) -> SecurityLevel {
        SecurityLevel::of(self.auth_protocol.is_some(), self.priv_protocol.is_some())
    }

    /// A comunidade, quando a versão usa uma — o que o script da VPN grava no
    /// equipamento. `None` no v3.
    #[must_use]
    pub fn community_text(&self) -> Option<&str> {
        (self.version != SnmpVersion::V3)
            .then_some(self.community.as_str())
            .filter(|value| !value.is_empty())
    }

    /// Confere se a credencial é montável: comunidade no v1/v2c; no v3,
    /// usuário, protocolos conhecidos, senha junto de cada protocolo e
    /// privacidade só com autenticação. As senhas do USM têm no mínimo 8
    /// caracteres (RFC 3414, §11.2) — o agente recusaria uma mais curta com
    /// um erro bem menos claro do que este.
    ///
    /// # Errors
    ///
    /// [`AppError::Validation`] com o que falta, em português.
    pub fn validate(&self) -> AppResult<()> {
        if self.version != SnmpVersion::V3 {
            if self.community.trim().is_empty() {
                return Err(AppError::validation("Informe a comunidade SNMP (v1/v2c)."));
            }
            return Ok(());
        }
        if self
            .username
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(AppError::validation("Informe o usuário SNMPv3."));
        }
        if let Some(protocol) = &self.auth_protocol {
            if !AUTH_PROTOCOLS.contains(&protocol.as_str()) {
                return Err(AppError::validation(format!(
                    "Protocolo de autenticação desconhecido: {protocol}. Use {}.",
                    AUTH_PROTOCOLS.join(", ")
                )));
            }
            validate_usm_key(self.auth_key.as_deref(), "autenticação")?;
        } else if self.auth_key.is_some() {
            return Err(AppError::validation(
                "Escolha o protocolo de autenticação da senha informada.",
            ));
        }
        if let Some(protocol) = &self.priv_protocol {
            if self.auth_protocol.is_none() {
                return Err(AppError::validation(
                    "A privacidade do SNMPv3 exige autenticação (authPriv).",
                ));
            }
            if !PRIV_PROTOCOLS.contains(&protocol.as_str()) {
                return Err(AppError::validation(format!(
                    "Protocolo de privacidade desconhecido: {protocol}. Use {}.",
                    PRIV_PROTOCOLS.join(", ")
                )));
            }
            validate_usm_key(self.priv_key.as_deref(), "privacidade")?;
        } else if self.priv_key.is_some() {
            return Err(AppError::validation(
                "Escolha o protocolo de privacidade da senha informada.",
            ));
        }
        Ok(())
    }

    /// Escreve a credencial na configuração de execução de um monitor SNMP
    /// (os campos que o checker lê), trocando a que estiver lá.
    pub fn write_into(&self, configuration: &mut serde_json::Map<String, Value>) {
        let optional = |value: &Option<String>| {
            value
                .as_ref()
                .map_or(Value::Null, |text| Value::String(text.clone()))
        };
        configuration.insert("version".into(), self.version.as_str().into());
        configuration.insert("community".into(), self.community.clone().into());
        configuration.insert("username".into(), optional(&self.username));
        configuration.insert("authProtocol".into(), optional(&self.auth_protocol));
        configuration.insert("authKey".into(), optional(&self.auth_key));
        configuration.insert("privProtocol".into(), optional(&self.priv_protocol));
        configuration.insert("privKey".into(), optional(&self.priv_key));
        configuration.insert("contextName".into(), optional(&self.context_name));
    }
}

fn validate_usm_key(key: Option<&str>, what: &str) -> AppResult<()> {
    match key {
        None => Err(AppError::validation(format!(
            "Informe a senha de {what} do SNMPv3."
        ))),
        Some(key) if key.chars().count() < 8 => Err(AppError::validation(format!(
            "A senha de {what} do SNMPv3 precisa de pelo menos 8 caracteres."
        ))),
        Some(_) => Ok(()),
    }
}

fn seal(value: Option<&str>) -> AppResult<Option<String>> {
    value.map(crypto::encrypt).transpose()
}

fn open(value: Option<&str>) -> AppResult<Option<String>> {
    value.map(crypto::decrypt).transpose()
}

/// Decifra um perfil.
///
/// # Errors
///
/// Versão gravada desconhecida, ou segredo que não decifra (a
/// `ENCRYPTION_KEY` mudou desde o cadastro).
pub fn decrypt(profile: &snmp_profiles::Model) -> AppResult<SnmpCredentials> {
    let version = SnmpVersion::parse(&profile.version).ok_or_else(|| {
        AppError::BusinessRule(format!(
            "Credencial SNMP \"{}\" com versão inválida: {}",
            profile.name, profile.version
        ))
    })?;
    let unreadable = |_| {
        AppError::BusinessRule(format!(
            "Não foi possível ler a credencial SNMP \"{}\" — a ENCRYPTION_KEY mudou desde o \
             cadastro? Grave os segredos de novo.",
            profile.name
        ))
    };
    Ok(SnmpCredentials {
        version,
        community: open(profile.community_encrypted.as_deref())
            .map_err(unreadable)?
            .unwrap_or_default(),
        username: profile.username.clone(),
        auth_protocol: profile.auth_protocol.clone(),
        auth_key: open(profile.auth_key_encrypted.as_deref()).map_err(unreadable)?,
        priv_protocol: profile.priv_protocol.clone(),
        priv_key: open(profile.priv_key_encrypted.as_deref()).map_err(unreadable)?,
        context_name: profile.context_name.clone(),
    })
}

/// Grava a credencial (validada) num `ActiveModel`, cifrando os segredos e
/// limpando o que não pertence à versão: um perfil que passou de v3 para v2c
/// não guarda senha de usuário nenhum.
///
/// # Errors
///
/// Credencial inválida ([`SnmpCredentials::validate`]) ou falha da cifra.
pub fn apply(row: &mut snmp_profiles::ActiveModel, credentials: &SnmpCredentials) -> AppResult<()> {
    credentials.validate()?;
    let v3 = credentials.version == SnmpVersion::V3;
    row.version = Set(credentials.version.as_str().to_owned());
    row.community_encrypted = Set(if v3 {
        None
    } else {
        seal(Some(credentials.community.as_str()))?
    });
    let only_v3 = |value: &Option<String>| if v3 { value.clone() } else { None };
    row.username = Set(only_v3(&credentials.username));
    row.auth_protocol = Set(only_v3(&credentials.auth_protocol));
    row.auth_key_encrypted = Set(seal(only_v3(&credentials.auth_key).as_deref())?);
    row.priv_protocol = Set(only_v3(&credentials.priv_protocol));
    row.priv_key_encrypted = Set(seal(only_v3(&credentials.priv_key).as_deref())?);
    row.context_name = Set(only_v3(&credentials.context_name));
    Ok(())
}

/// O perfil padrão, se houver.
///
/// # Errors
///
/// Erro do banco.
pub async fn default_profile<C: ConnectionTrait>(
    db: &C,
) -> AppResult<Option<snmp_profiles::Model>> {
    Ok(snmp_profiles::Entity::find()
        .filter(snmp_profiles::Column::IsDefault.eq(true))
        .order_by_asc(snmp_profiles::Column::Id)
        .one(db)
        .await?)
}

/// A credencial de um perfil, pelo id.
///
/// # Errors
///
/// Perfil inexistente (404), ilegível ou erro do banco.
pub async fn for_profile<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<SnmpCredentials> {
    let profile = snmp_profiles::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Credencial SNMP não encontrada"))?;
    decrypt(&profile)
}

/// A credencial com que se fala com o dispositivo — perfil dele, o padrão ou
/// o `public` de fábrica (ver o topo do módulo).
///
/// # Errors
///
/// Perfil ilegível ou erro do banco. Perfil apontado que sumiu não é erro (a
/// FK `RESTRICT` impede, mas uma leitura concorrente a uma troca pode ver o
/// id antigo): cai no padrão.
pub async fn for_device<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
) -> AppResult<SnmpCredentials> {
    if let Some(id) = device.snmp_profile_id {
        if let Some(profile) = snmp_profiles::Entity::find_by_id(id).one(db).await? {
            return decrypt(&profile);
        }
    }
    Ok(default_credentials(db).await?.1)
}

/// A credencial padrão — o perfil marcado (com o id dele) ou, sem nenhum, o
/// `public`/v2c de fábrica (sem id).
///
/// # Errors
///
/// Perfil ilegível ou erro do banco.
pub async fn default_credentials<C: ConnectionTrait>(
    db: &C,
) -> AppResult<(Option<i64>, SnmpCredentials)> {
    match default_profile(db).await? {
        Some(profile) => Ok((Some(profile.id), decrypt(&profile)?)),
        None => Ok((None, SnmpCredentials::fallback())),
    }
}

/// O id do perfil padrão, criando o `public`/v2c quando ainda não há perfil
/// padrão — o que um dispositivo novo com SNMP ligado e sem credencial
/// escolhida recebe.
///
/// # Errors
///
/// Erro do banco ou da cifra.
pub async fn ensure_default_profile<C: ConnectionTrait>(db: &C) -> AppResult<i64> {
    if let Some(profile) = default_profile(db).await? {
        return Ok(profile.id);
    }
    // Um perfil com o nome reservado que perdeu a marca de padrão volta a ser
    // o padrão, em vez de colidir com o UNIQUE do nome.
    if let Some(existing) = snmp_profiles::Entity::find()
        .filter(snmp_profiles::Column::Name.eq(FALLBACK_PROFILE_NAME))
        .one(db)
        .await?
    {
        let mut row: snmp_profiles::ActiveModel = existing.into();
        row.is_default = Set(true);
        return Ok(row.update(db).await?.id);
    }
    let mut row = snmp_profiles::ActiveModel {
        name: Set(FALLBACK_PROFILE_NAME.to_owned()),
        is_default: Set(true),
        ..Default::default()
    };
    apply(&mut row, &SnmpCredentials::fallback())?;
    Ok(row.insert(db).await?.id)
}

/// A credencial que um dispositivo grava: a pedida (que precisa existir) ou,
/// com SNMP ligado e nenhuma pedida, a padrão ([`ensure_default_profile`]).
/// Gravá-la no cadastro, e não só resolvê-la na coleta, deixa visível com
/// que credencial o equipamento fala — e trocar a padrão depois não troca a
/// de quem já estava funcionando.
///
/// # Errors
///
/// Credencial pedida inexistente (422) ou erro do banco.
pub async fn profile_for_device<C: ConnectionTrait>(
    db: &C,
    requested: Option<i64>,
    snmp_enabled: bool,
) -> AppResult<Option<i64>> {
    match requested {
        Some(id) => {
            if snmp_profiles::Entity::find_by_id(id)
                .one(db)
                .await?
                .is_none()
            {
                return Err(AppError::validation("Credencial SNMP não encontrada."));
            }
            Ok(Some(id))
        }
        None if snmp_enabled => Ok(Some(ensure_default_profile(db).await?)),
        None => Ok(None),
    }
}

/// Os perfis que a descoberta tenta, o padrão primeiro. Um perfil ilegível
/// fica de fora (com aviso) em vez de parar a varredura.
///
/// # Errors
///
/// Erro do banco.
pub async fn all_for_discovery<C: ConnectionTrait>(
    db: &C,
) -> AppResult<Vec<(i64, SnmpCredentials)>> {
    let profiles = snmp_profiles::Entity::find()
        .order_by_desc(snmp_profiles::Column::IsDefault)
        .order_by_asc(snmp_profiles::Column::Id)
        .all(db)
        .await?;
    Ok(profiles
        .iter()
        .filter_map(|profile| match decrypt(profile) {
            Ok(credentials) => Some((profile.id, credentials)),
            Err(error) => {
                tracing::warn!(profile_id = profile.id, %error, "credencial SNMP ignorada");
                None
            }
        })
        .collect())
}

/// A configuração com que um monitor roda: num monitor SNMP de dispositivo, a
/// credencial vem do perfil do dispositivo **na hora** — a gravada no monitor
/// não tem segredo, e trocar a credencial do equipamento vale para todos os
/// monitores dele sem regravá-los. Os demais monitores rodam como estão.
///
/// # Errors
///
/// Perfil ilegível ou erro do banco.
pub async fn execution_configuration<C: ConnectionTrait>(
    db: &C,
    monitor: &monitors::Model,
) -> AppResult<Value> {
    let mut configuration = monitor.configuration.clone();
    if !monitor.r#type.eq_ignore_ascii_case(SNMP_MONITOR_KIND) {
        return Ok(configuration);
    }
    let Some(device_id) = monitor.device_id else {
        return Ok(configuration);
    };
    let Some(device) = devices::Entity::find_by_id(device_id).one(db).await? else {
        return Ok(configuration);
    };
    let credentials = for_device(db, &device).await?;
    if let Value::Object(object) = &mut configuration {
        credentials.write_into(object);
    }
    Ok(configuration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::{CoreMigrator as Migrator, MigratorTrait};

    async fn banco() -> sea_orm::DatabaseConnection {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = sea_orm::Database::connect(options)
            .await
            .expect("sqlite em memória");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    fn v3(auth: Option<(&str, &str)>, privacy: Option<(&str, &str)>) -> SnmpCredentials {
        SnmpCredentials {
            version: SnmpVersion::V3,
            community: String::new(),
            username: Some("monitor".into()),
            auth_protocol: auth.map(|(protocol, _)| protocol.into()),
            auth_key: auth.map(|(_, key)| key.into()),
            priv_protocol: privacy.map(|(protocol, _)| protocol.into()),
            priv_key: privacy.map(|(_, key)| key.into()),
            context_name: None,
        }
    }

    async fn perfil(
        db: &sea_orm::DatabaseConnection,
        name: &str,
        credentials: &SnmpCredentials,
        is_default: bool,
    ) -> snmp_profiles::Model {
        let mut row = snmp_profiles::ActiveModel {
            name: Set(name.into()),
            is_default: Set(is_default),
            ..Default::default()
        };
        apply(&mut row, credentials).expect("credencial válida");
        row.insert(db).await.expect("gravar perfil")
    }

    async fn dispositivo(db: &sea_orm::DatabaseConnection, profile: Option<i64>) -> devices::Model {
        devices::ActiveModel {
            name: Set("sw".into()),
            r#type: Set("switch".into()),
            snmp_enabled: Set(true),
            snmp_profile_id: Set(profile),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("gravar dispositivo")
    }

    #[test]
    fn versao_aceita_as_grafias_de_sempre() {
        assert_eq!(SnmpVersion::parse("1"), Some(SnmpVersion::V1));
        assert_eq!(SnmpVersion::parse("v2c"), Some(SnmpVersion::V2c));
        assert_eq!(SnmpVersion::parse(" V3 "), Some(SnmpVersion::V3));
        assert_eq!(SnmpVersion::parse("4"), None);
    }

    #[test]
    fn nivel_de_seguranca_vem_do_que_foi_preenchido() {
        assert_eq!(v3(None, None).security_level(), SecurityLevel::NoAuthNoPriv);
        assert_eq!(
            v3(Some(("sha", "senha-auth")), None).security_level(),
            SecurityLevel::AuthNoPriv
        );
        assert_eq!(
            v3(Some(("sha", "senha-auth")), Some(("aes128", "senha-priv"))).security_level(),
            SecurityLevel::AuthPriv
        );
    }

    #[test]
    fn validacao_explica_o_que_falta() {
        assert!(SnmpCredentials::community(SnmpVersion::V2c, " ")
            .validate()
            .is_err());
        assert!(SnmpCredentials::fallback().validate().is_ok());
        assert!(v3(None, None).validate().is_ok(), "noAuthNoPriv é válido");
        let mut sem_usuario = v3(None, None);
        sem_usuario.username = None;
        assert!(sem_usuario.validate().is_err());
        assert!(v3(Some(("sha", "curta")), None).validate().is_err());
        assert!(v3(Some(("blake", "senha-longa")), None).validate().is_err());
        assert!(
            v3(None, Some(("aes128", "senha-priv"))).validate().is_err(),
            "privacidade sem autenticação"
        );
        assert!(
            v3(Some(("sha", "senha-auth")), Some(("aes999", "senha-priv")))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn debug_nao_mostra_segredo() {
        let texto = format!(
            "{:?}",
            v3(Some(("sha", "senha-auth")), Some(("aes128", "senha-priv")))
        );
        assert!(!texto.contains("senha-auth") && !texto.contains("senha-priv"));
        assert!(!format!("{:?}", SnmpCredentials::fallback()).contains("public"));
    }

    #[tokio::test]
    async fn segredos_ficam_cifrados_e_voltam_iguais() {
        let db = banco().await;
        let credencial = v3(Some(("sha", "senha-auth")), Some(("aes128", "senha-priv")));
        let gravado = perfil(&db, "v3", &credencial, false).await;
        assert_eq!(gravado.version, "v3");
        assert_ne!(gravado.auth_key_encrypted.as_deref(), Some("senha-auth"));
        assert_ne!(gravado.priv_key_encrypted.as_deref(), Some("senha-priv"));
        assert!(gravado.community_encrypted.is_none());
        assert_eq!(decrypt(&gravado).unwrap(), credencial);

        let comunidade = SnmpCredentials::community(SnmpVersion::V1, "segredo");
        let gravado = perfil(&db, "v1", &comunidade, false).await;
        assert_ne!(gravado.community_encrypted.as_deref(), Some("segredo"));
        assert!(gravado.username.is_none());
        assert_eq!(decrypt(&gravado).unwrap(), comunidade);
    }

    #[tokio::test]
    async fn dispositivo_resolve_perfil_proprio_depois_padrao_depois_fabrica() {
        let db = banco().await;
        let sem_perfil = dispositivo(&db, None).await;
        assert_eq!(
            for_device(&db, &sem_perfil).await.unwrap(),
            SnmpCredentials::fallback(),
            "sem perfil algum, public/v2c"
        );

        let padrao = SnmpCredentials::community(SnmpVersion::V2c, "da-casa");
        perfil(&db, "padrão", &padrao, true).await;
        assert_eq!(for_device(&db, &sem_perfil).await.unwrap(), padrao);

        let proprio = v3(Some(("sha256", "senha-auth")), None);
        let id = perfil(&db, "próprio", &proprio, false).await.id;
        let com_perfil = dispositivo(&db, Some(id)).await;
        assert_eq!(for_device(&db, &com_perfil).await.unwrap(), proprio);
    }

    #[tokio::test]
    async fn perfil_padrao_e_criado_uma_vez_so() {
        let db = banco().await;
        let id = ensure_default_profile(&db).await.unwrap();
        assert_eq!(ensure_default_profile(&db).await.unwrap(), id);
        assert_eq!(
            for_profile(&db, id).await.unwrap(),
            SnmpCredentials::fallback()
        );
        assert_eq!(all_for_discovery(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn monitor_snmp_de_dispositivo_recebe_a_credencial_na_hora() {
        let db = banco().await;
        let credencial = v3(Some(("sha", "senha-auth")), Some(("aes128", "senha-priv")));
        let id = perfil(&db, "v3", &credencial, false).await.id;
        let device = dispositivo(&db, Some(id)).await;
        let monitor = monitors::ActiveModel {
            device_id: Set(Some(device.id)),
            name: Set("Interface ether1".into()),
            r#type: Set(SNMP_MONITOR_KIND.into()),
            configuration: Set(serde_json::json!({
                "host": "10.0.0.1", "metric": "traffic", "community": "velha"
            })),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let config = execution_configuration(&db, &monitor).await.unwrap();
        assert_eq!(config["version"], "v3");
        assert_eq!(config["username"], "monitor");
        assert_eq!(config["authKey"], "senha-auth");
        assert_eq!(config["privProtocol"], "aes128");
        assert_eq!(config["community"], "", "a comunidade velha não sobra");
        assert_eq!(config["metric"], "traffic");
        assert!(
            monitor.configuration.get("authKey").is_none(),
            "a configuração gravada continua sem segredo"
        );
    }
}
