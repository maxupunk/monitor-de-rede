//! Bancos que rodam em contêineres dos agentes: o atalho do "Nova conexão".
//!
//! Todo host Docker é um agente (ADR 014). Aqui a central lista os contêineres
//! de cada um ([`list`], só o resumo — uma chamada por host) e reconhece os de
//! PostgreSQL, MySQL e MariaDB. Ao escolher um, [`resolve`] inspeciona só
//! aquele e monta os endereços pelos quais ele **pode** ser alcançado — na
//! ordem em que vale tentar:
//!
//! 1. o **nome na rede Docker** (`projetox_postgres`, o serviço do compose) —
//!    estável, sobrevive à recriação do contêiner;
//! 2. o **IP do contêiner** em cada rede;
//! 3. a **porta publicada** no host (o gateway da rede, ou o IP em que ela foi
//!    publicada);
//! 4. o **loopback**, para contêiner em `network_mode: host`.
//!
//! [`resolve`] testa pela rota de verdade — direto da central no agente local
//! (todos ao mesmo tempo), pela ponte do agente nos remotos (um por vez, com a
//! mesma política local, ADR 013) — e escolhe o primeiro da lista que
//! respondeu.
//!
//! Senha nunca sai daqui: só o usuário e o banco que as variáveis do
//! contêiner sugerem.

use std::time::Duration;

use futures::future::join_all;
use loco_rs::app::AppContext;
use serde::Serialize;
use ts_rs::TS;

use agent_protocol::{
    docker::views::{DockerContainerDetail, DockerContainerPort, DockerContainerSummary},
    policy::Permission,
};
use netmonitor_core::{
    agents::{bridge::LocalBridge, hub::AgentHub},
    network_tools::tcp_probe::{probe_tcp, TcpProbeState},
    shared::errors::{AppError, AppResult},
};

use super::DatabaseEngine;
use crate::docker::{
    engine::{inspect_container, list_containers},
    hosts::{self, HostKey},
};
use crate::views::docker::DockerHostView;

/// Teto de uma tentativa direta: um endereço que não responde em 3 s não
/// serve para backup de qualquer jeito.
const DIRECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Teto de uma tentativa pela ponte (o agente ainda resolve e conecta).
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(15);

/// Por onde um endereço chega ao contêiner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AddressKind {
    /// Nome do contêiner ou do serviço na rede Docker.
    Dns,
    /// IP do contêiner numa rede.
    ContainerIp,
    /// Porta publicada no host.
    PublishedPort,
    /// `127.0.0.1` — contêiner na rede do host.
    Loopback,
}

/// Um endereço candidato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct CandidateAddress {
    pub kind: AddressKind,
    pub host: String,
    pub port: u16,
    /// A rede Docker do endereço, quando ele é de uma.
    pub network: Option<String>,
}

/// Um contêiner de banco num host Docker, como a lista mostra. Sai só do
/// resumo da Engine (uma chamada por host); endereços e usuário vêm na
/// [`DatabaseContainerResolution`], que inspeciona um contêiner só.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseContainer {
    /// `agent-<id>`.
    pub host_key: String,
    pub host_name: String,
    /// Host da própria central: a conexão sai direto daqui.
    pub local: bool,
    /// A rota da conexão: `None` no host da central, o agente nos demais.
    #[ts(type = "number | null")]
    pub via_probe_id: Option<i64>,
    /// O agente libera a ponte de banco (`database` no `AGENT_ALLOW`).
    pub route_allowed: bool,
    pub container_id: String,
    pub container_name: String,
    pub project: Option<String>,
    pub service: Option<String>,
    pub image: String,
    pub engine: DatabaseEngine,
    pub running: bool,
    pub status: String,
    pub port: u16,
}

/// Host que não pôde ser consultado.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseContainerHostIssue {
    pub host_name: String,
    pub reason: String,
}

/// O que a tela recebe.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseContainersResponse {
    pub containers: Vec<DatabaseContainer>,
    pub issues: Vec<DatabaseContainerHostIssue>,
}

/// Uma tentativa de alcançar um endereço.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AddressAttempt {
    pub address: CandidateAddress,
    pub reachable: bool,
    /// Por que não alcançou.
    pub message: Option<String>,
}

/// O contêiner com o endereço escolhido.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DatabaseContainerResolution {
    pub container: DatabaseContainer,
    /// Usuário sugerido pelas variáveis do contêiner (nunca a senha).
    pub username: Option<String>,
    /// Banco criado pelas variáveis do contêiner.
    pub database: Option<String>,
    /// Do mais recomendado ao último recurso.
    pub addresses: Vec<CandidateAddress>,
    /// O primeiro endereço, na ordem recomendada, que respondeu.
    pub chosen: Option<CandidateAddress>,
    /// As tentativas, na ordem dos endereços (pela ponte, até a primeira
    /// que respondeu).
    pub attempts: Vec<AddressAttempt>,
}

// ---------------------------------------------------------------------------
// Regras puras
// ---------------------------------------------------------------------------

/// O SGBD de um contêiner: pela imagem e, sem pista nela, pela porta exposta.
#[must_use]
pub fn detect_engine(image: &str, private_ports: &[u16]) -> Option<DatabaseEngine> {
    let image = image.to_ascii_lowercase();
    // O nome sem o registro e sem a tag: `docker.io/bitnami/postgresql:16`.
    let name = image.rsplit('/').next().unwrap_or(&image);
    let name = name.split([':', '@']).next().unwrap_or(name);
    let path = image.split([':', '@']).next().unwrap_or(&image);
    if name.contains("mariadb") {
        return Some(DatabaseEngine::Mariadb);
    }
    if ["postgres", "postgis", "timescale", "pgvector"]
        .iter()
        .any(|hint| name.contains(hint) || path.contains(hint))
    {
        return Some(DatabaseEngine::Postgres);
    }
    if ["mysql", "percona"].iter().any(|hint| name.contains(hint)) {
        return Some(DatabaseEngine::Mysql);
    }
    if private_ports.contains(&5432) {
        return Some(DatabaseEngine::Postgres);
    }
    if private_ports.contains(&3306) {
        return Some(DatabaseEngine::Mysql);
    }
    None
}

/// A porta em que o banco escuta dentro do contêiner.
fn listening_port(engine: DatabaseEngine, ports: &[DockerContainerPort]) -> u16 {
    let default = engine.default_port();
    if ports.iter().any(|port| port.private_port == default) {
        return default;
    }
    ports
        .iter()
        .find(|port| port.protocol.is_empty() || port.protocol == "tcp")
        .map_or(default, |port| port.private_port)
}

fn env_value<'a>(environment: &'a [String], names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|wanted| {
        environment.iter().find_map(|entry| {
            let (name, value) = entry.split_once('=')?;
            (name == *wanted && !value.is_empty() && value != "********").then_some(value)
        })
    })
}

/// Usuário e banco que as variáveis da imagem oficial (e da Bitnami)
/// sugerem. Para backup de tudo, o superusuário é o melhor ponto de partida.
#[must_use]
pub fn credentials_hint(
    engine: DatabaseEngine,
    environment: &[String],
) -> (Option<String>, Option<String>) {
    match engine.family() {
        super::EngineFamily::Postgres => (
            Some(
                env_value(environment, &["POSTGRES_USER", "POSTGRESQL_USERNAME"])
                    .unwrap_or("postgres")
                    .to_string(),
            ),
            env_value(environment, &["POSTGRES_DB", "POSTGRESQL_DATABASE"]).map(str::to_string),
        ),
        super::EngineFamily::Mysql => {
            let has_root = environment.iter().any(|entry| {
                entry.starts_with("MYSQL_ROOT_PASSWORD=")
                    || entry.starts_with("MARIADB_ROOT_PASSWORD=")
            });
            let user = if has_root {
                None
            } else {
                env_value(environment, &["MYSQL_USER", "MARIADB_USER"])
            };
            (
                Some(user.unwrap_or("root").to_string()),
                env_value(environment, &["MYSQL_DATABASE", "MARIADB_DATABASE"]).map(str::to_string),
            )
        }
    }
}

/// O id curto que o Docker põe nos aliases: não é nome que alguém digitaria.
fn is_container_id(alias: &str, container_id: &str) -> bool {
    !alias.is_empty() && container_id.starts_with(alias) && alias.len() >= 12
}

fn unspecified(ip: &str) -> bool {
    ip.is_empty() || ip == "0.0.0.0" || ip == "::"
}

/// Os endereços do contêiner, do mais recomendado ao último recurso, sem
/// repetição.
#[must_use]
pub fn candidate_addresses(
    detail: &DockerContainerDetail,
    ports: &[DockerContainerPort],
    port: u16,
    local: bool,
) -> Vec<CandidateAddress> {
    let mut out: Vec<CandidateAddress> = Vec::new();
    let mut push = |kind, host: &str, port, network: Option<&str>| {
        let host = host.trim();
        if host.is_empty() || out.iter().any(|a| a.host == host && a.port == port) {
            return;
        }
        out.push(CandidateAddress {
            kind,
            host: host.to_string(),
            port,
            network: network.map(str::to_string),
        });
    };
    let networks: Vec<_> = detail
        .networks
        .iter()
        .filter(|network| !matches!(network.network_name.as_str(), "host" | "none"))
        .collect();

    // 1. Nomes: o do contêiner vale em toda rede definida pelo usuário; os
    //    aliases (o serviço do compose), na rede de cada um. A `bridge`
    //    padrão do Docker não tem DNS — nome ali nunca responde.
    let named: Vec<_> = networks
        .iter()
        .filter(|network| network.network_name != "bridge")
        .collect();
    if let Some(first) = named.first() {
        push(
            AddressKind::Dns,
            &detail.name,
            port,
            Some(&first.network_name),
        );
    }
    for network in &named {
        for alias in &network.aliases {
            if !is_container_id(alias, &detail.id) {
                push(AddressKind::Dns, alias, port, Some(&network.network_name));
            }
        }
    }
    // 2. IP do contêiner em cada rede.
    for network in &networks {
        push(
            AddressKind::ContainerIp,
            &network.ip_address,
            port,
            Some(&network.network_name),
        );
    }
    // 3. Porta publicada: no IP em que foi publicada, ou no gateway das redes
    //    (o próprio host visto de dentro de um contêiner). No host da central,
    //    também pelo nome do host no Docker Desktop e pelo loopback — o caso
    //    da central instalada fora de contêiner.
    for published in ports
        .iter()
        .filter(|p| p.private_port == port && p.public_port.is_some() && p.protocol != "udp")
    {
        let public = published.public_port.unwrap_or(port);
        match published.ip.as_deref() {
            Some(ip) if !unspecified(ip) => push(AddressKind::PublishedPort, ip, public, None),
            _ => {
                for network in &networks {
                    push(
                        AddressKind::PublishedPort,
                        &network.gateway,
                        public,
                        Some(&network.network_name),
                    );
                }
                if local {
                    push(
                        AddressKind::PublishedPort,
                        "host.docker.internal",
                        public,
                        None,
                    );
                    push(AddressKind::PublishedPort, "127.0.0.1", public, None);
                }
            }
        }
    }
    // 4. Rede do host: o banco escuta no próprio host.
    if detail.host_config.network_mode == "host" {
        push(AddressKind::Loopback, "127.0.0.1", port, None);
    }
    out
}

/// O item da lista, a partir do resumo da Engine.
#[must_use]
pub fn summarize(
    host: &DockerHostView,
    summary: &DockerContainerSummary,
    engine: DatabaseEngine,
) -> DatabaseContainer {
    let local = host.kind == "local";
    DatabaseContainer {
        host_key: host.key.clone(),
        host_name: host.name.clone(),
        local,
        via_probe_id: (!local).then_some(host.agent_id),
        route_allowed: local || host.policy.contains(&Permission::Database),
        container_id: summary.id.clone(),
        container_name: summary
            .names
            .first()
            .map(|name| name.trim_start_matches('/').to_string())
            .unwrap_or_else(|| summary.id.chars().take(12).collect()),
        project: summary.project_name.clone(),
        service: summary.labels.get("com.docker.compose.service").cloned(),
        image: summary.image.clone(),
        engine,
        running: summary.state == "running",
        status: summary.status.clone(),
        port: listening_port(engine, &summary.ports),
    }
}

/// O primeiro endereço, na ordem recomendada, que respondeu.
#[must_use]
pub fn choose(attempts: &[AddressAttempt]) -> Option<CandidateAddress> {
    attempts
        .iter()
        .find(|attempt| attempt.reachable)
        .map(|attempt| attempt.address.clone())
}

// ---------------------------------------------------------------------------
// Consulta aos hosts e teste de alcance
// ---------------------------------------------------------------------------

/// Os contêineres de banco de um host.
async fn host_containers(
    ctx: &AppContext,
    host: &DockerHostView,
) -> Result<Vec<DatabaseContainer>, String> {
    let key: HostKey = host
        .key
        .parse()
        .map_err(|error: AppError| error.to_string())?;
    let docker = hosts::resolve(ctx, key).map_err(|error| error.to_string())?;
    let summaries = list_containers(docker.engine.as_ref())
        .await
        .map_err(|error| error.to_string())?;
    let found: Vec<_> = summaries
        .iter()
        .filter_map(|summary| {
            let ports: Vec<u16> = summary.ports.iter().map(|p| p.private_port).collect();
            detect_engine(&summary.image, &ports).map(|engine| (summary, engine))
        })
        .collect();
    Ok(found
        .into_iter()
        .map(|(summary, engine)| summarize(host, summary, engine))
        .collect())
}

/// Os hosts que podem ter contêineres agora: conectados e com Docker.
async fn docker_hosts(ctx: &AppContext) -> AppResult<Vec<DockerHostView>> {
    Ok(hosts::list(ctx)
        .await?
        .into_iter()
        .filter(|host| host.online && host.docker_available)
        .collect())
}

/// Os contêineres de banco de todos os hosts Docker conectados. Host que
/// falha vai para `issues`, sem derrubar a lista.
///
/// # Errors
///
/// Erro do banco ao listar os hosts.
pub async fn list(ctx: &AppContext) -> AppResult<DatabaseContainersResponse> {
    let hosts = docker_hosts(ctx).await?;
    let results = join_all(hosts.iter().map(|host| host_containers(ctx, host))).await;
    let mut response = DatabaseContainersResponse {
        containers: Vec::new(),
        issues: Vec::new(),
    };
    for (host, result) in hosts.iter().zip(results) {
        match result {
            Ok(mut containers) => response.containers.append(&mut containers),
            Err(reason) => response.issues.push(DatabaseContainerHostIssue {
                host_name: host.name.clone(),
                reason,
            }),
        }
    }
    // Rodando primeiro; depois o host da central; depois pelo nome.
    response.containers.sort_by(|a, b| {
        (
            !a.running,
            !a.local,
            a.host_name.as_str(),
            a.container_name.as_str(),
        )
            .cmp(&(
                !b.running,
                !b.local,
                b.host_name.as_str(),
                b.container_name.as_str(),
            ))
    });
    Ok(response)
}

fn tcp_message(state: TcpProbeState, error: Option<String>) -> String {
    match state {
        TcpProbeState::Closed => "A porta recusou a conexão".to_string(),
        TcpProbeState::Filtered => "Não respondeu a tempo".to_string(),
        _ => error.unwrap_or_else(|| "Não foi possível conectar".to_string()),
    }
}

/// Tenta um endereço pela rota da conexão.
async fn attempt(
    hub: Option<&AgentHub>,
    via_probe_id: Option<i64>,
    address: CandidateAddress,
) -> AddressAttempt {
    let outcome: Result<(), String> = match via_probe_id {
        None => {
            let observed = probe_tcp((address.host.as_str(), address.port), DIRECT_TIMEOUT).await;
            if observed.state == TcpProbeState::Open {
                Ok(())
            } else {
                Err(tcp_message(observed.state, observed.error))
            }
        }
        Some(probe_id) => match hub {
            None => Err("O agente não está conectado".to_string()),
            Some(hub) => tokio::time::timeout(
                BRIDGE_TIMEOUT,
                LocalBridge::start(hub.clone(), probe_id, address.host.clone(), address.port),
            )
            .await
            .map_err(|_| "O agente não respondeu a tempo".to_string())
            .and_then(|opened| opened.map(drop).map_err(|error| error.message)),
        },
    };
    AddressAttempt {
        reachable: outcome.is_ok(),
        message: outcome.err(),
        address,
    }
}

/// Testa os endereços pela rota da conexão.
///
/// Direto, todos ao mesmo tempo — uma tentativa TCP não custa nada. Pela
/// ponte, um por vez, na ordem, parando no primeiro que responde: o agente
/// abre no máximo duas pontes simultâneas (ADR 013), e testar todos juntos
/// faria ele recusar endereços bons.
async fn attempt_all(
    hub: Option<&AgentHub>,
    via_probe_id: Option<i64>,
    addresses: &[CandidateAddress],
) -> Vec<AddressAttempt> {
    if via_probe_id.is_none() {
        return join_all(
            addresses
                .iter()
                .cloned()
                .map(|address| attempt(hub, None, address)),
        )
        .await;
    }
    let mut attempts = Vec::new();
    for address in addresses {
        let tried = attempt(hub, via_probe_id, address.clone()).await;
        let reached = tried.reachable;
        attempts.push(tried);
        if reached {
            break;
        }
    }
    attempts
}

/// Lê o contêiner de novo e testa os endereços dele pela rota que a conexão
/// vai usar ([`attempt_all`]).
///
/// # Errors
///
/// Host inválido ou desconectado, contêiner que não existe mais ou que não é
/// de banco.
pub async fn resolve(
    ctx: &AppContext,
    host_key: &str,
    container_id: &str,
) -> AppResult<DatabaseContainerResolution> {
    let key: HostKey = host_key.parse()?;
    let host = docker_hosts(ctx)
        .await?
        .into_iter()
        .find(|host| host.key == key.to_string())
        .ok_or_else(|| {
            AppError::service_unavailable("O agente deste host não está conectado à central")
        })?;
    let docker = hosts::resolve(ctx, key)?;
    // O resumo (portas publicadas) e o detalhe (redes, variáveis) vêm de duas
    // chamadas à Engine pelo agente: juntas, custam o tempo de uma.
    let (summaries, detail) = tokio::join!(
        list_containers(docker.engine.as_ref()),
        inspect_container(docker.engine.as_ref(), container_id)
    );
    let summary = summaries
        .map_err(|error| AppError::business_rule(error.to_string()))?
        .into_iter()
        .find(|summary| summary.id == container_id)
        .ok_or_else(|| AppError::not_found("O contêiner não existe mais neste host"))?;
    let detail = detail.map_err(|error| AppError::business_rule(error.to_string()))?;
    let ports: Vec<u16> = summary.ports.iter().map(|p| p.private_port).collect();
    let engine = detect_engine(&summary.image, &ports)
        .ok_or_else(|| AppError::validation("Este contêiner não parece ser um banco de dados"))?;
    let container = summarize(&host, &summary, engine);
    let addresses = candidate_addresses(&detail, &summary.ports, container.port, container.local);
    let (username, database) = credentials_hint(engine, &detail.config.environment);

    let hub = AgentHub::from_context(ctx).ok();
    let attempts = attempt_all(hub.as_ref(), container.via_probe_id, &addresses).await;
    Ok(DatabaseContainerResolution {
        container,
        username,
        database,
        chosen: choose(&attempts),
        addresses,
        attempts,
    })
}

#[cfg(test)]
mod tests {
    use agent_protocol::docker::views::{
        DockerContainerConfig, DockerContainerHostConfig, DockerContainerState,
        DockerNetworkEndpoint, DockerRestartPolicy,
    };

    use std::collections::HashMap;

    use super::*;

    fn port(private: u16, public: Option<u16>, ip: Option<&str>) -> DockerContainerPort {
        DockerContainerPort {
            ip: ip.map(str::to_string),
            private_port: private,
            public_port: public,
            protocol: "tcp".into(),
        }
    }

    fn network(name: &str, ip: &str, gateway: &str, aliases: &[&str]) -> DockerNetworkEndpoint {
        DockerNetworkEndpoint {
            network_id: format!("{name}-id"),
            network_name: name.into(),
            ip_address: ip.into(),
            gateway: gateway.into(),
            aliases: aliases.iter().map(|alias| (*alias).to_string()).collect(),
        }
    }

    fn detail(networks: Vec<DockerNetworkEndpoint>, mode: &str) -> DockerContainerDetail {
        DockerContainerDetail {
            id: "a1b2c3d4e5f6a7b8".into(),
            name: "projetox_postgres".into(),
            image: "postgres:16".into(),
            image_id: String::new(),
            created: String::new(),
            state: DockerContainerState {
                status: "running".into(),
                running: true,
                paused: false,
                restarting: false,
                pid: 1,
                started_at: String::new(),
                finished_at: String::new(),
                exit_code: 0,
            },
            config: DockerContainerConfig {
                hostname: String::new(),
                environment: Vec::new(),
                command: Vec::new(),
                entrypoint: Vec::new(),
                labels: HashMap::new(),
                working_dir: String::new(),
                user: String::new(),
            },
            host_config: DockerContainerHostConfig {
                restart_policy: DockerRestartPolicy {
                    name: String::new(),
                    maximum_retry_count: 0,
                },
                network_mode: mode.into(),
            },
            mounts: Vec::new(),
            networks,
        }
    }

    #[test]
    fn reconhece_o_sgbd_pela_imagem_e_pela_porta() {
        let cases = [
            ("postgres:16-alpine", vec![], Some(DatabaseEngine::Postgres)),
            (
                "docker.io/bitnami/postgresql:16",
                vec![],
                Some(DatabaseEngine::Postgres),
            ),
            (
                "postgis/postgis:15-3.4",
                vec![],
                Some(DatabaseEngine::Postgres),
            ),
            (
                "timescale/timescaledb:latest-pg16",
                vec![],
                Some(DatabaseEngine::Postgres),
            ),
            ("mariadb:11", vec![], Some(DatabaseEngine::Mariadb)),
            ("bitnami/mariadb", vec![], Some(DatabaseEngine::Mariadb)),
            ("mysql:8.4", vec![], Some(DatabaseEngine::Mysql)),
            (
                "percona/percona-server:8.0",
                vec![],
                Some(DatabaseEngine::Mysql),
            ),
            (
                "empresa/erp-db:2",
                vec![5432],
                Some(DatabaseEngine::Postgres),
            ),
            ("empresa/legado:1", vec![3306], Some(DatabaseEngine::Mysql)),
            ("nginx:latest", vec![80], None),
            ("redis:7", vec![6379], None),
        ];
        for (image, ports, expected) in cases {
            assert_eq!(detect_engine(image, &ports), expected, "{image}");
        }
    }

    #[test]
    fn rota_e_permissao_vem_do_host() {
        let host = |kind: &str, policy: Vec<Permission>| DockerHostView {
            key: "agent-7".into(),
            name: "filial".into(),
            kind: kind.into(),
            online: true,
            agent_id: 7,
            docker_available: true,
            compose_available: false,
            policy,
        };
        let summary = DockerContainerSummary {
            id: "abc".into(),
            names: vec!["/erp_db".into()],
            image: "postgres:16".into(),
            image_id: String::new(),
            state: "running".into(),
            status: "Up".into(),
            labels: HashMap::from([("com.docker.compose.service".into(), "db".into())]),
            ports: vec![port(5432, None, None)],
            created: 0,
            project_name: Some("erp".into()),
            protected: false,
        };
        let local = summarize(&host("local", vec![]), &summary, DatabaseEngine::Postgres);
        assert_eq!((local.via_probe_id, local.route_allowed), (None, true));
        assert_eq!(local.container_name, "erp_db");
        assert_eq!(local.service.as_deref(), Some("db"));

        let closed = summarize(&host("agent", vec![]), &summary, DatabaseEngine::Postgres);
        assert_eq!(
            (closed.via_probe_id, closed.route_allowed),
            (Some(7), false)
        );
        let open = summarize(
            &host("agent", vec![Permission::Database]),
            &summary,
            DatabaseEngine::Postgres,
        );
        assert!(open.route_allowed);
    }

    #[test]
    fn usuario_sugerido_vem_das_variaveis_sem_senha() {
        let env = |items: &[&str]| items.iter().map(|i| (*i).to_string()).collect::<Vec<_>>();
        assert_eq!(
            credentials_hint(
                DatabaseEngine::Postgres,
                &env(&[
                    "POSTGRES_USER=erp",
                    "POSTGRES_DB=vendas",
                    "POSTGRES_PASSWORD=********"
                ])
            ),
            (Some("erp".into()), Some("vendas".into()))
        );
        assert_eq!(
            credentials_hint(DatabaseEngine::Postgres, &[]),
            (Some("postgres".into()), None)
        );
        // Com a senha de root definida, o root copia tudo.
        assert_eq!(
            credentials_hint(
                DatabaseEngine::Mysql,
                &env(&[
                    "MYSQL_ROOT_PASSWORD=********",
                    "MYSQL_USER=app",
                    "MYSQL_DATABASE=loja"
                ])
            ),
            (Some("root".into()), Some("loja".into()))
        );
        assert_eq!(
            credentials_hint(DatabaseEngine::Mariadb, &env(&["MARIADB_USER=app"])),
            (Some("app".into()), None)
        );
    }

    #[test]
    fn enderecos_vao_do_nome_ao_ip_e_a_porta_publicada() {
        let detail = detail(
            vec![network(
                "projetox_default",
                "172.20.0.5",
                "172.20.0.1",
                &["postgres", "a1b2c3d4e5f6", "projetox_postgres"],
            )],
            "projetox_default",
        );
        let addresses = candidate_addresses(
            &detail,
            &[
                port(5432, Some(15432), Some("0.0.0.0")),
                port(5432, Some(15432), Some("::")),
            ],
            5432,
            false,
        );
        let flat: Vec<_> = addresses
            .iter()
            .map(|a| (a.kind, a.host.as_str(), a.port))
            .collect();
        assert_eq!(
            flat,
            vec![
                (AddressKind::Dns, "projetox_postgres", 5432),
                (AddressKind::Dns, "postgres", 5432),
                (AddressKind::ContainerIp, "172.20.0.5", 5432),
                (AddressKind::PublishedPort, "172.20.0.1", 15432),
            ]
        );
        assert_eq!(addresses[0].network.as_deref(), Some("projetox_default"));
    }

    #[test]
    fn porta_publicada_num_ip_especifico_usa_esse_ip() {
        let detail = detail(
            vec![network("bridge", "172.17.0.3", "172.17.0.1", &[])],
            "default",
        );
        let addresses = candidate_addresses(
            &detail,
            &[port(3306, Some(3306), Some("10.0.0.20"))],
            3306,
            true,
        );
        assert!(addresses
            .iter()
            .any(|a| a.kind == AddressKind::PublishedPort
                && a.host == "10.0.0.20"
                && a.port == 3306));
        assert!(!addresses
            .iter()
            .any(|a| a.kind == AddressKind::PublishedPort && a.host == "172.17.0.1"));
    }

    #[test]
    fn bridge_padrao_nao_tem_nome_e_o_host_da_central_tenta_o_loopback() {
        let detail = detail(
            vec![network("bridge", "172.17.0.4", "172.17.0.1", &[])],
            "default",
        );
        let published = [port(5432, Some(5491), Some("0.0.0.0"))];
        let hosts = |local| {
            candidate_addresses(&detail, &published, 5432, local)
                .into_iter()
                .map(|a| (a.kind, a.host))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            hosts(true),
            vec![
                (AddressKind::ContainerIp, "172.17.0.4".to_string()),
                (AddressKind::PublishedPort, "172.17.0.1".to_string()),
                (
                    AddressKind::PublishedPort,
                    "host.docker.internal".to_string()
                ),
                (AddressKind::PublishedPort, "127.0.0.1".to_string()),
            ]
        );
        // Num agente remoto o loopback seria o do agente, não o do host.
        assert_eq!(hosts(false).len(), 2);
    }

    #[test]
    fn rede_do_host_usa_o_loopback() {
        let detail = detail(vec![network("host", "", "", &[])], "host");
        let addresses = candidate_addresses(&detail, &[], 5432, false);
        assert_eq!(
            addresses,
            vec![CandidateAddress {
                kind: AddressKind::Loopback,
                host: "127.0.0.1".into(),
                port: 5432,
                network: None,
            }]
        );
    }

    #[test]
    fn escolhe_o_primeiro_da_ordem_que_respondeu() {
        let address = |host: &str| CandidateAddress {
            kind: AddressKind::Dns,
            host: host.into(),
            port: 5432,
            network: None,
        };
        let attempts = vec![
            AddressAttempt {
                address: address("projetox_postgres"),
                reachable: false,
                message: Some("Não respondeu a tempo".into()),
            },
            AddressAttempt {
                address: address("172.20.0.5"),
                reachable: true,
                message: None,
            },
            AddressAttempt {
                address: address("172.20.0.1"),
                reachable: true,
                message: None,
            },
        ];
        assert_eq!(choose(&attempts).map(|a| a.host), Some("172.20.0.5".into()));
        assert_eq!(choose(&attempts[..1]), None);
    }

    #[tokio::test]
    async fn direto_testa_todos_e_pela_ponte_para_no_primeiro() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let open = listener.local_addr().expect("addr").port();
        let address = |port| CandidateAddress {
            kind: AddressKind::Loopback,
            host: "127.0.0.1".into(),
            port,
            network: None,
        };
        // Porta fechada antes da aberta: direto, as duas são tentadas.
        let closed = {
            let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("porta livre");
            probe.local_addr().expect("addr").port()
        };
        let direct = attempt_all(None, None, &[address(closed), address(open)]).await;
        assert_eq!(
            direct.iter().map(|a| a.reachable).collect::<Vec<_>>(),
            vec![false, true]
        );
        assert_eq!(choose(&direct).map(|a| a.port), Some(open));

        // Pela ponte de um agente que não está conectado: tenta cada um, na
        // ordem, e diz por quê.
        let bridged = attempt_all(None, Some(7), &[address(open), address(closed)]).await;
        assert_eq!(bridged.len(), 2);
        assert!(bridged
            .iter()
            .all(|a| !a.reachable && a.message.as_deref() == Some("O agente não está conectado")));
    }

    #[tokio::test]
    async fn tentativa_direta_numa_porta_aberta_alcanca() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let port = listener.local_addr().expect("addr").port();
        let address = CandidateAddress {
            kind: AddressKind::Loopback,
            host: "127.0.0.1".into(),
            port,
            network: None,
        };
        let reached = attempt(None, None, address.clone()).await;
        assert!(reached.reachable, "{:?}", reached.message);
        drop(listener);
        let refused = attempt(None, None, address).await;
        assert!(!refused.reachable);
    }
}
