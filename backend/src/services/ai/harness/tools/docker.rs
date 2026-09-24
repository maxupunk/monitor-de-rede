//! Containers da Docker Engine para o diagnóstico — da central e de cada
//! agente remoto conectado (ADR 011).
//!
//! Só estado, rede e consumo — nunca variáveis de ambiente, comando ou
//! labels, que costumam carregar senha e token. Os logs de um container são
//! lidos pelo `grep` (fonte `docker`), que devolve só o que casa.

use async_trait::async_trait;
use chrono::Utc;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use super::{
    actions::{audit, VIA_AI},
    series::round2,
    AiToolHandler, ToolArgs, ToolGroup, ToolKind, ToolOutput,
};
use crate::{
    services::{
        agents::policy::Permission,
        audit::{AuditAction, AuditEntryInput, ResourceType},
        docker::{
            self,
            engine::{self, ContainerAction},
            hosts::{self, DockerHost, HostKey},
            realtime,
            source::DockerEngine,
            DockerError,
        },
        shared::errors::AppResult,
        telemetry::store,
    },
    views::{docker::DockerContainerSummary, telemetry::MetricsRange},
};

const MAX_LISTED: usize = 40;
/// Containers no ranking de consumo.
const MAX_USAGE_ROWS: usize = 15;
const MIB: f64 = 1024.0 * 1024.0;

/// Descrição do argumento `host`, igual em todas as ferramentas Docker.
pub(super) const HOST_ARG: &str =
    "Servidor Docker: nome do agente remoto ou 'local' (padrão: esta central). get_docker_hosts lista";

/// O host Docker pelo nome do agente, pela chave (`local`, `agent-3`) ou
/// pelo id; sem nada, esta central. `Err(mensagem)` para a IA quando não dá.
pub(super) async fn resolve_host(
    ctx: &AppContext,
    identifier: Option<&str>,
) -> AppResult<Result<DockerHost, String>> {
    let wanted = identifier.map(str::trim).unwrap_or_default().to_lowercase();
    if wanted.is_empty() || wanted == "local" || wanted == "central" {
        return Ok(Ok(DockerHost::local()));
    }
    let listed = hosts::list(ctx).await?;
    let exact = listed.iter().find(|host| {
        host.key.eq_ignore_ascii_case(&wanted)
            || host.name.eq_ignore_ascii_case(&wanted)
            || host.agent_id.is_some_and(|id| id.to_string() == wanted)
    });
    let found = exact.or_else(|| {
        let mut partial = listed
            .iter()
            .filter(|host| host.name.to_lowercase().contains(&wanted));
        match (partial.next(), partial.next()) {
            (Some(only), None) => Some(only),
            _ => None,
        }
    });
    let Some(found) = found else {
        return Ok(Err(format!(
            "Servidor Docker '{wanted}' não encontrado ou ambíguo; use get_docker_hosts"
        )));
    };
    let Ok(key) = found.key.parse::<HostKey>() else {
        return Ok(Err(format!("Servidor Docker '{}' inválido", found.name)));
    };
    Ok(hosts::resolve(ctx, key).map_err(|error| format!("{}: {error}", found.name)))
}

/// Mensagem para a IA quando a Engine não está ao alcance.
pub(super) fn unavailable_message(error: &DockerError) -> String {
    match error {
        DockerError::Disabled => docker::DISABLED_REASON.to_string(),
        _ => docker::UNAVAILABLE_REASON.to_string(),
    }
}

/// Escolhe o container: prefixo do id, nome exato, depois trecho único do
/// nome. Ambiguidade devolve `None` — a IA pergunta em vez de olhar o errado.
#[must_use]
pub(super) fn pick_container<'a>(
    list: &'a [DockerContainerSummary],
    identifier: &str,
) -> Option<&'a DockerContainerSummary> {
    let wanted = identifier.trim().trim_start_matches('/').to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let by_id = |container: &&DockerContainerSummary| {
        wanted.len() >= 4 && container.id.to_lowercase().starts_with(&wanted)
    };
    let exact =
        |container: &&DockerContainerSummary| container.display_name().to_lowercase() == wanted;
    if let Some(found) = list.iter().find(by_id).or_else(|| list.iter().find(exact)) {
        return Some(found);
    }
    let mut partial = list
        .iter()
        .filter(|container| container.display_name().to_lowercase().contains(&wanted));
    match (partial.next(), partial.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// O container pelo nome ou id; `Err(mensagem)` para a IA quando não dá.
pub(super) async fn resolve_container(
    source: &dyn DockerEngine,
    identifier: &str,
) -> Result<DockerContainerSummary, String> {
    let containers = engine::list_containers(source)
        .await
        .map_err(|error| unavailable_message(&error))?;
    pick_container(&containers, identifier)
        .cloned()
        .ok_or_else(|| {
            format!("Container '{identifier}' não encontrado ou ambíguo; use get_docker_containers")
        })
}

fn summary_row(container: &DockerContainerSummary) -> Value {
    json!({
        "name": container.display_name(),
        "image": container.image,
        "state": container.state,
        "status": container.status,
        "project": container.project_name,
    })
}

pub struct DockerContainers;

#[async_trait]
impl AiToolHandler for DockerContainers {
    fn name(&self) -> &'static str {
        "get_docker_containers"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "Containers de um servidor Docker (a central ou um agente remoto): nome, imagem e estado. Com 'container', o detalhe de um (estado, código de saída, reinícios, redes e portas). \
Consumo de CPU/memória: get_docker_usage. Logs: grep com source='docker'."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "container": { "type": "string", "description": "Nome ou id do container (opcional)" },
                "state": { "type": "string", "description": "Só os neste estado (running, exited, restarting...)" },
                "host": { "type": "string", "description": HOST_ARG }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let host = match resolve_host(ctx, args.text("host").as_deref()).await? {
            Ok(host) => host,
            Err(message) => return Ok(ToolOutput::not_found(message)),
        };
        let source = host.engine.as_ref();
        if let Some(identifier) = args.text("container") {
            let container = match resolve_container(source, &identifier).await {
                Ok(container) => container,
                Err(message) => return Ok(ToolOutput::not_found(message)),
            };
            let detail = match engine::inspect_container(source, &container.id).await {
                Ok(detail) => detail,
                Err(error) => return Ok(ToolOutput::not_found(unavailable_message(&error))),
            };
            return Ok(ToolOutput::data(json!({
                "name": container.display_name(),
                "image": detail.image,
                "state": detail.state.status,
                "status": container.status,
                "restarting": detail.state.restarting,
                "started_at": detail.state.started_at,
                "finished_at": detail.state.finished_at,
                "exit_code": detail.state.exit_code,
                "restart_policy": detail.host_config.restart_policy.name,
                "networks": detail.networks.iter().map(|network| json!({
                    "name": network.network_name,
                    "ip": network.ip_address,
                })).collect::<Vec<_>>(),
                "ports": container.ports.iter().map(|port| match port.public_port {
                    Some(public) => format!("{public}->{}/{}", port.private_port, port.protocol),
                    None => format!("{}/{}", port.private_port, port.protocol),
                }).collect::<Vec<_>>(),
            })));
        }

        let containers = match engine::list_containers(source).await {
            Ok(containers) => containers,
            Err(error) => return Ok(ToolOutput::not_found(unavailable_message(&error))),
        };
        let state = args.text("state").map(|state| state.to_lowercase());
        let selected: Vec<&DockerContainerSummary> = containers
            .iter()
            .filter(|container| {
                state
                    .as_deref()
                    .is_none_or(|state| container.state.eq_ignore_ascii_case(state))
            })
            .collect();
        let running = containers
            .iter()
            .filter(|container| container.state == "running")
            .count();
        Ok(ToolOutput::data(json!({
            "host": host.key.to_string(),
            "total": containers.len(),
            "running": running,
            "containers": selected.iter().take(MAX_LISTED).map(|c| summary_row(c)).collect::<Vec<_>>(),
            "omitted": selected.len().saturating_sub(MAX_LISTED),
        })))
    }
}

pub struct DockerHosts;

#[async_trait]
impl AiToolHandler for DockerHosts {
    fn name(&self) -> &'static str {
        "get_docker_hosts"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "Servidores Docker que o NetMonitor alcança: esta central e cada agente remoto, com conexão e se tem Docker/Compose. O 'name' serve de 'host' nas demais ferramentas Docker."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, ctx: &AppContext, _args: &ToolArgs) -> AppResult<ToolOutput> {
        let listed = hosts::list(ctx).await?;
        Ok(ToolOutput::data(json!({
            "hosts": listed.iter().map(|host| json!({
                "name": host.name,
                "key": host.key,
                "online": host.online,
                "docker": host.docker_available,
                "compose": host.compose_available,
            })).collect::<Vec<_>>(),
        })))
    }
}

/// A janela de telemetria que cobre `hours`.
const fn range_for(hours: i64) -> MetricsRange {
    match hours {
        ..=1 => MetricsRange::OneHour,
        2..=6 => MetricsRange::SixHours,
        7..=24 => MetricsRange::OneDay,
        25..=168 => MetricsRange::SevenDays,
        _ => MetricsRange::ThirtyDays,
    }
}

#[allow(clippy::cast_precision_loss)]
fn mib(bytes: u64) -> f64 {
    round2(bytes as f64 / MIB)
}

/// Média, pico e último valor de uma série, sem mandar os pontos.
fn stats(values: impl Iterator<Item = f64>) -> Value {
    let values: Vec<f64> = values.collect();
    let Some(last) = values.last().copied() else {
        return Value::Null;
    };
    let max = values.iter().copied().fold(f64::MIN, f64::max);
    #[allow(clippy::cast_precision_loss)]
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    json!({ "avg": round2(avg), "max": round2(max), "last": round2(last) })
}

pub struct DockerUsage;

#[async_trait]
impl AiToolHandler for DockerUsage {
    fn name(&self) -> &'static str {
        "get_docker_usage"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "Consumo de CPU e memória. Sem 'hours': agora, os containers que mais consomem. Com 'hours': o histórico gravado — do servidor (CPU, memória, carga, disco) e o ranking de containers na janela; com 'container', a série resumida só dele."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "host": { "type": "string", "description": HOST_ARG },
                "container": { "type": "string", "description": "Nome do container (opcional)" },
                "hours": { "type": "integer", "description": "Janela do histórico em horas (1 a 720). Omita para o consumo de agora" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let host = match resolve_host(ctx, args.text("host").as_deref()).await? {
            Ok(host) => host,
            Err(message) => return Ok(ToolOutput::not_found(message)),
        };
        let container = args.text("container").map(|name| name.to_lowercase());
        let Some(hours) = args.integer("hours").map(|hours| hours.clamp(1, 720)) else {
            return Ok(live_usage(ctx, &host, container.as_deref()).await);
        };

        let range = range_for(hours);
        let host_key = host.key.to_string();
        let now = Utc::now();
        if let Some(name) = container {
            let points = store::container_history(&ctx.db, &host_key, &name, range, now).await?;
            if points.is_empty() {
                return Ok(ToolOutput::not_found(format!(
                    "Sem histórico gravado para o container '{name}' na janela; confira o nome com get_docker_containers"
                )));
            }
            return Ok(ToolOutput::data(json!({
                "host": host_key,
                "container": name,
                "points": points.len(),
                "cpu_pct": stats(points.iter().map(|point| point.cpu_avg)),
                "cpu_peak_pct": round2(points.iter().map(|point| point.cpu_max).fold(0.0, f64::max)),
                "mem_mib": stats(points.iter().map(|point| point.memory_avg / MIB)),
            })));
        }

        let (server, containers) = tokio::try_join!(
            store::host_history(&ctx.db, &host_key, range, now),
            store::container_usage(&ctx.db, &host_key, range, now)
        )?;
        let server_summary = server.last().map(|last| {
            json!({
                "cpu_pct": stats(server.iter().map(|point| point.cpu_avg)),
                "mem_used_mib": stats(server.iter().map(|point| point.memory_used / MIB)),
                "mem_total_mib": mib(u64::try_from(last.memory_total).unwrap_or(0)),
                "load1": stats(server.iter().map(|point| point.load1)),
                "disks": last.disks,
            })
        });
        Ok(ToolOutput::data(json!({
            "host": host_key,
            "hours": hours,
            "server": server_summary,
            "containers": containers.iter().take(MAX_USAGE_ROWS).map(|row| json!({
                "name": row.name,
                "cpu_avg_pct": round2(row.cpu_avg),
                "cpu_max_pct": round2(row.cpu_max),
                "mem_max_mib": mib(u64::try_from(row.memory_max).unwrap_or(0)),
            })).collect::<Vec<_>>(),
            "omitted": containers.len().saturating_sub(MAX_USAGE_ROWS),
        })))
    }
}

/// Consumo de agora: o ranking por CPU, do cache de métricas do host.
async fn live_usage(ctx: &AppContext, host: &DockerHost, container: Option<&str>) -> ToolOutput {
    let metrics = host.metrics(ctx).await;
    if !metrics.docker_available {
        return ToolOutput::not_found(
            metrics
                .unavailable_reason
                .unwrap_or_else(|| docker::UNAVAILABLE_REASON.to_string()),
        );
    }
    let mut rows: Vec<_> = metrics
        .containers
        .iter()
        .filter(|row| {
            container.is_none_or(|wanted| row.container_name.to_lowercase().contains(wanted))
        })
        .collect();
    rows.sort_by(|a, b| b.cpu.usage_percent.total_cmp(&a.cpu.usage_percent));
    ToolOutput::data(json!({
        "host": host.key.to_string(),
        "collected_at": metrics.collected_at,
        "containers": rows.iter().take(MAX_USAGE_ROWS).map(|row| json!({
            "name": row.container_name,
            "cpu_pct": round2(row.cpu.usage_percent),
            "mem_mib": mib(row.memory.usage_bytes),
            "mem_pct": round2(row.memory.usage_percent),
        })).collect::<Vec<_>>(),
        "omitted": rows.len().saturating_sub(MAX_USAGE_ROWS),
    }))
}

/// O que a IA pode fazer com um container. Remover e atualizar ficam de fora:
/// são destrutivos demais para um modo automático.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleAction {
    Start,
    Stop,
    Restart,
}

impl LifecycleAction {
    const NAMES: [&'static str; 3] = ["start", "stop", "restart"];

    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "start" | "iniciar" => Some(Self::Start),
            "stop" | "parar" => Some(Self::Stop),
            "restart" | "reiniciar" => Some(Self::Restart),
            _ => None,
        }
    }

    const fn verb(self) -> &'static str {
        match self {
            Self::Start => "Iniciar",
            Self::Stop => "Parar",
            Self::Restart => "Reiniciar",
        }
    }

    const fn done(self) -> &'static str {
        match self {
            Self::Start => "iniciado",
            Self::Stop => "parado",
            Self::Restart => "reiniciado",
        }
    }

    const fn engine(self) -> ContainerAction {
        match self {
            Self::Start => ContainerAction::Start,
            Self::Stop => ContainerAction::Stop,
            Self::Restart => ContainerAction::Restart,
        }
    }
}

/// O alvo resolvido: host, container e ação — ou a mensagem para a IA.
struct Target {
    host: DockerHost,
    host_name: String,
    container: DockerContainerSummary,
    action: LifecycleAction,
}

/// Resolve host, container e ação e confere a permissão do host. A política
/// de um agente é a dele (`AGENT_ALLOW`, ADR 011): sem `lifecycle`, a central
/// nem pede.
async fn resolve_target(ctx: &AppContext, args: &ToolArgs) -> AppResult<Result<Target, String>> {
    let Some(action) = args
        .text("action")
        .as_deref()
        .and_then(LifecycleAction::parse)
    else {
        return Ok(Err(format!(
            "Informe 'action': {}",
            LifecycleAction::NAMES.join(", ")
        )));
    };
    let Some(identifier) = args.text("container") else {
        return Ok(Err("Informe 'container' (nome ou id)".into()));
    };
    let host = match resolve_host(ctx, args.text("host").as_deref()).await? {
        Ok(host) => host,
        Err(message) => return Ok(Err(message)),
    };
    let host_name = if host.key.is_local() {
        "central".to_string()
    } else {
        let listed = hosts::list(ctx).await?;
        let view = listed.iter().find(|view| view.key == host.key.to_string());
        if !view.is_some_and(|view| view.policy.contains(&Permission::Lifecycle)) {
            return Ok(Err(
                "A política deste agente (AGENT_ALLOW) não permite iniciar/parar containers".into(),
            ));
        }
        view.map_or_else(|| host.key.to_string(), |view| view.name.clone())
    };
    let container = match resolve_container(host.engine.as_ref(), &identifier).await {
        Ok(container) => container,
        Err(message) => return Ok(Err(message)),
    };
    Ok(Ok(Target {
        host,
        host_name,
        container,
        action,
    }))
}

pub struct DockerContainerAction;

#[async_trait]
impl AiToolHandler for DockerContainerAction {
    fn name(&self) -> &'static str {
        "docker_container_action"
    }

    fn kind(&self) -> ToolKind {
        ToolKind::ContainerAction
    }

    fn description(&self) -> &'static str {
        "Inicia, para ou reinicia um container (da central ou de um agente remoto). Use quando resolver o pedido — ex: container parado ou travado. \
Conforme a configuração, roda direto ou pede confirmação ao usuário."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": LifecycleAction::NAMES },
                "container": { "type": "string", "description": "Nome ou id do container" },
                "host": { "type": "string", "description": HOST_ARG }
            },
            "required": ["action", "container"]
        })
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        Ok(match resolve_target(ctx, args).await? {
            Ok(target) => format!(
                "{} o container {} ({})",
                target.action.verb(),
                target.container.display_name(),
                target.host_name
            ),
            Err(message) => message,
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let target = match resolve_target(ctx, args).await? {
            Ok(target) => target,
            Err(message) => return Ok(ToolOutput::not_found(message)),
        };
        let name = target.container.display_name();
        if let Err(error) = engine::container_action(
            target.host.engine.as_ref(),
            &target.container.id,
            target.action.engine(),
        )
        .await
        {
            return Ok(ToolOutput::not_found(format!(
                "Falha ao {} {name}: {error}",
                target.action.verb().to_lowercase()
            )));
        }
        realtime::refresh(ctx, target.host.key).await;
        let label = if target.host.key.is_local() {
            name.clone()
        } else {
            format!("{}/{name}", target.host.key)
        };
        audit(
            ctx,
            args,
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::DockerContainer,
                resource_id: None,
                resource_label: Some(label),
                description: Some(format!(
                    "Container {name} {} ({VIA_AI})",
                    target.action.done()
                )),
                changes: None,
            },
        )
        .await;
        Ok(ToolOutput::data(json!({
            "ok": true,
            "container": name,
            "host": target.host_name,
            "action": target.action.done(),
        })))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn container(id: &str, name: &str) -> DockerContainerSummary {
        DockerContainerSummary {
            id: id.into(),
            names: vec![format!("/{name}")],
            image: "img".into(),
            image_id: String::new(),
            state: "running".into(),
            status: "Up".into(),
            labels: HashMap::new(),
            ports: Vec::new(),
            created: 0,
            project_name: None,
        }
    }

    #[test]
    fn acao_aceita_ingles_e_portugues_e_recusa_remover() {
        assert_eq!(
            LifecycleAction::parse("Restart"),
            Some(LifecycleAction::Restart)
        );
        assert_eq!(LifecycleAction::parse("parar"), Some(LifecycleAction::Stop));
        assert_eq!(LifecycleAction::parse("remove"), None);
        assert_eq!(LifecycleAction::parse("update"), None);
    }

    #[test]
    fn janela_de_telemetria_cobre_as_horas_pedidas() {
        assert_eq!(range_for(1), MetricsRange::OneHour);
        assert_eq!(range_for(5), MetricsRange::SixHours);
        assert_eq!(range_for(24), MetricsRange::OneDay);
        assert_eq!(range_for(48), MetricsRange::SevenDays);
        assert_eq!(range_for(500), MetricsRange::ThirtyDays);
    }

    #[test]
    fn serie_vira_media_pico_e_ultimo() {
        assert_eq!(
            stats([1.0, 3.0, 2.0].into_iter()),
            json!({ "avg": 2.0, "max": 3.0, "last": 2.0 })
        );
        assert_eq!(stats(std::iter::empty()), Value::Null);
    }

    #[test]
    fn escolhe_por_id_nome_exato_ou_trecho_unico() {
        let lista = vec![
            container("abcdef123456", "netmonitor-api"),
            container("0123456789ab", "netmonitor-api-worker"),
            container("fedcba987654", "postgres"),
        ];
        let nome =
            |termo: &str| pick_container(&lista, termo).map(DockerContainerSummary::display_name);
        assert_eq!(nome("abcd").as_deref(), Some("netmonitor-api"));
        assert_eq!(
            nome("/NETMONITOR-API").as_deref(),
            Some("netmonitor-api"),
            "exato vence trecho"
        );
        assert_eq!(nome("post").as_deref(), Some("postgres"));
        assert_eq!(nome("netmonitor"), None, "dois casam: ambíguo");
        assert_eq!(nome("abc"), None, "id curto demais não vale");
    }
}
