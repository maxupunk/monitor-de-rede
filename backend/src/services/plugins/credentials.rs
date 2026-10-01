//! Credenciais de acesso ao equipamento, com a escolha de guardar ou não.
//!
//! O provisionamento de syslog decidiu **não** guardar senha — ela é usada
//! uma vez por aparelho. Plugins são outro caso: rodam de novo, validam em
//! outro dia, são testados pela IA. Por isso a escolha é por credencial:
//!
//! * `vault` — a senha é cifrada (`crypto::encrypt`, `ENCRYPTION_KEY`) e
//!   gravada. Nunca volta para a tela, nem para a IA, nem para o script: é
//!   decifrada só no instante de montar a conexão.
//! * `ask` — nada vai ao banco. A senha é informada por sessão e fica em
//!   memória ([`session_store`]) até expirar.
//!
//! Por dispositivo há no máximo uma credencial de cada tipo (SSH, HTTP,
//! Telnet), com a porta e, opcionalmente, o agente remoto por onde o acesso
//! sai.

use std::{sync::OnceLock, time::Duration};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{
    manifest::TransportKind,
    runtime::{AccessProfile, Credentials},
    transport::Login,
};
use crate::{
    models::{device_credentials, probes},
    services::{
        shared::{
            crypto,
            errors::{AppError, AppResult},
        },
        vpn::secret_store::EphemeralSecretStore,
    },
};

/// Quanto uma senha informada "por sessão" continua valendo.
pub const SESSION_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum SecretStorage {
    /// Cifrada no banco.
    Vault,
    /// Pedida a cada sessão, só em memória.
    Ask,
}

impl SecretStorage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vault => "vault",
            Self::Ask => "ask",
        }
    }

    fn parse(value: &str) -> Self {
        if value == "vault" {
            Self::Vault
        } else {
            Self::Ask
        }
    }
}

/// O que a tela vê. Sem senha, nunca.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct CredentialView {
    pub kind: TransportKind,
    pub username: String,
    pub storage: SecretStorage,
    pub port: u16,
    /// HTTPS em vez de HTTP (só para `http`).
    pub https: bool,
    #[ts(type = "number | null")]
    pub via_probe_id: Option<i64>,
    /// Há senha cifrada guardada.
    pub has_stored_secret: bool,
    /// Há senha de sessão em memória, ainda válida.
    pub session_active: bool,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct CredentialInput {
    pub kind: TransportKind,
    pub username: String,
    /// Vazio mantém a senha guardada (no `vault`).
    #[serde(default)]
    #[ts(optional)]
    pub secret: Option<String>,
    pub storage: SecretStorage,
    #[serde(default)]
    #[ts(optional)]
    pub port: Option<u16>,
    #[serde(default)]
    #[ts(optional)]
    pub https: Option<bool>,
    #[serde(default)]
    #[ts(optional, type = "number | null")]
    pub via_probe_id: Option<i64>,
}

/// O cofre das senhas de sessão. Um por processo — a senha informada na tela
/// vale para a API, nunca para o agendador.
pub fn session_store() -> &'static EphemeralSecretStore {
    static STORE: OnceLock<EphemeralSecretStore> = OnceLock::new();
    STORE.get_or_init(|| EphemeralSecretStore::new(SESSION_TTL))
}

fn session_key(device_id: i64, kind: TransportKind) -> String {
    format!("plugin-credential:{device_id}:{}", kind.as_str())
}

fn https_of(model: &device_credentials::Model) -> bool {
    model
        .extra
        .as_ref()
        .and_then(|extra| extra.get("https"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn port_of(model: &device_credentials::Model, kind: TransportKind) -> u16 {
    model
        .port
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
        .unwrap_or_else(|| {
            if kind == TransportKind::Http && https_of(model) {
                443
            } else {
                kind.default_port()
            }
        })
}

fn to_view(model: &device_credentials::Model) -> AppResult<CredentialView> {
    let kind = TransportKind::parse(&model.kind).map_err(AppError::validation)?;
    Ok(CredentialView {
        kind,
        username: model.username.clone(),
        storage: SecretStorage::parse(&model.storage),
        port: port_of(model, kind),
        https: https_of(model),
        via_probe_id: model.via_probe_id,
        has_stored_secret: model.secret_encrypted.is_some(),
        session_active: session_store().has(&session_key(model.device_id, kind)),
    })
}

/// # Errors
///
/// Erro do banco.
pub async fn list<C: ConnectionTrait>(db: &C, device_id: i64) -> AppResult<Vec<CredentialView>> {
    device_credentials::Entity::find()
        .filter(device_credentials::Column::DeviceId.eq(device_id))
        .order_by_asc(device_credentials::Column::Kind)
        .all(db)
        .await?
        .iter()
        .map(to_view)
        .collect()
}

async fn find<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    kind: TransportKind,
) -> AppResult<Option<device_credentials::Model>> {
    Ok(device_credentials::Entity::find()
        .filter(device_credentials::Column::DeviceId.eq(device_id))
        .filter(device_credentials::Column::Kind.eq(kind.as_str()))
        .one(db)
        .await?)
}

/// Cria ou substitui a credencial de um tipo.
///
/// # Errors
///
/// Usuário vazio, agente inexistente, falha de cifra ou do banco.
pub async fn upsert<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    input: CredentialInput,
) -> AppResult<CredentialView> {
    let username = input.username.trim().to_owned();
    if username.is_empty() || username.len() > 128 {
        return Err(AppError::validation(
            "Informe o usuário de acesso (até 128 caracteres).",
        ));
    }
    if let Some(probe_id) = input.via_probe_id {
        if probes::Entity::find_by_id(probe_id)
            .one(db)
            .await?
            .is_none()
        {
            return Err(AppError::validation("Agente remoto não encontrado."));
        }
    }
    let secret = input.secret.filter(|secret| !secret.is_empty());
    let existing = find(db, device_id, input.kind).await?;
    let secret_encrypted = match input.storage {
        SecretStorage::Vault => match &secret {
            Some(secret) => Some(crypto::encrypt(secret)?),
            None => existing
                .as_ref()
                .and_then(|model| model.secret_encrypted.clone()),
        },
        // Trocar para "pedir a cada sessão" apaga a senha guardada — é o
        // sentido da troca.
        SecretStorage::Ask => None,
    };
    let key = session_key(device_id, input.kind);
    match (input.storage, secret) {
        (SecretStorage::Ask, Some(secret)) => session_store().put(key, secret),
        (SecretStorage::Vault, _) => session_store().forget(&key),
        (SecretStorage::Ask, None) => {}
    }
    let extra = serde_json::json!({ "https": input.https.unwrap_or(false) });
    let port = input.port.map(i32::from);

    let saved = if let Some(model) = existing {
        let mut active: device_credentials::ActiveModel = model.into();
        active.username = Set(username);
        active.secret_encrypted = Set(secret_encrypted);
        active.storage = Set(input.storage.as_str().to_owned());
        active.port = Set(port);
        active.via_probe_id = Set(input.via_probe_id);
        active.extra = Set(Some(extra));
        active.update(db).await?
    } else {
        device_credentials::ActiveModel {
            device_id: Set(device_id),
            kind: Set(input.kind.as_str().to_owned()),
            username: Set(username),
            secret_encrypted: Set(secret_encrypted),
            storage: Set(input.storage.as_str().to_owned()),
            port: Set(port),
            via_probe_id: Set(input.via_probe_id),
            extra: Set(Some(extra)),
            ..Default::default()
        }
        .insert(db)
        .await?
    };
    to_view(&saved)
}

/// Informa a senha de uma credencial `ask` para esta sessão.
///
/// # Errors
///
/// Credencial inexistente ou senha vazia.
pub async fn open_session<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    kind: TransportKind,
    secret: &str,
) -> AppResult<CredentialView> {
    let model = find(db, device_id, kind)
        .await?
        .ok_or_else(|| AppError::not_found("Credencial não cadastrada para este equipamento."))?;
    if secret.is_empty() {
        return Err(AppError::validation("Informe a senha."));
    }
    session_store().put(session_key(device_id, kind), secret.to_owned());
    to_view(&model)
}

/// # Errors
///
/// Erro do banco.
pub async fn delete<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    kind: TransportKind,
) -> AppResult<()> {
    session_store().forget(&session_key(device_id, kind));
    device_credentials::Entity::delete_many()
        .filter(device_credentials::Column::DeviceId.eq(device_id))
        .filter(device_credentials::Column::Kind.eq(kind.as_str()))
        .exec(db)
        .await?;
    Ok(())
}

/// Credenciais prontas para uma execução, e por qual agente sair.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub credentials: Credentials,
    pub via_probe_id: Option<i64>,
}

/// Monta as credenciais dos transportes pedidos.
///
/// HTTP sem credencial é aceito (página pública, porta 80): o script ainda
/// pode ler, só não tem `{{username}}`/`{{password}}` para usar.
///
/// # Errors
///
/// Transporte SSH/Telnet sem credencial, senha `ask` sem sessão aberta, ou
/// falha ao decifrar.
pub async fn resolve<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    transports: &[TransportKind],
) -> AppResult<Resolved> {
    let mut resolved = Resolved::default();
    for &kind in transports {
        let Some(model) = find(db, device_id, kind).await? else {
            if kind == TransportKind::Http {
                resolved.credentials.http = Some(AccessProfile {
                    login: Login {
                        username: String::new(),
                        password: None,
                    },
                    port: kind.default_port(),
                    https: false,
                });
                continue;
            }
            return Err(AppError::business_rule(format!(
                "Cadastre a credencial {} deste equipamento para usar o plugin.",
                kind.as_str().to_uppercase()
            )));
        };
        let password = match SecretStorage::parse(&model.storage) {
            SecretStorage::Vault => model
                .secret_encrypted
                .as_deref()
                .map(crypto::decrypt)
                .transpose()?,
            SecretStorage::Ask => Some(session_store().get(&session_key(device_id, kind)).ok_or_else(
                || {
                    AppError::conflict(format!(
                        "A senha {} deste equipamento é pedida a cada sessão. Informe-a para continuar.",
                        kind.as_str().to_uppercase()
                    ))
                },
            )?),
        };
        let profile = AccessProfile {
            login: Login {
                username: model.username.clone(),
                password,
            },
            port: port_of(&model, kind),
            https: https_of(&model),
        };
        if resolved.via_probe_id.is_none() {
            resolved.via_probe_id = model.via_probe_id;
        }
        match kind {
            TransportKind::Ssh => resolved.credentials.ssh = Some(profile),
            TransportKind::Http => resolved.credentials.http = Some(profile),
            TransportKind::Telnet => resolved.credentials.telnet = Some(profile),
        }
    }
    Ok(resolved)
}
