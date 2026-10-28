//! Acesso dedicado por chave SSH (ADR 016).
//!
//! O operador informa a senha do administrador **uma vez**; o NetMonitor entra,
//! instala uma chave ed25519 exclusiva daquele equipamento, prova que ela
//! funciona e só então guarda a chave privada (cifrada) como a credencial SSH
//! — modo `key` de [`super::credentials`]. A senha não vai ao banco nem à
//! memória de sessão: serve à instalação e é descartada.
//!
//! O que se ganha: a senha humana não fica guardada, trocá-la não quebra o
//! monitoramento, cada equipamento tem uma chave própria (vazou uma, não vazam
//! as outras), e a chave pode ser rotacionada sem o administrador e revogada
//! no equipamento. A chave de host vista na instalação fica fixada: dali em
//! diante, equipamento que se apresente com outra é recusado.
//!
//! É Rust nativo, não plugin: script de plugin não cria usuário nem mexe em
//! `authorized_keys` (`review::static_scan`), e é essa linha que mantém a
//! revisão de plugins honesta. Os comandos saem pelo mesmo
//! [`DeviceTransport`] dos plugins — da central ou do agente remoto do site,
//! sem comando novo no protocolo.
//!
//! Cada plataforma ([`Platform`]) diz só os seus comandos; o fluxo (detectar,
//! instalar, provar, desfazer se falhar) é um só.

pub mod keys;
pub mod mikrotik;
pub mod openwrt;

use loco_rs::app::AppContext;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use self::keys::GeneratedKey;
use super::{
    credentials::{self, CredentialView},
    manifest::TransportKind,
    runs::{device_ip, transport_for},
    transport::{
        is_reachable_target, DeviceIoCall, DeviceIoReply, DeviceTransport, Endpoint, Login,
    },
};
use crate::{
    models::devices,
    services::{
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        shared::errors::{AppError, AppResult},
    },
};

/// Prazo de cada passo no equipamento.
const STEP_TIMEOUT_MS: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DedicatedPlatform {
    Mikrotik,
    Openwrt,
}

impl DedicatedPlatform {
    const ALL: [Self; 2] = [Self::Openwrt, Self::Mikrotik];

    fn implementation(self) -> &'static dyn Platform {
        match self {
            Self::Mikrotik => &mikrotik::Mikrotik,
            Self::Openwrt => &openwrt::Openwrt,
        }
    }
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

/// A senha do administrador, informada uma vez.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ProvisionInput {
    /// Sem plataforma, o NetMonitor descobre (OpenWrt ou RouterOS).
    #[serde(default)]
    #[ts(optional)]
    pub platform: Option<DedicatedPlatform>,
    pub username: String,
    pub password: String,
    #[serde(default)]
    #[ts(optional)]
    pub port: Option<u16>,
    #[serde(default)]
    #[ts(optional, type = "number | null")]
    pub via_probe_id: Option<i64>,
}

/// Como apagar o acesso dedicado.
#[derive(Debug, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RevokeInput {
    /// Só apagar a credencial aqui, sem falar com o equipamento (ele foi
    /// trocado, descartado ou está fora de alcance).
    #[serde(default)]
    #[ts(optional)]
    pub local_only: Option<bool>,
}

/// O que sobrou para o operador depois de revogar.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct RevokeOutcome {
    /// Passos que o equipamento recusou — a chave já não entra, mas pode ter
    /// ficado algo para remover à mão (o usuário `netmonitor`, por exemplo).
    pub leftovers: Vec<String>,
}

/// A saída de um passo.
#[derive(Debug, Clone)]
pub struct StepOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub host_key: Option<String>,
}

/// Uma plataforma: os seus comandos e como ela diz que deu errado.
pub(crate) trait Platform: Send + Sync {
    /// Usuário com que o NetMonitor entra depois da instalação.
    fn username(&self) -> &'static str;
    /// Comando que identifica a plataforma — e prova a chave nova.
    fn detect_command(&self) -> &'static str;
    /// `Ok` se a saída de [`Self::detect_command`] é desta plataforma e ela
    /// suporta o acesso dedicado.
    fn check_detected(&self, output: &StepOutput) -> AppResult<()>;
    /// Instala a chave (e o que mais for preciso). `password` é a senha
    /// descartável de um usuário criado.
    fn install_commands(&self, key: &GeneratedKey, password: &str) -> Vec<String>;
    /// Retira a chave de comentário `comment`, deixando as outras.
    fn retire_commands(&self, comment: &str) -> Vec<String>;
    /// Retira o acesso inteiro.
    fn revoke_commands(&self, comment: &str) -> Vec<String>;
    /// O erro do passo, se houve.
    fn failure(&self, output: &StepOutput) -> Option<String>;
}

/// Onde o equipamento está e por onde se chega a ele.
#[derive(Debug, Clone)]
pub struct Target {
    pub ip: String,
    pub port: u16,
    pub via_probe_id: Option<i64>,
}

struct Device<'a> {
    transport: &'a dyn DeviceTransport,
    endpoint: Endpoint,
}

impl Device<'_> {
    async fn run(&self, login: &Login, command: &str) -> AppResult<StepOutput> {
        let reply = self
            .transport
            .execute(&DeviceIoCall::SshExec {
                endpoint: self.endpoint.clone(),
                login: login.clone(),
                command: command.to_owned(),
                timeout_ms: STEP_TIMEOUT_MS,
            })
            .await?;
        match reply {
            DeviceIoReply::Exec {
                stdout,
                stderr,
                exit_code,
                host_key,
            } => Ok(StepOutput {
                stdout,
                stderr,
                exit_code,
                host_key,
            }),
            DeviceIoReply::Http { .. } => Err(AppError::Internal(anyhow::anyhow!(
                "resposta HTTP a um exec SSH"
            ))),
        }
    }

    async fn step(&self, platform: &dyn Platform, login: &Login, command: &str) -> AppResult<()> {
        let output = self.run(login, command).await?;
        match platform.failure(&output) {
            Some(failure) => Err(AppError::business_rule(format!(
                "O equipamento recusou um passo do acesso dedicado: {failure}"
            ))),
            None => Ok(()),
        }
    }

    async fn steps(
        &self,
        platform: &dyn Platform,
        login: &Login,
        commands: &[String],
    ) -> AppResult<()> {
        for command in commands {
            self.step(platform, login, command).await?;
        }
        Ok(())
    }

    /// Desfazer: cada passo é tentado, e o que falhar volta em texto.
    async fn best_effort(
        &self,
        platform: &dyn Platform,
        login: &Login,
        commands: &[String],
    ) -> Vec<String> {
        let mut leftovers = Vec::new();
        for command in commands {
            if let Err(error) = self.step(platform, login, command).await {
                leftovers.push(error.to_string());
            }
        }
        leftovers
    }

    /// Qual plataforma é, e a chave de host que o equipamento apresentou.
    async fn detect(
        &self,
        login: &Login,
        requested: Option<DedicatedPlatform>,
    ) -> AppResult<(DedicatedPlatform, Option<String>)> {
        let candidates =
            requested.map_or(DedicatedPlatform::ALL.to_vec(), |platform| vec![platform]);
        let mut unsupported = None;
        for platform in candidates {
            let implementation = platform.implementation();
            let output = self.run(login, implementation.detect_command()).await?;
            match implementation.check_detected(&output) {
                Ok(()) => return Ok((platform, output.host_key.filter(|key| !key.is_empty()))),
                Err(error) => unsupported = Some(error),
            }
        }
        Err(match (requested, unsupported) {
            (Some(_), Some(error)) => error,
            _ => AppError::business_rule(
                "O acesso dedicado cobre MikroTik RouterOS e OpenWrt, e este equipamento não \
                 respondeu como nenhum dos dois. Continue com a senha.",
            ),
        })
    }
}

/// A plataforma que o sistema conhecido do equipamento sugere — observado,
/// cadastrado ou o fabricante. Só sugestão: a instalação confere de novo.
#[must_use]
pub fn suggested_platform(device: &devices::Model) -> Option<DedicatedPlatform> {
    let known = [
        &device.observed_os,
        &device.operating_system,
        &device.vendor,
    ]
    .into_iter()
    .flatten()
    .map(|value| value.to_ascii_lowercase())
    .collect::<Vec<_>>()
    .join(" ");
    if known.contains("openwrt") {
        Some(DedicatedPlatform::Openwrt)
    } else if known.contains("routeros") || known.contains("mikrotik") {
        Some(DedicatedPlatform::Mikrotik)
    } else {
        None
    }
}

fn dedicated_login(platform: &dyn Platform, key: &GeneratedKey, host_key: Option<String>) -> Login {
    Login {
        username: platform.username().to_owned(),
        password: None,
        private_key: Some(key.private_openssh.clone()),
        host_key,
    }
}

fn record(
    platform: DedicatedPlatform,
    key: &GeneratedKey,
    host_key: Option<String>,
) -> DedicatedAccess {
    DedicatedAccess {
        platform,
        public_key: key.public_openssh.clone(),
        key_fingerprint: key.fingerprint.clone(),
        key_comment: key.comment.clone(),
        host_key,
        provisioned_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Instala o acesso dedicado com o login do administrador.
///
/// # Errors
///
/// Login vazio, plataforma não suportada, passo recusado ou chave que não
/// entrou — nesses casos o que foi instalado é desfeito e nada é gravado.
pub async fn provision_on<C: ConnectionTrait>(
    db: &C,
    transport: &dyn DeviceTransport,
    device_id: i64,
    target: &Target,
    input: ProvisionInput,
) -> AppResult<CredentialView> {
    let username = input.username.trim();
    if username.is_empty() || input.password.is_empty() {
        return Err(AppError::validation(
            "Informe o usuário e a senha do administrador do equipamento.",
        ));
    }
    let admin = Login::with_password(username, Some(input.password));
    let device = Device {
        transport,
        endpoint: Endpoint {
            host: target.ip.clone(),
            port: target.port,
        },
    };
    let (platform, host_key) = device.detect(&admin, input.platform).await?;
    let implementation = platform.implementation();
    let key = keys::generate()?;

    let installed = device
        .steps(
            implementation,
            &admin,
            &implementation.install_commands(&key, &keys::throwaway_password()),
        )
        .await;
    let proved = match installed {
        Ok(()) => {
            let login = dedicated_login(implementation, &key, host_key.clone());
            match device.run(&login, implementation.detect_command()).await {
                Ok(output) => implementation.check_detected(&output),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    };
    if let Err(error) = proved {
        let _ = device
            .best_effort(
                implementation,
                &admin,
                &implementation.revoke_commands(&key.comment),
            )
            .await;
        return Err(AppError::business_rule(format!(
            "O acesso dedicado não foi criado — o que chegou a ser instalado foi desfeito e nada \
             foi guardado. {error}"
        )));
    }

    credentials::save_dedicated(
        db,
        device_id,
        implementation.username(),
        &key.private_openssh,
        &record(platform, &key, host_key),
        target.port,
        target.via_probe_id,
    )
    .await
}

/// Troca a chave usando a própria chave atual — sem o administrador.
///
/// # Errors
///
/// Sem acesso dedicado, equipamento inacessível ou chave nova recusada (a
/// antiga volta a valer).
pub async fn rotate_on<C: ConnectionTrait>(
    db: &C,
    transport: &dyn DeviceTransport,
    device_id: i64,
    target: &Target,
) -> AppResult<CredentialView> {
    let (current, access) = credentials::dedicated_login(db, device_id).await?;
    let implementation = access.platform.implementation();
    let device = Device {
        transport,
        endpoint: Endpoint {
            host: target.ip.clone(),
            port: target.port,
        },
    };
    let key = keys::generate()?;
    let old = GeneratedKey {
        private_openssh: String::new(),
        public_openssh: access.public_key.clone(),
        fingerprint: access.key_fingerprint.clone(),
        comment: access.key_comment.clone(),
    };

    let installed = device
        .steps(
            implementation,
            &current,
            &implementation.install_commands(&key, &keys::throwaway_password()),
        )
        .await;
    let new_login = dedicated_login(implementation, &key, access.host_key.clone());
    let proved = match installed {
        Ok(()) => match device
            .run(&new_login, implementation.detect_command())
            .await
        {
            Ok(output) => implementation.check_detected(&output),
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    if let Err(error) = proved {
        // A sessão da chave antiga continua aberta no transporte: por ela a
        // antiga volta e a nova sai.
        let mut undo = implementation.install_commands(&old, &keys::throwaway_password());
        undo.extend(implementation.retire_commands(&key.comment));
        let _ = device.best_effort(implementation, &current, &undo).await;
        return Err(AppError::business_rule(format!(
            "A chave nova não foi aceita; a atual continua valendo. {error}"
        )));
    }
    let leftovers = device
        .best_effort(
            implementation,
            &new_login,
            &implementation.retire_commands(&old.comment),
        )
        .await;
    if !leftovers.is_empty() {
        tracing::warn!(
            device_id,
            ?leftovers,
            "a chave antiga do acesso dedicado ficou no equipamento"
        );
    }

    credentials::save_dedicated(
        db,
        device_id,
        implementation.username(),
        &key.private_openssh,
        &record(access.platform, &key, access.host_key),
        target.port,
        target.via_probe_id,
    )
    .await
}

/// Retira a chave do equipamento e apaga a credencial.
///
/// # Errors
///
/// Sem acesso dedicado, ou equipamento inacessível (sem `local_only`): aí
/// nada é apagado, para a chave não ficar esquecida lá.
pub async fn revoke_on<C: ConnectionTrait>(
    db: &C,
    transport: &dyn DeviceTransport,
    device_id: i64,
    target: &Target,
    input: &RevokeInput,
) -> AppResult<RevokeOutcome> {
    let (current, access) = credentials::dedicated_login(db, device_id).await?;
    let mut leftovers = Vec::new();
    if !input.local_only.unwrap_or(false) {
        let implementation = access.platform.implementation();
        let device = Device {
            transport,
            endpoint: Endpoint {
                host: target.ip.clone(),
                port: target.port,
            },
        };
        // Primeiro, falar com ele: inacessível não é "revogado".
        device
            .run(&current, implementation.detect_command())
            .await
            .map_err(|error| {
                AppError::business_rule(format!(
                    "Não foi possível falar com o equipamento para retirar a chave ({error}). \
                     Se ele foi trocado ou descartado, apague só no NetMonitor."
                ))
            })?;
        leftovers = device
            .best_effort(
                implementation,
                &current,
                &implementation.revoke_commands(&access.key_comment),
            )
            .await;
    }
    credentials::delete(db, device_id, TransportKind::Ssh).await?;
    Ok(RevokeOutcome { leftovers })
}

/// Transporte para quando não se fala com o equipamento.
struct Unreachable;

#[async_trait::async_trait]
impl DeviceTransport for Unreachable {
    async fn execute(&self, _call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
        Err(AppError::business_rule("Equipamento fora de alcance."))
    }

    fn origin(&self) -> String {
        "nenhum".into()
    }
}

/// O equipamento e o alvo — o que as três operações precisam.
async fn locate(
    ctx: &AppContext,
    device_id: i64,
    port: Option<u16>,
    via_probe_id: Option<i64>,
) -> AppResult<(devices::Model, Target)> {
    let device = devices::Entity::find_by_id(device_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Equipamento não encontrado."))?;
    let ip = device_ip(&device)?;
    if !ip.parse().is_ok_and(is_reachable_target) {
        return Err(AppError::business_rule(
            "O acesso dedicado só alcança equipamentos da rede privada.",
        ));
    }
    let current = credentials::list(&ctx.db, device_id)
        .await?
        .into_iter()
        .find(|view| view.kind == TransportKind::Ssh);
    let target = Target {
        ip,
        port: port
            .or_else(|| current.as_ref().map(|view| view.port))
            .unwrap_or_else(|| TransportKind::Ssh.default_port()),
        via_probe_id: via_probe_id.or_else(|| current.and_then(|view| view.via_probe_id)),
    };
    Ok((device, target))
}

async fn audit(ctx: &AppContext, user_id: i64, device: &devices::Model, what: &str) {
    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor {
                user_id: Some(user_id),
                ..AuditActor::default()
            },
            AuditEntryInput {
                action: AuditAction::Execute,
                resource_type: ResourceType::Device,
                resource_id: Some(device.id),
                resource_label: Some(device.name.clone()),
                description: Some(format!("O operador {what} de {}", device.name)),
                changes: None,
            },
        )
        .await;
}

/// # Errors
///
/// Os de [`provision_on`], equipamento inexistente ou fora da rede privada.
pub async fn provision(
    ctx: &AppContext,
    device_id: i64,
    user_id: i64,
    input: ProvisionInput,
) -> AppResult<CredentialView> {
    let (device, target) = locate(ctx, device_id, input.port, input.via_probe_id).await?;
    let transport = transport_for(ctx, target.via_probe_id).await?;
    let view = provision_on(&ctx.db, transport.as_ref(), device_id, &target, input).await?;
    audit(
        ctx,
        user_id,
        &device,
        "criou o acesso dedicado por chave SSH",
    )
    .await;
    Ok(view)
}

/// # Errors
///
/// Os de [`rotate_on`].
pub async fn rotate(ctx: &AppContext, device_id: i64, user_id: i64) -> AppResult<CredentialView> {
    let (device, target) = locate(ctx, device_id, None, None).await?;
    let transport = transport_for(ctx, target.via_probe_id).await?;
    let view = rotate_on(&ctx.db, transport.as_ref(), device_id, &target).await?;
    audit(
        ctx,
        user_id,
        &device,
        "rotacionou a chave do acesso dedicado",
    )
    .await;
    Ok(view)
}

/// # Errors
///
/// Os de [`revoke_on`].
pub async fn revoke(
    ctx: &AppContext,
    device_id: i64,
    user_id: i64,
    input: RevokeInput,
) -> AppResult<RevokeOutcome> {
    let (device, target) = locate(ctx, device_id, None, None).await?;
    let local_only = input.local_only.unwrap_or(false);
    let outcome = if local_only {
        // Sem falar com o equipamento — nem exigir o agente conectado.
        revoke_on(&ctx.db, &Unreachable, device_id, &target, &input).await?
    } else {
        let transport = transport_for(ctx, target.via_probe_id).await?;
        revoke_on(&ctx.db, transport.as_ref(), device_id, &target, &input).await?
    };
    let what = if local_only {
        "apagou só no NetMonitor o acesso dedicado"
    } else {
        "revogou o acesso dedicado"
    };
    audit(ctx, user_id, &device, what).await;
    Ok(outcome)
}
