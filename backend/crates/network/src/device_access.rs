//! O acesso ao equipamento que a Rede oferece ao núcleo
//! ([`netmonitor_core::devices::access::DeviceAccess`]): a identidade SNMP, o
//! leitor de Wi-Fi, a credencial salva (ADR 016) e a shell SSH/Telnet que a
//! ativação do log usa. Registrado pelo ponto de montagem.

use std::net::IpAddr;

use agent_protocol::device_io::{Login, TransportKind};
use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    devices::access::{CredentialView, DeviceAccess, DeviceShell, SnmpSystemInfo},
    models::devices,
    shared::errors::{AppError, AppResult},
};
use sea_orm::DatabaseConnection;

use crate::{
    plugins::{
        self, credentials,
        transport::shell::{ssh, telnet},
    },
    snmp::{
        client::{SnmpClient, SnmpConfig, SnmpVersion},
        collectors::{collect_hardware, collect_system},
    },
    wifi,
};

/// O acesso da Rede.
pub struct NetworkDeviceAccess;

/// Lê a identidade SNMP do equipamento — `sysDescr` **e** `sysObjectId`.
///
/// Os dois, e não só a descrição: o `sysObjectId` é o número de empresa da
/// IANA, e num equipamento cuja descrição é o `uname` genérico ele é a única
/// evidência que ainda diz alguma coisa.
///
/// Falha vira `None` — é um palpite, e um agente SNMP fora do ar não pode
/// impedir a tela de abrir.
pub async fn snmp_identity(
    host: IpAddr,
    comunidade: &str,
    versao: Option<&str>,
) -> Option<SnmpSystemInfo> {
    if comunidade.trim().is_empty() {
        return None;
    }
    let mut config = SnmpConfig::v2c(host.to_string(), comunidade.to_owned(), 161);
    if versao.map(str::trim) == Some("v1") {
        config.version = SnmpVersion::V1;
    }
    // Teto curto: isto roda na abertura de um diálogo, não num ciclo de coleta.
    config.timeout_ms = 1_500;
    let client = SnmpClient::new(config);
    let mut identidade = collect_system(&client).await.ok()?;
    if let Ok((fabricante, modelo)) = collect_hardware(&client).await {
        identidade.hardware_vendor = fabricante;
        identidade.hardware_model = modelo;
    }
    Some(identidade)
}

/// Uma das duas sessões interativas.
enum Shell {
    Ssh(Box<ssh::Shell>),
    Telnet(Box<telnet::Shell>),
}

#[async_trait]
impl DeviceShell for Shell {
    async fn send_line(&mut self, line: &str) -> AppResult<()> {
        match self {
            Self::Ssh(shell) => shell.envia_linha(line).await,
            Self::Telnet(shell) => shell.envia_linha(line).await,
        }
    }

    async fn read_until_silent(&mut self) -> AppResult<String> {
        match self {
            Self::Ssh(shell) => shell.le_ate_silenciar().await,
            Self::Telnet(shell) => shell.le_ate_silenciar().await,
        }
    }

    async fn close(self: Box<Self>) {
        match *self {
            Self::Ssh(shell) => shell.encerra().await,
            Self::Telnet(shell) => shell.encerra().await,
        }
    }
}

#[async_trait]
impl DeviceAccess for NetworkDeviceAccess {
    async fn snmp_identity(
        &self,
        host: IpAddr,
        community: &str,
        version: Option<&str>,
    ) -> Option<SnmpSystemInfo> {
        snmp_identity(host, community, version).await
    }

    async fn has_wifi_reader(&self, db: &DatabaseConnection, device_id: i64) -> AppResult<bool> {
        Ok(wifi::source::for_device(db, device_id).await?.is_some())
    }

    async fn stored_login(
        &self,
        db: &DatabaseConnection,
        device_id: i64,
        kind: TransportKind,
    ) -> AppResult<Login> {
        credentials::stored_login(db, device_id, kind).await
    }

    async fn remember_login(
        &self,
        db: &DatabaseConnection,
        device_id: i64,
        kind: TransportKind,
        login: &Login,
        port: Option<u16>,
    ) -> AppResult<CredentialView> {
        credentials::remember_login(db, device_id, kind, login, port).await
    }

    async fn device_saved(&self, ctx: &AppContext, device: &devices::Model) {
        if let Err(error) = plugins::service::activate_for_device(&ctx.db, device).await {
            tracing::warn!(%error, device_id = device.id, "falha ao ligar plugins compatíveis");
        }
        // Sem sistema declarado, vale o que o equipamento mostrar (SSH/SNMP, ou
        // o Decisor) — e não o fabricante do cadastro. A sonda leva segundos:
        // em segundo plano, e o menu Aplicativos sabe pelo SSE se algo ligou.
        if device.operating_system.is_none() {
            plugins::identity::refresh_in_background(ctx, device.clone());
        }
    }

    async fn open_shell(
        &self,
        kind: TransportKind,
        host: IpAddr,
        port: u16,
        login: &Login,
    ) -> AppResult<Box<dyn DeviceShell>> {
        match kind {
            TransportKind::Ssh => Ok(Box::new(Shell::Ssh(Box::new(
                ssh::abre(host, port, login).await?,
            )))),
            TransportKind::Telnet => Ok(Box::new(Shell::Telnet(Box::new(
                telnet::abre(
                    host,
                    port,
                    &login.username,
                    login.password.as_deref().unwrap_or_default(),
                )
                .await?,
            )))),
            TransportKind::Http => Err(AppError::validation(
                "A credencial HTTP não abre shell: use a de SSH ou Telnet.",
            )),
        }
    }
}
