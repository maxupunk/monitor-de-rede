//! Projetos Docker Compose: descoberta pelas labels e execução no agente.
//!
//! A Engine não conhece compose — só as labels que o CLI grava nos
//! containers. A descoberta e a montagem da linha de comando são puras e
//! rodam em qualquer processo. A **execução** ([`run`]) só é chamada pelo
//! agente, no host remoto: o processo da API nunca executa o CLI `docker`
//! (ADR 010, AGENTS §7).

use std::{collections::BTreeMap, process::Stdio, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use ts_rs::TS;

use crate::views::docker::{DockerActionResponse, DockerContainerSummary};

use super::{
    maintenance::{ComposeAction, ComposeRequest, Progress},
    DockerError,
};

const PROJECT_LABEL: &str = "com.docker.compose.project";
const WORKING_DIR_LABEL: &str = "com.docker.compose.project.working_dir";
const CONFIG_FILES_LABEL: &str = "com.docker.compose.project.config_files";
const SERVICE_LABEL: &str = "com.docker.compose.service";
const RUN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// Um serviço do projeto e o estado dos containers dele (réplicas incluídas).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ComposeService {
    pub name: String,
    pub containers: usize,
    pub running: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ComposeProject {
    pub name: String,
    pub working_dir: String,
    pub config_files: Vec<String>,
    pub services: Vec<ComposeService>,
    pub containers: usize,
    pub running: usize,
}

impl ComposeProject {
    #[must_use]
    pub fn has_service(&self, name: &str) -> bool {
        self.services.iter().any(|service| service.name == name)
    }
}

/// Agrupa os containers pelas labels do compose. Containers sem diretório de
/// trabalho conhecido ficam de fora: sem ele não há como executar o CLI.
#[must_use]
pub fn projects(containers: &[DockerContainerSummary]) -> Vec<ComposeProject> {
    let mut grouped: BTreeMap<String, ComposeProject> = BTreeMap::new();
    for container in containers {
        let Some(name) = container
            .labels
            .get(PROJECT_LABEL)
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        let Some(working_dir) = container
            .labels
            .get(WORKING_DIR_LABEL)
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        let project = grouped
            .entry(name.clone())
            .or_insert_with(|| ComposeProject {
                name: name.clone(),
                working_dir: working_dir.clone(),
                config_files: container
                    .labels
                    .get(CONFIG_FILES_LABEL)
                    .map(|files| {
                        files
                            .split(',')
                            .map(str::trim)
                            .filter(|file| !file.is_empty())
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                services: Vec::new(),
                containers: 0,
                running: 0,
            });
        let running = usize::from(container.state == "running");
        project.containers += 1;
        project.running += running;
        if let Some(name) = container.labels.get(SERVICE_LABEL) {
            match project
                .services
                .iter_mut()
                .find(|service| &service.name == name)
            {
                Some(service) => {
                    service.containers += 1;
                    service.running += running;
                }
                None => project.services.push(ComposeService {
                    name: name.clone(),
                    containers: 1,
                    running,
                }),
            }
        }
    }
    let mut output: Vec<_> = grouped.into_values().collect();
    for project in &mut output {
        project.services.sort_by(|a, b| a.name.cmp(&b.name));
    }
    output
}

/// Argumentos do CLI (`docker <args>`). Nunca passa por shell: cada item é
/// um argumento, então nome de projeto ou serviço não vira injeção.
///
/// # Errors
///
/// `Validation` para projeto inexistente, serviço desconhecido ou `down` de
/// um serviço só (o compose não tem essa operação).
pub fn build_args(
    known: &[ComposeProject],
    request: &ComposeRequest,
) -> Result<Vec<String>, DockerError> {
    let project = known
        .iter()
        .find(|project| project.name == request.project)
        .ok_or_else(|| {
            DockerError::Validation("Projeto compose não encontrado neste host".into())
        })?;
    if let Some(service) = &request.service {
        if !project.has_service(service) {
            return Err(DockerError::Validation(
                "Serviço não pertence ao projeto compose".into(),
            ));
        }
        if request.action == ComposeAction::Down {
            return Err(DockerError::Validation(
                "`down` age sobre o projeto inteiro; para um serviço use stop".into(),
            ));
        }
    }

    let mut args = vec![
        "compose".to_string(),
        "--project-name".to_string(),
        project.name.clone(),
        "--project-directory".to_string(),
        project.working_dir.clone(),
    ];
    for file in &project.config_files {
        args.push("--file".into());
        args.push(file.clone());
    }
    match request.action {
        ComposeAction::Pull => args.push("pull".into()),
        ComposeAction::Up => args.extend(["up".into(), "--detach".into()]),
        ComposeAction::Restart => args.push("restart".into()),
        ComposeAction::Stop => args.push("stop".into()),
        ComposeAction::Down => args.push("down".into()),
    }
    if let Some(service) = &request.service {
        // `--` separa opções de nomes: um serviço chamado `-v` não vira flag.
        args.push("--".into());
        args.push(service.clone());
    }
    Ok(args)
}

/// Se o CLI com o plugin compose existe neste host.
pub async fn availability() -> Result<String, String> {
    let output = tokio::time::timeout(
        VERSION_TIMEOUT,
        tokio::process::Command::new("docker")
            .args(["compose", "version", "--short"])
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| "o CLI docker não respondeu".to_string())?
    .map_err(|_| "CLI docker não instalado".to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("plugin docker compose indisponível".to_string())
    }
}

/// Executa o CLI e repassa cada linha de saída como progresso.
///
/// # Errors
///
/// `Unavailable` quando o CLI não existe ou estoura o tempo; `Validation`
/// com as últimas linhas quando o compose termina com erro.
pub async fn run(
    args: Vec<String>,
    request: &ComposeRequest,
    progress: &Progress,
) -> Result<DockerActionResponse, DockerError> {
    let mut child = tokio::process::Command::new("docker")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| DockerError::Unavailable)?;
    let stdout = child.stdout.take().ok_or(DockerError::Engine)?;
    let stderr = child.stderr.take().ok_or(DockerError::Engine)?;
    let (lines_tx, mut lines_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    for reader in [
        Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Unpin + Send>,
        Box::new(stderr),
    ] {
        let lines_tx = lines_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if lines_tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    drop(lines_tx);

    let mut tail: Vec<String> = Vec::new();
    let collect = async {
        while let Some(line) = lines_rx.recv().await {
            let _ = progress.send(line.clone());
            tail.push(line);
            if tail.len() > 20 {
                tail.remove(0);
            }
        }
        child.wait().await
    };
    let status = tokio::time::timeout(RUN_TIMEOUT, collect)
        .await
        .map_err(|_| DockerError::Unavailable)?
        .map_err(|_| DockerError::Engine)?;
    let target = request.service.as_ref().map_or_else(
        || request.project.clone(),
        |service| format!("{}/{service}", request.project),
    );
    if status.success() {
        Ok(DockerActionResponse {
            success: true,
            message: format!("compose {} concluído em {target}.", request.action.label()),
        })
    } else {
        Err(DockerError::Validation(format!(
            "compose {} falhou em {target}: {}",
            request.action.label(),
            tail.join(" | ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn container(project: &str, service: &str, state: &str) -> DockerContainerSummary {
        DockerContainerSummary {
            id: format!("{project}-{service}"),
            names: vec![format!("/{project}-{service}-1")],
            image: "img".into(),
            image_id: String::new(),
            state: state.into(),
            status: String::new(),
            labels: HashMap::from([
                (PROJECT_LABEL.to_string(), project.to_string()),
                (WORKING_DIR_LABEL.to_string(), format!("/srv/{project}")),
                (
                    CONFIG_FILES_LABEL.to_string(),
                    format!("/srv/{project}/compose.yml,/srv/{project}/compose.prod.yml"),
                ),
                (SERVICE_LABEL.to_string(), service.to_string()),
            ]),
            ports: Vec::new(),
            created: 0,
            project_name: Some(project.into()),
        }
    }

    fn known() -> Vec<ComposeProject> {
        projects(&[
            container("portal", "web", "running"),
            container("portal", "db", "exited"),
            container("erp", "api", "running"),
        ])
    }

    #[test]
    fn agrupa_projetos_pelas_labels() {
        let projects = known();
        assert_eq!(projects.len(), 2);
        let portal = projects
            .iter()
            .find(|p| p.name == "portal")
            .expect("portal");
        let services: Vec<_> = portal
            .services
            .iter()
            .map(|service| (service.name.as_str(), service.running))
            .collect();
        assert_eq!(services, vec![("db", 0), ("web", 1)]);
        assert_eq!((portal.containers, portal.running), (2, 1));
        assert_eq!(portal.config_files.len(), 2);
    }

    #[test]
    fn linhas_de_comando_do_compose() {
        let known = known();
        let cases = [
            ("portal", ComposeAction::Pull, None),
            ("portal", ComposeAction::Up, None),
            ("portal", ComposeAction::Up, Some("web")),
            ("portal", ComposeAction::Restart, Some("db")),
            ("erp", ComposeAction::Stop, None),
            ("erp", ComposeAction::Down, None),
        ];
        let rendered: Vec<String> = cases
            .iter()
            .map(|(project, action, service)| {
                build_args(
                    &known,
                    &ComposeRequest {
                        project: (*project).into(),
                        action: *action,
                        service: service.map(ToString::to_string),
                    },
                )
                .expect("args")
                .join(" ")
            })
            .collect();
        insta::assert_snapshot!(rendered.join("\n"));
    }

    #[test]
    fn recusa_projeto_servico_ou_down_parcial_invalidos() {
        let known = known();
        let request = |project: &str, action, service: Option<&str>| ComposeRequest {
            project: project.into(),
            action,
            service: service.map(ToString::to_string),
        };
        assert!(build_args(&known, &request("outro", ComposeAction::Up, None)).is_err());
        assert!(build_args(&known, &request("portal", ComposeAction::Up, Some("api"))).is_err());
        assert!(build_args(&known, &request("portal", ComposeAction::Down, Some("web"))).is_err());
    }
}
