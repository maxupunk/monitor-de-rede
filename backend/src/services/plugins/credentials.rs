//! Credenciais de acesso ao equipamento — sem senha no banco.
//!
//! É o lugar único das credenciais do equipamento: plugins rodam de novo,
//! validam em outro dia, são testados pela IA, e a ativação do syslog
//! ([`crate::services::syslog::login`]) usa a mesma credencial. Senha de
//! equipamento **nunca** é gravada (ADR 016); há dois modos:
//!
//! * `ask` — o banco guarda usuário, porta e agente. A senha é pedida na
//!   janela do acesso, quando faz falta, e fica em memória
//!   ([`session_store`]) por 30 minutos.
//! * `key` — o acesso dedicado (ADR 016): a senha do administrador foi usada
//!   uma vez para instalar no equipamento uma chave exclusiva do NetMonitor, e
//!   só a chave privada, cifrada, é guardada. Nasce só por
//!   [`super::dedicated_access`]; o formulário comum não a cria nem a troca.
//!
//! Sem senha guardada, o que roda sozinho (agendador, lote de frota) só
//! alcança equipamento com acesso dedicado: a senha da janela vale para a API,
//! nunca para o agendador.
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
    dedicated_access::DedicatedAccess,
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
    /// Pedida na janela do acesso, só em memória.
    Ask,
    /// Chave do acesso dedicado, cifrada no banco (ADR 016).
    Key,
}

impl SecretStorage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Key => "key",
        }
    }

    fn parse(value: &str) -> Self {
        if value == "key" {
            Self::Key
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
    /// O acesso dedicado instalado no equipamento (só em `key`). Leva a chave
    /// pública e as impressões digitais — nunca a chave privada.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedicated: Option<DedicatedAccess>,
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

/// O acesso dedicado guardado em `extra.dedicated`, se houver.
fn dedicated_of(model: &device_credentials::Model) -> Option<DedicatedAccess> {
    model
        .extra
        .as_ref()
        .and_then(|extra| extra.get("dedicated"))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn is_dedicated(model: &device_credentials::Model) -> bool {
    SecretStorage::parse(&model.storage) == SecretStorage::Key
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
        dedicated: dedicated_of(model),
    })
}

/// O login pronto para uso agora: a senha informada na janela (sessão) ou a
/// chave do acesso dedicado, com a chave de host fixada.
///
/// SSH/Telnet sem a senha da sessão é erro aqui — a tela abre a janela. HTTP
/// segue sem senha: a página pode ser pública (a credencial só guarda porta e
/// HTTPS), e o plugin que precisa de login recebe a recusa do equipamento.
fn login_of(model: &device_credentials::Model, kind: TransportKind) -> AppResult<Login> {
    match SecretStorage::parse(&model.storage) {
        SecretStorage::Key => Ok(Login {
            username: model.username.clone(),
            password: None,
            private_key: model
                .secret_encrypted
                .as_deref()
                .map(crypto::decrypt)
                .transpose()?,
            host_key: dedicated_of(model).and_then(|dedicated| dedicated.host_key),
        }),
        SecretStorage::Ask => {
            let password = session_store().get(&session_key(model.device_id, kind));
            if password.is_none() && kind != TransportKind::Http {
                return Err(session_missing(model.device_id, kind));
            }
            Ok(Login::with_password(model.username.clone(), password))
        }
    }
}

/// O erro único de "falta o acesso": a tela o reconhece pelo código e abre o
/// formulário do acesso daquele transporte (ver [`AppError::MissingCredential`]).
#[must_use]
pub fn missing_credential(device_id: i64, kind: TransportKind, message: String) -> AppError {
    AppError::MissingCredential {
        device_id,
        transport: kind.as_str(),
        message,
    }
}

/// `(device_id, transporte)` de um erro de acesso faltando — para quem guarda
/// a falha (lote de frota) dizer à tela qual acesso pedir.
#[must_use]
pub fn missing_access_of(error: &AppError) -> Option<(i64, TransportKind)> {
    match error {
        AppError::MissingCredential {
            device_id,
            transport,
            ..
        } => TransportKind::parse(transport)
            .ok()
            .map(|kind| (*device_id, kind)),
        _ => None,
    }
}

fn session_missing(device_id: i64, kind: TransportKind) -> AppError {
    missing_credential(
        device_id,
        kind,
        format!(
            "A senha {} deste equipamento não fica guardada. Informe-a para continuar (vale 30 \
             minutos).",
            kind.as_str().to_uppercase()
        ),
    )
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
/// Usuário vazio, agente inexistente, troca indevida do acesso dedicado ou
/// erro do banco.
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
    let existing_dedicated = existing
        .as_ref()
        .filter(|model| is_dedicated(model))
        .and_then(dedicated_of);
    match (existing_dedicated.is_some(), input.storage) {
        // Editar porta ou agente de um acesso dedicado vale; trocar a chave
        // por uma digitada, não.
        (true, SecretStorage::Key) if secret.is_some() => {
            return Err(AppError::validation(
                "A chave do acesso dedicado não é digitada: use \"Rotacionar chave\".",
            ))
        }
        (true, SecretStorage::Key) | (false, SecretStorage::Ask) => {}
        (true, SecretStorage::Ask) => {
            return Err(AppError::validation(
                "Este equipamento usa o acesso dedicado do NetMonitor. Revogue-o antes de \
                 voltar para senha — assim a chave sai do equipamento também.",
            ))
        }
        (false, SecretStorage::Key) => {
            return Err(AppError::validation(
                "O acesso dedicado é criado pelo botão \"Criar acesso dedicado\", com a senha \
                 do administrador informada uma vez.",
            ))
        }
    }
    // Só a chave do acesso dedicado vai ao banco; a senha digitada abre a
    // sessão em memória.
    let secret_encrypted = match input.storage {
        SecretStorage::Ask => None,
        SecretStorage::Key => existing
            .as_ref()
            .and_then(|model| model.secret_encrypted.clone()),
    };
    let key = session_key(device_id, input.kind);
    match (input.storage, secret) {
        (SecretStorage::Ask, Some(secret)) => session_store().put(key, secret),
        (SecretStorage::Key, _) => session_store().forget(&key),
        (SecretStorage::Ask, None) => {}
    }
    let mut extra = serde_json::json!({ "https": input.https.unwrap_or(false) });
    if let Some(dedicated) = existing_dedicated {
        extra["dedicated"] = serde_json::to_value(dedicated)
            .map_err(|error| AppError::Internal(anyhow::anyhow!("acesso dedicado: {error}")))?;
    }
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
    if find(db, device_id, kind)
        .await?
        .is_some_and(|model| is_dedicated(&model))
    {
        // Apagar só aqui deixaria a chave esquecida no equipamento.
        return Err(AppError::business_rule(
            "Este é o acesso dedicado do NetMonitor: use \"Revogar acesso\", que tira a chave \
             do equipamento também.",
        ));
    }
    forget(db, device_id, kind).await
}

/// Apaga a credencial do tipo, seja qual for — para a revogação do acesso
/// dedicado, que já cuidou do equipamento.
///
/// # Errors
///
/// Erro do banco.
pub(crate) async fn forget<C: ConnectionTrait>(
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

/// O que falta para entrar no equipamento por um transporte — o mesmo
/// critério na frota ("Sem acesso"), no plano do teste de enlace e na tela
/// que pede o acesso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessGap {
    /// Nenhuma credencial deste tipo.
    Missing(TransportKind),
    /// Acesso dedicado sem a chave guardada.
    NoSecret(TransportKind),
    /// A senha não foi informada na janela (ou a sessão expirou).
    SessionPending(TransportKind),
}

impl AccessGap {
    #[must_use]
    pub const fn kind(self) -> TransportKind {
        match self {
            Self::Missing(kind) | Self::NoSecret(kind) | Self::SessionPending(kind) => kind,
        }
    }

    /// A frase para a tela: "`AP-terraço` está sem acesso SSH".
    #[must_use]
    pub fn describe(self, device_name: &str) -> String {
        let label = self.kind().as_str().to_uppercase();
        match self {
            Self::Missing(_) => format!("{device_name} está sem acesso {label}"),
            Self::NoSecret(_) => {
                format!("{device_name} tem o acesso dedicado {label} sem a chave guardada")
            }
            Self::SessionPending(_) => {
                format!("A senha {label} de {device_name} ainda não foi informada")
            }
        }
    }
}

/// A primeira lacuna de acesso entre as credenciais já carregadas. HTTP sem
/// credencial vale (página pública); SSH/Telnet precisam de senha pronta.
#[must_use]
pub fn gap_in(list: &[CredentialView], transports: &[TransportKind]) -> Option<AccessGap> {
    transports
        .iter()
        .filter(|kind| **kind != TransportKind::Http)
        .find_map(|&kind| match list.iter().find(|view| view.kind == kind) {
            None => Some(AccessGap::Missing(kind)),
            Some(view) => match view.storage {
                SecretStorage::Key if !view.has_stored_secret => Some(AccessGap::NoSecret(kind)),
                SecretStorage::Ask if !view.session_active => Some(AccessGap::SessionPending(kind)),
                _ => None,
            },
        })
}

/// O que falta para entrar no equipamento pelos transportes pedidos.
///
/// # Errors
///
/// Erro do banco.
pub async fn access_gap<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    transports: &[TransportKind],
) -> AppResult<Option<AccessGap>> {
    if transports.iter().all(|kind| *kind == TransportKind::Http) {
        return Ok(None);
    }
    Ok(gap_in(&list(db, device_id).await?, transports))
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
                    login: Login::with_password(String::new(), None),
                    port: kind.default_port(),
                    https: false,
                });
                continue;
            }
            return Err(missing_credential(
                device_id,
                kind,
                format!(
                    "Cadastre a credencial {} deste equipamento para usar o plugin.",
                    kind.as_str().to_uppercase()
                ),
            ));
        };
        let profile = AccessProfile {
            login: login_of(&model, kind)?,
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

/// O login cadastrado de um tipo, para quem entra no equipamento fora de um
/// plugin (a ativação do syslog): a senha da sessão ou a chave do acesso
/// dedicado. Não sai do servidor.
///
/// # Errors
///
/// Credencial inexistente, senha não informada, ou falha ao decifrar a chave.
pub async fn stored_login<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    kind: TransportKind,
) -> AppResult<Login> {
    let label = kind.as_str().to_uppercase();
    let model = find(db, device_id, kind).await?.ok_or_else(|| {
        missing_credential(
            device_id,
            kind,
            format!("Este equipamento não tem credencial {label} salva. Informe usuário e senha."),
        )
    })?;
    let login = login_of(&model, kind)?;
    if login.password.is_none() && login.private_key.is_none() {
        return Err(missing_credential(
            device_id,
            kind,
            format!("A credencial {label} deste equipamento está sem senha. Informe a senha."),
        ));
    }
    Ok(login)
}

/// Grava o acesso dedicado que acabou de ser instalado e provado: a chave
/// privada cifrada, o resto em `extra.dedicated`. Substitui a credencial SSH
/// que houver — a senha que estava guardada sai do banco.
///
/// # Errors
///
/// Falha de cifra ou do banco.
pub async fn save_dedicated<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    username: &str,
    private_key: &str,
    dedicated: &DedicatedAccess,
    port: u16,
    via_probe_id: Option<i64>,
) -> AppResult<CredentialView> {
    let kind = TransportKind::Ssh;
    session_store().forget(&session_key(device_id, kind));
    let extra = serde_json::json!({
        "https": false,
        "dedicated": serde_json::to_value(dedicated)
            .map_err(|error| AppError::Internal(anyhow::anyhow!("acesso dedicado: {error}")))?,
    });
    let secret = Some(crypto::encrypt(private_key)?);
    let port = Some(i32::from(port));
    let saved = if let Some(model) = find(db, device_id, kind).await? {
        let mut active: device_credentials::ActiveModel = model.into();
        active.username = Set(username.to_owned());
        active.secret_encrypted = Set(secret);
        active.storage = Set(SecretStorage::Key.as_str().to_owned());
        active.port = Set(port);
        active.via_probe_id = Set(via_probe_id);
        active.extra = Set(Some(extra));
        active.update(db).await?
    } else {
        device_credentials::ActiveModel {
            device_id: Set(device_id),
            kind: Set(kind.as_str().to_owned()),
            username: Set(username.to_owned()),
            secret_encrypted: Set(secret),
            storage: Set(SecretStorage::Key.as_str().to_owned()),
            port: Set(port),
            via_probe_id: Set(via_probe_id),
            extra: Set(Some(extra)),
            ..Default::default()
        }
        .insert(db)
        .await?
    };
    to_view(&saved)
}

/// O login do acesso dedicado, para rotacionar ou revogar.
///
/// # Errors
///
/// O equipamento não tem acesso dedicado, ou falha ao decifrar.
pub async fn dedicated_login<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
) -> AppResult<(Login, DedicatedAccess)> {
    let kind = TransportKind::Ssh;
    let model = find(db, device_id, kind)
        .await?
        .filter(is_dedicated)
        .ok_or_else(|| AppError::business_rule("Este equipamento não tem acesso dedicado."))?;
    let access = dedicated_of(&model)
        .ok_or_else(|| AppError::business_rule("O registro do acesso dedicado está incompleto."))?;
    let login = login_of(&model, kind)?;
    if login.private_key.is_none() {
        return Err(AppError::business_rule(
            "A chave do acesso dedicado não está guardada.",
        ));
    }
    Ok((login, access))
}

/// Lembra um login que acabou de funcionar no equipamento: o usuário vai ao
/// cadastro e a senha abre a sessão de 30 minutos — nunca ao banco.
///
/// O resto da credencial daquele tipo (HTTPS, agente remoto) é preservado:
/// lembrar o login não pode desfazer a rota que um plugin já usa. Sem `port`,
/// vale a já cadastrada.
///
/// # Errors
///
/// Os mesmos de [`upsert`].
pub async fn remember_login<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    kind: TransportKind,
    login: &Login,
    port: Option<u16>,
) -> AppResult<CredentialView> {
    let existing = find(db, device_id, kind).await?;
    if let Some(model) = existing.as_ref().filter(|model| is_dedicated(model)) {
        // O acesso dedicado continua valendo: a senha que acabou de funcionar
        // não o substitui (nem vai ao banco).
        return to_view(model);
    }
    let input = CredentialInput {
        kind,
        username: login.username.clone(),
        secret: login.password.clone(),
        storage: SecretStorage::Ask,
        port: port.or_else(|| {
            existing
                .as_ref()
                .and_then(|model| model.port)
                .and_then(|port| u16::try_from(port).ok())
        }),
        https: existing.as_ref().map(https_of),
        via_probe_id: existing.as_ref().and_then(|model| model.via_probe_id),
    };
    upsert(db, device_id, input).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(
        kind: TransportKind,
        storage: SecretStorage,
        stored: bool,
        session: bool,
    ) -> CredentialView {
        CredentialView {
            kind,
            username: "root".into(),
            storage,
            port: kind.default_port(),
            https: false,
            via_probe_id: None,
            has_stored_secret: stored,
            session_active: session,
            dedicated: None,
        }
    }

    #[test]
    fn lacuna_de_acesso_por_transporte() {
        let ssh = [TransportKind::Ssh];
        assert_eq!(
            gap_in(&[], &ssh),
            Some(AccessGap::Missing(TransportKind::Ssh))
        );
        assert_eq!(
            gap_in(&[], &[TransportKind::Http]),
            None,
            "HTTP sem login vale"
        );
        let sessao = view(TransportKind::Ssh, SecretStorage::Ask, false, false);
        assert_eq!(
            gap_in(std::slice::from_ref(&sessao), &ssh),
            Some(AccessGap::SessionPending(TransportKind::Ssh))
        );
        let aberta = CredentialView {
            session_active: true,
            ..sessao
        };
        assert_eq!(gap_in(&[aberta], &ssh), None);
        // O acesso dedicado vale como a senha guardada.
        let chave = view(TransportKind::Ssh, SecretStorage::Key, true, false);
        assert_eq!(gap_in(&[chave], &ssh), None);
        let chave_perdida = view(TransportKind::Ssh, SecretStorage::Key, false, false);
        assert_eq!(
            gap_in(&[chave_perdida], &ssh),
            Some(AccessGap::NoSecret(TransportKind::Ssh))
        );
        // A do outro tipo não serve.
        let telnet = view(TransportKind::Telnet, SecretStorage::Ask, false, true);
        assert_eq!(
            gap_in(&[telnet], &ssh),
            Some(AccessGap::Missing(TransportKind::Ssh))
        );
        assert_eq!(
            AccessGap::Missing(TransportKind::Ssh).describe("AP-terraço"),
            "AP-terraço está sem acesso SSH"
        );
    }
}
