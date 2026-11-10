//! A compilação no host do agente (o lado que executa).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use bollard::image::ImportImageOptions;
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use agent_protocol::tunnel::TunnelStream;

use super::{
    artifacts::{BuildsRoot, WorkDir},
    buildx::{self, BuilderLimits, BUILDER, BUILD_NETWORK},
    BuildChunk, BuildEvents, BuildStage, BuilderReadiness, BuiltArtifact, ImageBuildOutcome,
    ImageBuildRequest, ImageHost, ImageImportRequest, ImportReport,
};
use crate::docker::{
    call,
    cli::{self, Cli},
    client,
    maintenance::Progress,
    source::LocalEngine,
    stacks::{git, readiness::parse_df},
    DockerError,
};

const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const PREPARE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Teto do `docker load` de uma imagem.
const IMPORT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

fn stage(events: &BuildEvents, stage: BuildStage, image: Option<&str>) {
    let _ = events.send(BuildChunk::Stage {
        stage,
        image: image.map(ToString::to_string),
    });
}

fn line(events: &BuildEvents, text: impl Into<String>) {
    let _ = events.send(BuildChunk::Line { text: text.into() });
}

/// Repassa as linhas de um CLI como linhas do console.
async fn with_lines<T, F, Fut>(events: &BuildEvents, work: F) -> T
where
    F: FnOnce(Progress) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let (progress, mut lines) = tokio::sync::mpsc::unbounded_channel::<String>();
    let events = events.clone();
    let forward = tokio::spawn(async move {
        while let Some(text) = lines.recv().await {
            if events.send(BuildChunk::Line { text }).is_err() {
                break;
            }
        }
    });
    let result = work(progress).await;
    let _ = forward.await;
    result
}

/// O tempo que resta à compilação.
fn remaining(deadline: Instant) -> Result<Duration, DockerError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| DockerError::Validation("A compilação passou do tempo limite".into()))
}

async fn docker(
    events: &BuildEvents,
    args: &[String],
    envs: &[(String, String)],
    timeout: Duration,
) -> Result<cli::CliOutcome, DockerError> {
    with_lines(events, |progress| async move {
        Cli::new("docker", args, timeout)
            .envs(envs)
            .run(&progress)
            .await
    })
    .await
}

async fn quiet(args: &[&str]) -> bool {
    cli::capture("docker", args, None, CHECK_TIMEOUT)
        .await
        .is_ok_and(|(ok, _)| ok)
}

fn host_cpus() -> Option<u32> {
    std::thread::available_parallelism()
        .ok()
        .and_then(|count| u32::try_from(count.get()).ok())
}

/// O builder dedicado, criado na primeira compilação (com a rede dele).
async fn ensure_builder(events: &BuildEvents) -> Result<(), DockerError> {
    if quiet(&["buildx", "inspect", "--bootstrap", BUILDER]).await {
        return Ok(());
    }
    if !quiet(&["buildx", "version"]).await {
        return Err(DockerError::Unsupported(
            "O plugin docker buildx não está instalado neste host".into(),
        ));
    }
    line(
        events,
        "Criando o builder dedicado (primeira compilação neste host)…",
    );
    if !quiet(&["network", "inspect", BUILD_NETWORK]).await
        && !quiet(&["network", "create", BUILD_NETWORK]).await
    {
        return Err(DockerError::Validation(format!(
            "Não foi possível criar a rede {BUILD_NETWORK} do builder"
        )));
    }
    let args = buildx::create_args(&BuilderLimits::from_env(host_cpus()));
    let created = docker(events, &args, &[], PREPARE_TIMEOUT).await?;
    if created.success {
        Ok(())
    } else {
        Err(DockerError::Validation(format!(
            "O builder dedicado não subiu: {}",
            created.summary()
        )))
    }
}

/// A compilação inteira.
async fn build(
    request: &ImageBuildRequest,
    events: &BuildEvents,
) -> Result<ImageBuildOutcome, DockerError> {
    request.validate()?;
    let deadline = Instant::now() + Duration::from_secs(u64::from(request.timeout_secs));
    let root = BuildsRoot::from_env();
    root.prepare().await?;

    stage(events, BuildStage::Prepare, None);
    ensure_builder(events).await?;

    stage(events, BuildStage::Clone, None);
    let work = WorkDir(root.work_dir(Uuid::new_v4()));
    let dir = work.0.clone();
    let commit = with_lines(events, |progress| async move {
        git::sync(&dir, &request.source, &progress).await
    })
    .await?;
    line(events, format!("Repositório no commit {commit}"));

    let envs = buildx::secret_envs(request);
    if let Some(target) = &request.test_target {
        stage(events, BuildStage::Test, None);
        if let Some(args) = buildx::test_args(request, &work.0, target) {
            let tested = docker(events, &args, &envs, remaining(deadline)?).await?;
            if !tested.success {
                return Err(DockerError::Validation(format!(
                    "Os testes (estágio {target}) falharam: {}",
                    tested.summary()
                )));
            }
            line(events, "✔ Testes passaram");
        }
    }

    let mut artifacts = Vec::with_capacity(request.images.len());
    for image in &request.images {
        stage(events, BuildStage::Build, Some(&image.name));
        let artifact = Uuid::new_v4();
        let output = root.artifact_path(artifact);
        let args = buildx::image_args(request, image, &work.0, &output);
        let built = docker(events, &args, &envs, remaining(deadline)?).await?;
        if !built.success {
            let _ = tokio::fs::remove_file(&output).await;
            return Err(DockerError::Validation(format!(
                "A imagem {} não compilou: {}",
                image.name,
                built.summary()
            )));
        }
        let bytes = tokio::fs::metadata(&output)
            .await
            .map(|meta| meta.len())
            .map_err(|_| {
                DockerError::Validation(format!("O buildx não gerou o arquivo de {}", image.name))
            })?;
        artifacts.push(BuiltArtifact {
            image: image.name.clone(),
            artifact,
            bytes,
        });
    }
    Ok(ImageBuildOutcome { commit, artifacts })
}

/// `docker load` alimentado pela ponte; ao fim, o relatório volta pela mesma
/// ponte e ela fecha.
fn import(request: ImageImportRequest) -> Result<TunnelStream, DockerError> {
    request.validate()?;
    let client = client()?;
    let (agent_side, ours) = tokio::io::duplex(256 * 1024);
    let (mut reader, mut writer) = tokio::io::split(ours);
    tokio::spawn(async move {
        let (tar, mut tar_feed) = tokio::io::duplex(256 * 1024);
        // O corpo do load lê do `tar`; a ponte escreve no `reader`.
        let copy = tokio::spawn(async move {
            let result = tokio::io::copy(&mut reader, &mut tar_feed).await;
            let _ = tar_feed.shutdown().await;
            result
        });
        let body = tokio_util::io::ReaderStream::new(tar).filter_map(|chunk| async { chunk.ok() });
        let loading = async {
            let mut stream =
                client.import_image_stream(ImportImageOptions { quiet: true }, body, None);
            let mut failure = None;
            while let Some(item) = stream.next().await {
                match item {
                    Ok(info) => {
                        if let Some(error) = info.error {
                            failure = Some(error);
                        }
                    }
                    Err(error) => failure = Some(error.to_string()),
                }
            }
            failure
        };
        let failure = match tokio::time::timeout(IMPORT_TIMEOUT, loading).await {
            Ok(failure) => failure,
            Err(_) => Some("O docker load passou do tempo limite".into()),
        };
        let _ = copy.await;
        let mut loaded = Vec::new();
        let mut error = failure;
        if error.is_none() {
            for reference in &request.references {
                if call(client.inspect_image(reference)).await.is_ok() {
                    loaded.push(reference.clone());
                } else {
                    error = Some(format!("A imagem {reference} não apareceu depois do load"));
                }
            }
        }
        let report = ImportReport { loaded, error };
        if let Ok(json) = serde_json::to_vec(&report) {
            let _ = writer.write_all(&json).await;
        }
        let _ = writer.shutdown().await;
    });
    Ok(Box::new(agent_side))
}

#[async_trait]
impl ImageHost for LocalEngine {
    async fn build_images(
        &self,
        request: &ImageBuildRequest,
        events: BuildEvents,
        cancel: CancellationToken,
    ) -> Result<ImageBuildOutcome, DockerError> {
        // Os CLIs são `kill_on_drop`: soltar a compilação encerra o `buildx`.
        tokio::select! {
            result = build(request, &events) => result,
            () = cancel.cancelled() => Err(DockerError::Validation("Compilação cancelada".into())),
        }
    }

    async fn builder_readiness(&self) -> Result<BuilderReadiness, DockerError> {
        let version = |output: String| output.lines().next().map(str::trim).map(String::from);
        let buildx_version =
            match cli::capture("docker", &["buildx", "version"], None, CHECK_TIMEOUT).await {
                Ok((true, output)) => version(output),
                _ => None,
            };
        let git_version = match cli::capture("git", &["--version"], None, CHECK_TIMEOUT).await {
            Ok((true, output)) => version(output),
            _ => None,
        };
        let builder_ready =
            buildx_version.is_some() && quiet(&["buildx", "inspect", BUILDER]).await;
        let root = BuildsRoot::from_env();
        let _ = tokio::fs::create_dir_all(root.path()).await;
        let path = root.path().display().to_string();
        let free_bytes = cli::capture("df", &["-Pk", &path], None, CHECK_TIMEOUT)
            .await
            .ok()
            .and_then(|(_, output)| parse_df(&output));
        let cpus = host_cpus();
        let limits = BuilderLimits::from_env(cpus);
        Ok(BuilderReadiness {
            buildx_version,
            builder_ready,
            git_version,
            free_bytes,
            cpus,
            cpu_limit: limits.cpu_label(),
            memory_limit: limits.memory,
        })
    }

    async fn open_artifact(&self, artifact: Uuid) -> Result<TunnelStream, DockerError> {
        let reader = BuildsRoot::from_env().open(artifact).await?;
        Ok(Box::new(tokio::io::join(reader, tokio::io::sink())))
    }

    async fn open_import(&self, request: &ImageImportRequest) -> Result<TunnelStream, DockerError> {
        import(request.clone())
    }
}

/// Lê o relatório de um import do fim da ponte.
///
/// # Errors
///
/// `Engine` quando a ponte fechou sem relatório.
pub async fn read_report<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<ImportReport, DockerError> {
    let mut raw = Vec::new();
    reader
        .take(64 * 1024)
        .read_to_end(&mut raw)
        .await
        .map_err(|_| DockerError::Engine)?;
    serde_json::from_slice(&raw).map_err(|_| {
        DockerError::Validation("O agente não confirmou o carregamento da imagem".into())
    })
}
