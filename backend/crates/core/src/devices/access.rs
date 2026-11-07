//! O acesso ao equipamento que é da Rede, visto pelo núcleo (ADR 017).
//!
//! A identificação de dispositivo, as capacidades da tela e a ativação do log
//! no roteador precisam de três coisas que moram na área Rede: a identidade
//! SNMP do equipamento, a credencial salva nele (a aba "Plugins ›
//! Credenciais", ADR 016) e uma shell interativa (SSH ou Telnet). Elas chegam
//! pelo [`DeviceAccess`], que o ponto de montagem registra. Sem a área, o
//! núcleo segue sem palpite de SNMP, sem leitor de Wi-Fi e sem shell.
//!
//! Os dados que cruzam a fronteira (a vista da credencial, sem senha, e a
//! identidade SNMP) moram aqui, para o núcleo poder nomeá-los.

use std::{net::IpAddr, sync::Arc, time::Duration};

use agent_protocol::device_io::{Login, TransportKind};
use async_trait::async_trait;
use loco_rs::app::AppContext;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::devices,
    shared::errors::{AppError, AppResult},
};

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

    #[must_use]
    pub fn parse(value: &str) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DedicatedPlatform {
    Mikrotik,
    Openwrt,
}

/// O que fica em `device_credentials.extra.dedicated` e o que a tela vê.
/// Nada aqui é segredo: a chave privada mora em `secret_encrypted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DedicatedAccess {
    pub platform: DedicatedPlatform,
    /// `ssh-ed25519 AAAA… netmonitor-<id>`.
    pub public_key: String,
    /// `SHA256:…` da chave do NetMonitor.
    pub key_fingerprint: String,
    /// A marca da chave no equipamento (`netmonitor-<id>`).
    pub key_comment: String,
    /// `SHA256:…` da chave de host fixada na instalação.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key: Option<String>,
    /// Quando a chave atual foi instalada (RFC 3339).
    pub provisioned_at: String,
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpSystemInfo {
    pub sys_descr: Option<String>,
    pub sys_object_id: Option<String>,
    pub sys_up_time: Option<u64>,
    pub sys_contact: Option<String>,
    pub sys_name: Option<String>,
    pub sys_location: Option<String>,
    /// Fabricante e modelo do chassi, quando o agente implementa ENTITY-MIB.
    pub hardware_vendor: Option<String>,
    pub hardware_model: Option<String>,
}

impl SnmpSystemInfo {
    /// Houve contato real com o agente? (matriz de paridade #19)
    ///
    /// Só `sysDescr`, `sysName` e `sysUpTime` contam. `sysContact` e
    /// `sysLocation` ficam de fora de propósito: são campos de texto livre que
    /// muitos agentes devolvem vazios, e aceitá-los como prova marcaria o
    /// equipamento "online" sem que nenhum OID de identidade tivesse respondido.
    #[must_use]
    pub const fn responded(&self) -> bool {
        self.sys_descr.is_some() || self.sys_name.is_some() || self.sys_up_time.is_some()
    }
}

/// Teto da sessão inteira. Um equipamento que não responde não pode segurar
/// um worker do Axum indefinidamente.
pub const TETO_DA_SESSAO: Duration = Duration::from_secs(45);
/// Teto para o TCP abrir. Roteador desligado falha rápido, e a tela diz isso em
/// vez de girar.
pub const TETO_DE_CONEXAO: Duration = Duration::from_secs(8);
/// Silêncio que declara um comando terminado. Ver a nota do módulo.
pub const SILENCIO: Duration = Duration::from_millis(700);
/// Teto por comando, quando o equipamento não para de falar.
pub const TETO_POR_COMANDO: Duration = Duration::from_secs(10);

/// Uma shell interativa aberta no equipamento: digita linhas e lê até o
/// equipamento silenciar.
#[async_trait]
pub trait DeviceShell: Send {
    /// Digita uma linha.
    ///
    /// # Errors
    ///
    /// Sessão caída.
    async fn send_line(&mut self, line: &str) -> AppResult<()>;

    /// Lê até o equipamento parar de falar (ou o teto do comando).
    ///
    /// # Errors
    ///
    /// Sessão caída.
    async fn read_until_silent(&mut self) -> AppResult<String>;

    /// Encerra a sessão.
    async fn close(self: Box<Self>);
}

/// O que a Rede oferece ao núcleo sobre um equipamento.
#[async_trait]
pub trait DeviceAccess: Send + Sync {
    /// A identidade SNMP — `sysDescr`, `sysObjectId` e o chassi —, com teto
    /// curto. Falha vira `None`: é palpite.
    async fn snmp_identity(
        &self,
        host: IpAddr,
        community: &str,
        version: Option<&str>,
    ) -> Option<SnmpSystemInfo>;

    /// Há quem leia o Wi-Fi do equipamento (um plugin ativo com o bloco
    /// `wifi`)?
    ///
    /// # Errors
    ///
    /// Erro do banco.
    async fn has_wifi_reader(&self, db: &DatabaseConnection, device_id: i64) -> AppResult<bool>;

    /// O login salvo no equipamento, daquele tipo.
    ///
    /// # Errors
    ///
    /// Credencial ausente ou senha de sessão vencida.
    async fn stored_login(
        &self,
        db: &DatabaseConnection,
        device_id: i64,
        kind: TransportKind,
    ) -> AppResult<Login>;

    /// Lembra o login que acabou de funcionar como a credencial do
    /// equipamento e devolve a vista dela (sem senha).
    ///
    /// # Errors
    ///
    /// Erro do banco.
    async fn remember_login(
        &self,
        db: &DatabaseConnection,
        device_id: i64,
        kind: TransportKind,
        login: &Login,
        port: Option<u16>,
    ) -> AppResult<CredentialView>;

    /// O dispositivo foi cadastrado (ou trocou de sistema): liga os plugins
    /// que dizem servir a ele e, sem sistema declarado, identifica o
    /// equipamento em segundo plano. Falhar aqui não desfaz o cadastro.
    async fn device_saved(&self, ctx: &AppContext, device: &devices::Model);

    /// Abre uma shell no equipamento (`Ssh` ou `Telnet`).
    ///
    /// # Errors
    ///
    /// Falha de conexão ou de autenticação; tipo que não abre shell.
    async fn open_shell(
        &self,
        kind: TransportKind,
        host: IpAddr,
        port: u16,
        login: &Login,
    ) -> AppResult<Box<dyn DeviceShell>>;
}

#[derive(Clone)]
struct Registered(Arc<dyn DeviceAccess>);

/// Registra o acesso da área no `shared_store` (o `after_context`: vale para
/// o servidor e para as tarefas).
pub fn install(ctx: &AppContext, access: Arc<dyn DeviceAccess>) {
    ctx.shared_store.insert(Registered(access));
}

/// O acesso registrado, se a área Rede foi montada.
#[must_use]
pub fn from_context(ctx: &AppContext) -> Option<Arc<dyn DeviceAccess>> {
    ctx.shared_store
        .get::<Registered>()
        .map(|registered| registered.0)
}

/// O acesso registrado, ou o erro que a tela mostra quando ele falta.
///
/// # Errors
///
/// `service_unavailable` sem a área Rede montada.
pub fn required(ctx: &AppContext) -> AppResult<Arc<dyn DeviceAccess>> {
    from_context(ctx).ok_or_else(|| {
        AppError::ServiceUnavailable(
            "O acesso ao equipamento é da área Rede, que não está montada neste processo".into(),
        )
    })
}
