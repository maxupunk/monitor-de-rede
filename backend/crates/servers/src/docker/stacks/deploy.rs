//! A sequência de um deploy no agente: sincronizar, gravar, baixar, subir,
//! limpar.
//!
//! Nada é compilado no host (`--no-build`): as imagens chegam prontas do
//! registry, na tag do commit. O `up --wait` delega ao próprio compose a
//! espera dos healthchecks; se ele falha, as últimas linhas dos serviços que
//! caíram vão ao console antes do erro — é o que a tela mostra em vermelho.
//!
//! O rollback **não** mora aqui: é a central que pede outro deploy, com o
//! commit, a tag e o `.env` da versão anterior. O agente só sabe deployar
//! uma revisão.

use std::{
    collections::BTreeSet,
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use agent_protocol::docker::{stacks::validate_project, views::DockerContainerSummary};
use tokio::sync::mpsc;

use super::{
    super::{
        cli::{Cli, CliOutcome},
        compose::{self, PROJECT_LABEL, SERVICE_LABEL},
        engine,
        maintenance::Progress,
        source::DockerEngine,
        DockerError,
    },
    env_file, git, images,
    registry::EphemeralDockerConfig,
    StackChunk, StackDeployOutcome, StackDeployRequest, StackEvents, StackServiceState, StackStage,
    StacksRoot,
};

/// Folga sobre o `--wait-timeout`: o compose ainda recria containers antes
/// de começar a esperar.
const START_GRACE: Duration = Duration::from_secs(5 * 60);
const PULL_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const LOGS_TIMEOUT: Duration = Duration::from_secs(30);
/// Linhas de log de cada serviço que caiu.
const FAILED_SERVICE_LINES: &str = "40";

fn stage(events: &StackEvents, stage: StackStage) {
    let _ = events.send(StackChunk::Stage { stage });
}

fn line(events: &StackEvents, text: impl Into<String>) {
    let _ = events.send(StackChunk::Line { text: text.into() });
}

/// Roda `work` com um [`Progress`] cujas linhas viram `StackChunk::Line`,
/// e só volta quando a última linha foi entregue: a etapa seguinte nunca
/// chega à tela antes das linhas da anterior.
async fn with_lines<T, F, Fut>(events: &StackEvents, work: F) -> T
where
    F: FnOnce(Progress) -> Fut,
    Fut: Future<Output = T>,
{
    let (progress, mut lines) = mpsc::unbounded_channel::<String>();
    let forward = {
        let events = events.clone();
        tokio::spawn(async move {
            while let Some(text) = lines.recv().await {
                if events.send(StackChunk::Line { text }).is_err() {
                    break;
                }
            }
        })
    };
    let result = work(progress).await;
    let _ = forward.await;
    result
}

/// `docker compose` do projeto, com os arquivos do repositório.
#[must_use]
pub fn compose_args(project: &str, dir: &Path, files: &[String], command: &[&str]) -> Vec<String> {
    let mut args = vec![
        "compose".to_string(),
        "--project-name".to_string(),
        project.to_string(),
        "--project-directory".to_string(),
        dir.display().to_string(),
    ];
    for file in files {
        args.push("--file".into());
        args.push(dir.join(file).display().to_string());
    }
    args.extend(command.iter().map(ToString::to_string));
    args
}

/// `up` que não compila, remove serviços que saíram do arquivo e espera os
/// healthchecks.
#[must_use]
pub fn up_command(wait_timeout_secs: u32, pull: bool) -> Vec<String> {
    let mut up: Vec<String> = vec![
        "up".into(),
        "--detach".into(),
        "--no-build".into(),
        "--remove-orphans".into(),
    ];
    if !pull {
        // A central entregou as imagens: nada de registry no `up`.
        up.extend(["--pull".into(), "never".into()]);
    }
    up.extend([
        "--wait".into(),
        "--wait-timeout".into(),
        wait_timeout_secs.to_string(),
    ]);
    up
}

/// O `compose pull`. Com as imagens entregues pela central (`pull: false`),
/// só baixa o que falta (o banco, um proxy): a imagem da aplicação já está
/// no host e não existe em registry público nenhum.
#[must_use]
pub const fn pull_command(pull: bool) -> &'static [&'static str] {
    if pull {
        &["pull"]
    } else {
        &["pull", "--policy", "missing"]
    }
}

/// Ambiente do compose: a tag da imagem, saída sem cor e, com login, o
/// `DOCKER_CONFIG` temporário.
#[must_use]
pub fn compose_env(
    request: &StackDeployRequest,
    docker_config: Option<&Path>,
) -> Vec<(String, String)> {
    let mut envs = vec![
        (
            request.image_tag_variable.clone(),
            request.image_tag.clone(),
        ),
        ("COMPOSE_ANSI".to_string(), "never".to_string()),
        ("COMPOSE_PROGRESS".to_string(), "plain".to_string()),
    ];
    if let Some(path) = docker_config {
        envs.push(("DOCKER_CONFIG".to_string(), path.display().to_string()));
    }
    envs
}

/// `healthy`/`unhealthy`/`starting` do texto de estado do Docker
/// (`Up 3 minutes (healthy)`).
#[must_use]
pub fn health_of(status: &str) -> Option<String> {
    if status.contains("(unhealthy)") {
        Some("unhealthy".into())
    } else if status.contains("(healthy)") {
        Some("healthy".into())
    } else if status.contains("(health: starting)") {
        Some("starting".into())
    } else {
        None
    }
}

fn belongs(container: &DockerContainerSummary, project: &str) -> bool {
    container.labels.get(PROJECT_LABEL).map(String::as_str) == Some(project)
}

/// Os serviços do projeto, como estão agora.
#[must_use]
pub fn services_of(containers: &[DockerContainerSummary], project: &str) -> Vec<StackServiceState> {
    let mut output: Vec<StackServiceState> = containers
        .iter()
        .filter(|container| belongs(container, project))
        .map(|container| StackServiceState {
            name: container
                .labels
                .get(SERVICE_LABEL)
                .cloned()
                .unwrap_or_else(|| container.id.chars().take(12).collect()),
            image: container.image.clone(),
            state: container.state.clone(),
            health: health_of(&container.status),
        })
        .collect();
    output.sort_by(|a, b| a.name.cmp(&b.name));
    output
}

/// Serviços que caíram ou não ficaram saudáveis.
#[must_use]
pub fn failed_services(services: &[StackServiceState]) -> Vec<String> {
    let mut names: Vec<String> = services
        .iter()
        .filter(|service| {
            service.state != "running" || service.health.as_deref() == Some("unhealthy")
        })
        .map(|service| service.name.clone())
        .collect();
    names.dedup();
    names
}

async fn refuse_protected(engine: &dyn DockerEngine, project: &str) -> Result<(), DockerError> {
    let containers = engine::list_containers(engine).await?;
    if compose::projects(&containers)
        .iter()
        .any(|known| known.name == project && known.protected)
    {
        return Err(DockerError::Forbidden(
            "Projeto da instalação NetMonitor: o deploy derrubaria a própria central".into(),
        ));
    }
    Ok(())
}

struct Compose<'a> {
    project: &'a str,
    dir: &'a Path,
    files: &'a [String],
    envs: &'a [(String, String)],
}

impl Compose<'_> {
    async fn run(
        &self,
        events: &StackEvents,
        command: &[&str],
        timeout: Duration,
    ) -> Result<CliOutcome, DockerError> {
        let args = compose_args(self.project, self.dir, self.files, command);
        with_lines(events, |progress| async move {
            Cli::new("docker", &args, timeout)
                .envs(self.envs)
                .cwd(self.dir)
                .run(&progress)
                .await
        })
        .await
    }
}

/// Executa o deploy pedido.
///
/// # Errors
///
/// `Validation`/`Forbidden` antes de qualquer efeito; `Validation` com as
/// últimas linhas da etapa que falhou.
pub async fn run(
    engine: &dyn DockerEngine,
    root: &StacksRoot,
    request: &StackDeployRequest,
    events: &StackEvents,
) -> Result<StackDeployOutcome, DockerError> {
    request.validate()?;
    validate_project(&request.project)?;
    root.check().await.map_err(DockerError::Validation)?;
    refuse_protected(engine, &request.project).await?;
    let dir: PathBuf = root.project_dir(&request.project);

    stage(events, StackStage::Sync);
    let commit = with_lines(events, |progress| async move {
        git::sync(&dir, &request.source, &progress).await
    })
    .await?;
    let dir = root.project_dir(&request.project);
    line(events, format!("Repositório no commit {commit}"));

    if let Some(env) = &request.env_file {
        stage(events, StackStage::Env);
        env_file::write(&dir, env.expose()).await?;
        line(events, ".env gravado (0600)");
    }

    let registry = request
        .registry
        .as_ref()
        .map(EphemeralDockerConfig::create)
        .transpose()?;
    let envs = compose_env(request, registry.as_ref().map(EphemeralDockerConfig::path));
    let compose = Compose {
        project: &request.project,
        dir: &dir,
        files: &request.compose_files,
        envs: &envs,
    };

    stage(events, StackStage::Pull);
    let pulled = compose
        .run(events, pull_command(request.pull), PULL_TIMEOUT)
        .await?;
    if !pulled.success {
        return Err(DockerError::Validation(format!(
            "Não foi possível baixar as imagens da tag {}: {}",
            request.image_tag,
            pulled.summary()
        )));
    }

    stage(events, StackStage::Start);
    let up = up_command(request.wait_timeout_secs, request.pull);
    let up: Vec<&str> = up.iter().map(String::as_str).collect();
    let started = compose
        .run(
            events,
            &up,
            Duration::from_secs(u64::from(request.wait_timeout_secs)) + START_GRACE,
        )
        .await?;
    // A credencial não precisa durar além do `up` (que ainda pode baixar).
    drop(registry);

    let containers = engine::list_containers(engine).await?;
    let services = services_of(&containers, &request.project);
    if !started.success {
        let failed = failed_services(&services);
        for service in &failed {
            line(events, format!("── últimas linhas de {service} ──"));
            let _ = compose
                .run(
                    events,
                    &[
                        "logs",
                        "--no-color",
                        "--tail",
                        FAILED_SERVICE_LINES,
                        service,
                    ],
                    LOGS_TIMEOUT,
                )
                .await;
        }
        return Err(DockerError::Validation(if failed.is_empty() {
            format!("Os containers não ficaram saudáveis: {}", started.summary())
        } else {
            format!("Não ficaram saudáveis: {}", failed.join(", "))
        }));
    }

    stage(events, StackStage::Prune);
    let removed_images = prune(engine, &containers, &request.project, request.keep_images).await;
    if removed_images > 0 {
        line(
            events,
            format!("{removed_images} imagem(ns) antiga(s) removida(s)"),
        );
    }
    Ok(StackDeployOutcome {
        commit,
        services,
        removed_images,
    })
}

/// Remove as tags antigas das imagens da stack e as camadas órfãs. Falha
/// aqui não desfaz o deploy: é só limpeza.
async fn prune(
    engine: &dyn DockerEngine,
    containers: &[DockerContainerSummary],
    project: &str,
    keep: u32,
) -> u32 {
    let mine: Vec<&DockerContainerSummary> = containers
        .iter()
        .filter(|container| belongs(container, project))
        .collect();
    let repositories: BTreeSet<String> = mine
        .iter()
        .map(|container| images::repository_of(&container.image).to_string())
        .collect();
    let in_use: BTreeSet<String> = mine
        .iter()
        .map(|container| container.image_id.clone())
        .collect();
    let Ok(listed) = engine::list_images(engine).await else {
        return 0;
    };
    let mut removed = 0;
    for id in images::to_remove(&listed, &repositories, &in_use, keep) {
        if engine.remove_image(&id, false).await.is_ok() {
            removed += 1;
        }
    }
    let _ = engine.prune_images_raw().await;
    removed
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn container(
        project: &str,
        service: &str,
        state: &str,
        status: &str,
    ) -> DockerContainerSummary {
        DockerContainerSummary {
            id: format!("{project}-{service}"),
            names: vec![format!("/{project}-{service}")],
            image: format!("ghcr.io/a/{service}:abc"),
            image_id: format!("sha256:{service}"),
            state: state.into(),
            status: status.into(),
            labels: HashMap::from([
                (PROJECT_LABEL.to_string(), project.to_string()),
                (SERVICE_LABEL.to_string(), service.to_string()),
            ]),
            ports: Vec::new(),
            created: 0,
            project_name: Some(project.into()),
            protected: false,
        }
    }

    #[test]
    fn compose_usa_os_arquivos_do_repositorio() {
        let dir = Path::new("/srv/netmonitor/stacks/sgc");
        let args = compose_args("sgc", dir, &["docker-compose.yml".into()], &["pull"]);
        // O separador vem do `Path` (o agente roda em Linux; o teste, onde for).
        let file = dir.join("docker-compose.yml").display().to_string();
        assert_eq!(
            args,
            [
                "compose",
                "--project-name",
                "sgc",
                "--project-directory",
                "/srv/netmonitor/stacks/sgc",
                "--file",
                file.as_str(),
                "pull"
            ]
        );
    }

    #[test]
    fn up_nunca_compila_e_espera_os_healthchecks() {
        let up = up_command(180, true);
        for flag in ["--no-build", "--wait", "--remove-orphans", "--detach"] {
            assert!(up.contains(&flag.to_string()), "{flag}");
        }
        assert!(!up.contains(&"--pull".to_string()));
        assert_eq!(up.last().map(String::as_str), Some("180"));
    }

    #[test]
    fn imagem_da_central_nao_vai_ao_registry() {
        // Visto no e2e real: sem isso o `compose pull` procura a imagem da
        // aplicação no Docker Hub e o deploy falha com ela já no host.
        assert_eq!(pull_command(true), ["pull"]);
        assert_eq!(pull_command(false), ["pull", "--policy", "missing"]);
        let up = up_command(60, false);
        let at = up.iter().position(|arg| arg == "--pull").expect("--pull");
        assert_eq!(up[at + 1], "never");
        assert_eq!(up.last().map(String::as_str), Some("60"));
    }

    #[test]
    fn tag_vai_pelo_ambiente() {
        let request = StackDeployRequest {
            project: "sgc".into(),
            source: agent_protocol::docker::stacks::GitSource {
                repository_url: "https://github.com/a/b.git".into(),
                commit: "abcdef1".into(),
                token: None,
            },
            compose_files: vec!["docker-compose.yml".into()],
            env_file: None,
            image_tag_variable: "IMAGE_TAG".into(),
            image_tag: "abcdef1".into(),
            registry: None,
            wait_timeout_secs: 60,
            keep_images: 2,
            pull: true,
        };
        let envs = compose_env(&request, Some(Path::new("/tmp/cfg")));
        assert!(envs.contains(&("IMAGE_TAG".into(), "abcdef1".into())));
        assert!(envs.contains(&("DOCKER_CONFIG".into(), "/tmp/cfg".into())));
    }

    #[test]
    fn saude_lida_do_estado() {
        assert_eq!(
            health_of("Up 3 minutes (healthy)").as_deref(),
            Some("healthy")
        );
        assert_eq!(
            health_of("Up 1 second (health: starting)").as_deref(),
            Some("starting")
        );
        assert_eq!(
            health_of("Up 2 minutes (unhealthy)").as_deref(),
            Some("unhealthy")
        );
        assert_eq!(health_of("Up 2 minutes"), None);
    }

    #[test]
    fn falhas_sao_os_parados_e_os_doentes() {
        let containers = vec![
            container("sgc", "api", "running", "Up (unhealthy)"),
            container("sgc", "nginx", "running", "Up (healthy)"),
            container("sgc", "worker", "exited", "Exited (1)"),
            container("outro", "db", "exited", "Exited (1)"),
        ];
        let services = services_of(&containers, "sgc");
        assert_eq!(services.len(), 3);
        assert_eq!(failed_services(&services), vec!["api", "worker"]);
    }
}
