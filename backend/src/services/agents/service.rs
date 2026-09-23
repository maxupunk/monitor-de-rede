//! Cadastro, enrollment e ciclo de vida dos agentes remotos.
//!
//! O agente é uma linha de `probes` (ADR 011) marcada com
//! `configuration.role = "agent"`. Tudo que o controller faz com agentes passa
//! por aqui; as regras puras (origem pelo túnel, comandos de instalação)
//! ficam em funções livres, testáveis sem banco.

use std::net::IpAddr;

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};

use crate::{
    dtos::agents::{AgentCreateInput, AgentEnrollInput, AgentUpdateInput},
    models::{devices, probes},
    services::{
        docker::hosts::HostKey,
        events::EventBus,
        maintenance::resource_cleanup::ResourceCleanupService,
        probes::{
            dispatcher,
            liveness::{status_payload, STATUS_OFFLINE, STATUS_ONLINE},
            DEFAULT_VPN_PROBE_TOKEN,
        },
        shared::{
            crypto::{random_token, sha256_hex},
            errors::{AppError, AppResult},
        },
        telemetry,
        vpn::monitor_provisioner::vpn_probe_name,
    },
    views::agents::{AgentCapabilityView, AgentHostInfo, AgentView},
};

use super::{enrollment, protocol::Hello};

pub const ROLE_AGENT: &str = "agent";
const STATUS_PENDING: &str = "pending";
/// Validade do código de enrollment, igual à do cofre (15 minutos).
pub const ENROLLMENT_TTL_SECONDS: u64 = 15 * 60;
const MAX_NAME: usize = 120;

fn is_agent(probe: &probes::Model) -> bool {
    probe
        .configuration
        .as_ref()
        .and_then(|value| value.get("role"))
        .and_then(Value::as_str)
        == Some(ROLE_AGENT)
}

fn enforce_tunnel_ip(probe: &probes::Model) -> bool {
    probe
        .configuration
        .as_ref()
        .and_then(|value| value.get("enforceTunnelIp"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(AppError::validation(
            "Nome do agente deve ter entre 1 e 120 caracteres",
        ));
    }
    Ok(name.to_string())
}

/// A conexão de um agente vinculado a um dispositivo da VPN precisa vir do IP
/// do túnel daquele dispositivo. É o que impede um token vazado de ser usado
/// de fora da VPN.
///
/// # Errors
///
/// `unauthorized` quando o vínculo exige o túnel e a origem é outra.
pub fn check_tunnel_origin(
    device: Option<&devices::Model>,
    enforce: bool,
    peer: Option<IpAddr>,
) -> AppResult<()> {
    let Some(device) = device else {
        return Ok(());
    };
    if !enforce || device.access_mode.as_deref() != Some("vpn") {
        return Ok(());
    }
    let expected = device
        .ip_address
        .as_deref()
        .and_then(|ip| ip.trim().parse::<IpAddr>().ok());
    match (expected, peer) {
        (Some(expected), Some(peer)) if canonical(peer) == canonical(expected) => Ok(()),
        _ => Err(AppError::unauthorized(
            "O agente deste servidor só pode se conectar pelo túnel VPN",
        )),
    }
}

/// `::ffff:10.8.0.2` e `10.8.0.2` são a mesma origem.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

/// Imagem do agente publicada pelo workflow `agent-image.yml` (alvo `agent`
/// do `Dockerfile`, amd64 e arm64). `AGENT_IMAGE` na central troca por outra —
/// uma versão fixa ou um registry próprio.
pub const DEFAULT_AGENT_IMAGE: &str = "ghcr.io/maxupunk/netmonitor-agent:latest";

fn agent_image() -> String {
    std::env::var("AGENT_IMAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_IMAGE.to_string())
}

/// Comandos de instalação exibidos uma vez, com o código embutido.
///
/// O container recebe o socket do Docker, `/proc` e a raiz do host somente
/// leitura (métricas de host) e roda na rede do host — é por ela que o
/// túnel WireGuard do servidor está acessível.
#[must_use]
pub fn install_commands(server_url: &str, code: &str) -> (String, String) {
    let common = format!(
        "docker run -d --name netmonitor-agent --restart unless-stopped \\\n  \
         --network host --pid host \\\n  \
         -v /var/run/docker.sock:/var/run/docker.sock \\\n  \
         -v /proc:/host/proc:ro -v /:/host:ro \\\n  \
         -v netmonitor-agent:/var/lib/netmonitor-agent \\\n  \
         -e AGENT_SERVER_URL={server_url} \\\n  \
         -e AGENT_ENROLL_CODE={code} \\\n  \
         -e AGENT_ALLOW=read,lifecycle,monitor,discovery \\\n  "
    );
    let docker = format!("{common}{image}", image = agent_image());
    let systemd = format!(
        "curl -fsSL {server_url}/api/agents/install.sh | \\\n  \
         sudo AGENT_SERVER_URL={server_url} AGENT_ENROLL_CODE={code} sh"
    );
    (docker, systemd)
}

/// Monta a view a partir do probe, do dispositivo vinculado e da conexão.
#[must_use]
pub fn to_view(
    probe: &probes::Model,
    device: Option<&devices::Model>,
    connected: bool,
) -> AgentView {
    let host = probe
        .configuration
        .as_ref()
        .and_then(|value| value.get("host"))
        .cloned()
        .and_then(|value| serde_json::from_value::<AgentHostInfo>(value).ok())
        .unwrap_or_default();
    AgentView {
        id: probe.id,
        name: probe.name.clone(),
        host_key: HostKey::agent(probe.id).to_string(),
        site_id: probe.site_id,
        device_id: probe.device_id,
        device_name: device.map(|device| device.name.clone()),
        device_ip: device.and_then(|device| device.ip_address.clone()),
        status: probe.status.clone(),
        connected,
        version: probe.version.clone(),
        last_seen_at: probe.last_seen_at.map(|value| value.to_rfc3339()),
        registered_at: probe.registered_at.map(|value| value.to_rfc3339()),
        enforce_tunnel_ip: enforce_tunnel_ip(probe),
        host,
        created_at: probe.created_at.to_rfc3339(),
    }
}

fn host_info(hello: &Hello) -> AgentHostInfo {
    let capability = |cap: &super::protocol::Capability| AgentCapabilityView {
        available: cap.available,
        version: cap.version.clone(),
        reason: cap.reason.clone(),
    };
    AgentHostInfo {
        hostname: Some(hello.hostname.clone()),
        os: Some(hello.os.clone()),
        arch: Some(hello.arch.clone()),
        in_container: hello.in_container,
        policy: hello.policy.clone(),
        docker: capability(&hello.docker),
        compose: capability(&hello.compose),
    }
}

pub struct AgentService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> AgentService<'a> {
    #[must_use]
    pub const fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    async fn ensure_device(&self, device_id: Option<i64>) -> AppResult<Option<devices::Model>> {
        let Some(device_id) = device_id else {
            return Ok(None);
        };
        devices::Entity::find_by_id(device_id)
            .one(self.db)
            .await?
            .map(Some)
            .ok_or_else(|| AppError::validation("Dispositivo informado não existe"))
    }

    /// Cadastra o agente e emite o primeiro código de enrollment.
    ///
    /// O `token_hash` inicial é de um token que ninguém conhece: até o
    /// enrollment, nenhuma credencial autentica este probe.
    ///
    /// # Errors
    ///
    /// Validação do nome ou do dispositivo; erro do banco.
    pub async fn create(&self, input: &AgentCreateInput) -> AppResult<(probes::Model, String)> {
        let name = validate_name(&input.name)?;
        self.ensure_device(input.device_id).await?;
        let saved = probes::ActiveModel {
            site_id: Set(input.site_id),
            name: Set(name),
            token_hash: Set(sha256_hex(&random_token())),
            status: Set(STATUS_PENDING.into()),
            device_id: Set(input.device_id),
            configuration: Set(Some(json!({
                "role": ROLE_AGENT,
                "enforceTunnelIp": input.enforce_tunnel_ip.unwrap_or(true),
            }))),
            ..Default::default()
        }
        .insert(self.db)
        .await?;
        let code = enrollment::issue(saved.id);
        Ok((saved, code))
    }

    /// Agente criado junto com o peer da VPN: já nasce vinculado ao
    /// dispositivo e exigindo o túnel. O código sai na geração do script.
    ///
    /// # Errors
    ///
    /// Validação do nome ou erro do banco.
    pub async fn create_for_device(
        &self,
        name: &str,
        site_id: Option<i64>,
        device_id: i64,
    ) -> AppResult<probes::Model> {
        if let Some(existing) = self.for_device(device_id).await? {
            return Ok(existing);
        }
        let (probe, _code) = self
            .create(&AgentCreateInput {
                name: name.to_string(),
                site_id,
                device_id: Some(device_id),
                enforce_tunnel_ip: Some(true),
            })
            .await?;
        Ok(probe)
    }

    /// Agente ativo vinculado ao dispositivo.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn for_device(&self, device_id: i64) -> AppResult<Option<probes::Model>> {
        Ok(probes::Entity::find()
            .filter(probes::Column::DeviceId.eq(device_id))
            .filter(probes::Column::Status.ne(probes::STATUS_REVOKED))
            .order_by_asc(probes::Column::Id)
            .all(self.db)
            .await?
            .into_iter()
            .find(is_agent))
    }

    /// # Errors
    ///
    /// `not_found` quando o id não é de um agente.
    pub async fn find(&self, id: i64) -> AppResult<probes::Model> {
        probes::Entity::find_by_id(id)
            .one(self.db)
            .await?
            .filter(is_agent)
            .ok_or_else(|| AppError::not_found("Agente não encontrado"))
    }

    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn list(&self) -> AppResult<Vec<probes::Model>> {
        Ok(probes::Entity::find()
            .order_by_asc(probes::Column::Name)
            .all(self.db)
            .await?
            .into_iter()
            .filter(is_agent)
            .collect())
    }

    /// Dispositivos vinculados, para montar as views sem N consultas.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn devices_of(&self, agents: &[probes::Model]) -> AppResult<Vec<devices::Model>> {
        let ids: Vec<i64> = agents.iter().filter_map(|agent| agent.device_id).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(devices::Entity::find()
            .filter(devices::Column::Id.is_in(ids))
            .all(self.db)
            .await?)
    }

    /// # Errors
    ///
    /// Validação, agente inexistente ou erro do banco.
    pub async fn update(&self, id: i64, input: &AgentUpdateInput) -> AppResult<probes::Model> {
        let probe = self.find(id).await?;
        let mut configuration = probe.configuration.clone().unwrap_or_else(|| json!({}));
        let mut active: probes::ActiveModel = probe.into();
        if let Some(name) = &input.name {
            active.name = Set(validate_name(name)?);
        }
        if input.device_id.is_some() {
            self.ensure_device(input.device_id).await?;
            active.device_id = Set(input.device_id);
        }
        if let Some(enforce) = input.enforce_tunnel_ip {
            configuration["enforceTunnelIp"] = Value::Bool(enforce);
            active.configuration = Set(Some(configuration));
        }
        Ok(active.update(self.db).await?)
    }

    /// Novo código para reinstalar o agente. O token atual segue valendo até
    /// o novo enrollment trocá-lo.
    ///
    /// # Errors
    ///
    /// Agente inexistente ou revogado.
    pub async fn reissue_code(&self, id: i64) -> AppResult<String> {
        let probe = self.find(id).await?;
        if probe.status == probes::STATUS_REVOKED {
            return Err(AppError::business_rule(
                "Agente revogado: cadastre um novo para reinstalar",
            ));
        }
        Ok(enrollment::issue(probe.id))
    }

    /// Troca o código de uso único pelo token de longa duração.
    ///
    /// # Errors
    ///
    /// `unauthorized` para código inválido, expirado ou já usado.
    pub async fn enroll(&self, input: &AgentEnrollInput) -> AppResult<(probes::Model, String)> {
        let invalid = || AppError::unauthorized("Código de enrollment inválido ou expirado");
        let probe_id = enrollment::redeem(&input.code).ok_or_else(invalid)?;
        let probe = self.find(probe_id).await.map_err(|_| invalid())?;
        if probe.status == probes::STATUS_REVOKED {
            return Err(invalid());
        }
        let token = random_token();
        let mut active: probes::ActiveModel = probe.into();
        active.token_hash = Set(sha256_hex(&token));
        active.registered_at = Set(Some(Utc::now().into()));
        Ok((active.update(self.db).await?, token))
    }

    /// Autentica a abertura do canal.
    ///
    /// # Errors
    ///
    /// `unauthorized` para token inválido, token compartilhado fora do
    /// `vpn-probe` ou origem fora do túnel exigido.
    pub async fn authenticate(
        &self,
        raw_token: &str,
        peer: Option<IpAddr>,
    ) -> AppResult<probes::Model> {
        let unauthorized = || AppError::unauthorized("Agente não encontrado ou token inválido");
        let raw_token = raw_token.trim();
        if raw_token.is_empty() {
            return Err(unauthorized());
        }
        let probe = probes::Entity::find_by_token(raw_token)
            .order_by_asc(probes::Column::Id)
            .one(self.db)
            .await?
            .ok_or_else(unauthorized)?;
        // O token compartilhado existe para o `vpn-probe` zero-config (AGENTS
        // §6). No canal de comando ele daria acesso ao Docker de um host a
        // quem conhece o valor padrão — por isso só vale para aquele probe.
        if raw_token == DEFAULT_VPN_PROBE_TOKEN && probe.name != vpn_probe_name() {
            return Err(unauthorized());
        }
        let device = self.ensure_device(probe.device_id).await.ok().flatten();
        check_tunnel_origin(device.as_ref(), enforce_tunnel_ip(&probe), peer)?;
        Ok(probe)
    }

    /// Registra a conexão: `online`, versão e o que o `Hello` anunciou.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn mark_connected(
        &self,
        probe: probes::Model,
        hello: &Hello,
    ) -> AppResult<probes::Model> {
        let mut configuration = probe.configuration.clone().unwrap_or_else(|| json!({}));
        configuration["role"] = Value::String(ROLE_AGENT.into());
        configuration["host"] = serde_json::to_value(host_info(hello))
            .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
        let mut active: probes::ActiveModel = probe.into();
        active.status = Set(STATUS_ONLINE.into());
        active.last_seen_at = Set(Some(Utc::now().into()));
        active.version = Set(Some(hello.agent_version.clone()));
        active.configuration = Set(Some(configuration));
        Ok(active.update(self.db).await?)
    }

    /// Mantém o `last_seen_at` fresco enquanto o canal está aberto: é o que o
    /// agendador consulta para decidir entre despachar ou rodar local.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn touch(&self, probe_id: i64) -> AppResult<()> {
        probes::Entity::update_many()
            .col_expr(
                probes::Column::LastSeenAt,
                sea_orm::sea_query::Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(
                    Utc::now(),
                )),
            )
            .filter(probes::Column::Id.eq(probe_id))
            .exec(self.db)
            .await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn mark_disconnected(&self, probe_id: i64) -> AppResult<Option<probes::Model>> {
        let Some(probe) = probes::Entity::find_by_id(probe_id).one(self.db).await? else {
            return Ok(None);
        };
        if probe.status == probes::STATUS_REVOKED {
            return Ok(Some(probe));
        }
        let mut active: probes::ActiveModel = probe.into();
        active.status = Set(STATUS_OFFLINE.into());
        Ok(Some(active.update(self.db).await?))
    }

    /// Revoga: nenhum token deste agente autentica mais e a fila é limpa.
    ///
    /// # Errors
    ///
    /// Agente inexistente ou erro do banco.
    pub async fn revoke(&self, id: i64) -> AppResult<probes::Model> {
        let probe = self.find(id).await?;
        let mut active: probes::ActiveModel = probe.into();
        active.status = Set(probes::STATUS_REVOKED.into());
        active.revoked_at = Set(Some(Utc::now().into()));
        let saved = active.update(self.db).await?;
        dispatcher::clear_tasks_for_probe(self.db, saved.id).await?;
        Ok(saved)
    }

    /// Remove o agente, seus monitores e o histórico de métricas do host.
    ///
    /// # Errors
    ///
    /// Agente inexistente ou erro do banco.
    pub async fn delete(&self, id: i64) -> AppResult<probes::Model> {
        let probe = self.find(id).await?;
        telemetry::store::forget_host(self.db, &HostKey::agent(probe.id).to_string()).await?;
        ResourceCleanupService::delete_probe(self.db, probe.id).await?;
        Ok(probe)
    }
}

/// URL da central vista de dentro do túnel: `AGENT_SERVER_URL` explícita ou
/// o IP da central na VPN com a porta da API (`APP_PORT`, 3333 por padrão).
#[must_use]
pub fn tunnel_server_url(server_vpn_address: &str) -> String {
    if let Ok(url) = std::env::var("AGENT_SERVER_URL") {
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.is_empty() {
            return url;
        }
    }
    let port = std::env::var("APP_PORT")
        .ok()
        .and_then(|port| port.trim().parse::<u16>().ok())
        .unwrap_or(3333);
    format!("http://{server_vpn_address}:{port}")
}

/// Publica `probe:status` para a tela de agentes e probes.
pub async fn publish_status(ctx: &AppContext, probe: &probes::Model) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus
            .publish(&ctx.db, "probe:status", status_payload(probe))
            .await
        {
            tracing::warn!(%error, probe_id = probe.id, "falha ao publicar probe:status");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(access_mode: Option<&str>, ip: Option<&str>) -> devices::Model {
        let mut value = serde_json::json!({
            "id": 1, "name": "srv", "type": "server", "status": "online",
            "is_monitored": true, "snmp_enabled": false, "snmp_poll_interval_seconds": 60,
            "created_at": Utc::now(), "updated_at": Utc::now()
        });
        value["access_mode"] = serde_json::to_value(access_mode).expect("modo");
        value["ip_address"] = serde_json::to_value(ip).expect("ip");
        serde_json::from_value(value).expect("dispositivo de teste")
    }

    #[test]
    fn origem_do_tunel_e_exigida_so_para_dispositivo_vpn_com_trava() {
        let vpn = device(Some("vpn"), Some("10.8.0.5"));
        let tunnel: IpAddr = "10.8.0.5".parse().expect("ip");
        let mapped: IpAddr = "::ffff:10.8.0.5".parse().expect("ip");
        let outside: IpAddr = "200.1.2.3".parse().expect("ip");

        assert!(check_tunnel_origin(Some(&vpn), true, Some(tunnel)).is_ok());
        assert!(check_tunnel_origin(Some(&vpn), true, Some(mapped)).is_ok());
        assert!(check_tunnel_origin(Some(&vpn), true, Some(outside)).is_err());
        assert!(check_tunnel_origin(Some(&vpn), true, None).is_err());
        assert!(check_tunnel_origin(Some(&vpn), false, Some(outside)).is_ok());

        let lan = device(Some("local"), Some("192.168.0.9"));
        assert!(check_tunnel_origin(Some(&lan), true, Some(outside)).is_ok());
        assert!(check_tunnel_origin(None, true, Some(outside)).is_ok());
    }

    #[test]
    #[serial_test::serial]
    fn comandos_de_instalacao() {
        std::env::remove_var("AGENT_IMAGE");
        let (docker, systemd) = install_commands("http://10.8.0.1:3333", "nma_abc");
        insta::assert_snapshot!(format!("{docker}\n\n{systemd}"));
    }

    #[test]
    #[serial_test::serial]
    fn imagem_propria_substitui_a_padrao() {
        std::env::set_var("AGENT_IMAGE", "registry.exemplo/netmonitor-agent:1.0");
        let (docker, _) = install_commands("http://10.8.0.1:3333", "nma_abc");
        std::env::remove_var("AGENT_IMAGE");
        assert!(docker.ends_with("registry.exemplo/netmonitor-agent:1.0"));
        let (padrao, _) = install_commands("http://10.8.0.1:3333", "nma_abc");
        assert!(padrao.ends_with(DEFAULT_AGENT_IMAGE));
    }

    #[test]
    fn nome_vazio_ou_com_controle_e_recusado() {
        assert!(validate_name("  ").is_err());
        assert!(validate_name("srv\u{0}").is_err());
        assert_eq!(validate_name(" srv-01 ").expect("nome"), "srv-01");
    }
}
