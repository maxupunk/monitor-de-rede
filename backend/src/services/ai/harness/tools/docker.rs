//! Containers da Docker Engine para o diagnóstico.
//!
//! Só estado e rede — nunca variáveis de ambiente, comando ou labels, que
//! costumam carregar senha e token. Os logs de um container são lidos pelo
//! `grep` (fonte `docker`), que devolve só o que casa.

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use super::{AiToolHandler, ToolArgs, ToolGroup, ToolOutput};
use crate::{
    services::{
        docker::{self, engine, source::LocalEngine, DockerError},
        shared::errors::AppResult,
    },
    views::docker::DockerContainerSummary,
};

const MAX_LISTED: usize = 40;

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
pub(super) async fn resolve_container(identifier: &str) -> Result<DockerContainerSummary, String> {
    let containers = engine::list_containers(&LocalEngine)
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
        "Containers da Docker Engine do servidor: nome, imagem e estado. Com 'container', o detalhe de um (estado, código de saída, reinícios, redes e portas). \
Para os logs de um container, use grep com source='docker'."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "container": { "type": "string", "description": "Nome ou id do container (opcional)" },
                "state": { "type": "string", "description": "Só os neste estado (running, exited, restarting...)" }
            }
        })
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        if let Some(identifier) = args.text("container") {
            let container = match resolve_container(&identifier).await {
                Ok(container) => container,
                Err(message) => return Ok(ToolOutput::not_found(message)),
            };
            let detail = match engine::inspect_container(&LocalEngine, &container.id).await {
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

        let containers = match engine::list_containers(&LocalEngine).await {
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
            "total": containers.len(),
            "running": running,
            "containers": selected.iter().take(MAX_LISTED).map(|c| summary_row(c)).collect::<Vec<_>>(),
            "omitted": selected.len().saturating_sub(MAX_LISTED),
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
