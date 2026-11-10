//! Uma compilação do começo ao fim (ADR 023).
//!
//! 1. escolhe o agente: o da aplicação ou o do primeiro alvo do ref;
//! 2. pede `ImageBuild` (clone, teste, imagens) com o console ao vivo;
//! 3. traz cada artefato pela ponte e guarda no registry, com a origem;
//! 4. analisa as vulnerabilidades (o resumo vai ao console);
//! 5. abre os deploys dos alvos que acompanham o ref — a mesma fila do
//!    aviso do GitHub Actions (aprovação, janela, canário).
//!
//! Um builder compila uma coisa por vez: as outras esperam na fila
//! (`queued`), e a que fica velha na espera sai substituída.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::json;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;

use agent_protocol::docker::{
    images::{BuildArg, BuildChunk, BuildImageSpec, ImageBuildRequest},
    stacks::GitSource,
};
use netmonitor_core::{
    agents::HostKey,
    models::{deploy_applications, deploy_builds, deploy_targets},
    shared::errors::{AppError, AppResult},
};

use super::{publish, publish_lines, secrets, BuildStage, BuildStatus};
use crate::{
    deploys::{
        access,
        applications::{self, repository_url},
        console::{human_bytes, Console, ConsoleSink},
        images::{
            image_store, repository_for, tag_for, ImageOrigin, OriginKind, ScanStatus, StoredImage,
        },
        orchestrator::Revision,
        refs::{self, GitRef},
        short_digest, short_sha, webhook,
    },
    docker::{hosts, DockerError},
    dtos::deploys::BuildArgInput,
};

/// Teto de uma compilação inteira.
pub const BUILD_TIMEOUT_SECS: u32 = 60 * 60;

fn slots() -> &'static Mutex<HashMap<i64, Arc<Semaphore>>> {
    static SLOTS: OnceLock<Mutex<HashMap<i64, Arc<Semaphore>>>> = OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn running() -> &'static Mutex<HashMap<i64, CancellationToken>> {
    static RUNNING: OnceLock<Mutex<HashMap<i64, CancellationToken>>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Uma vaga por builder.
fn slot(probe_id: i64) -> Arc<Semaphore> {
    let mut map = slots()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(probe_id)
        .or_insert_with(|| Arc::new(Semaphore::new(1)))
        .clone()
}

/// Interrompe a compilação em curso (o pedido ao agente é cancelado).
pub fn cancel(id: i64) {
    if let Some(token) = running()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&id)
    {
        token.cancel();
    }
}

/// O ref gravado (`refs/heads/main`, `refs/tags/v1`; nome solto é branch).
#[must_use]
pub fn parse_ref(raw: &str) -> Option<GitRef> {
    if raw.starts_with("refs/") {
        GitRef::parse(raw).ok()
    } else {
        GitRef::parse(&format!("refs/heads/{raw}")).ok()
    }
}

/// Dispara em segundo plano.
pub fn spawn(ctx: &AppContext, id: i64) {
    let ctx = ctx.clone();
    tokio::spawn(async move {
        execute(ctx, id).await;
    });
}

/// O console de uma compilação: o log e a etapa do `deploy_builds`.
struct BuildSink {
    id: i64,
}

#[async_trait::async_trait]
impl ConsoleSink for BuildSink {
    type Stage = (BuildStage, Option<String>);

    fn publish(&self, ctx: &AppContext, lines: Vec<String>) {
        publish_lines(ctx, self.id, lines);
    }

    async fn save(&self, ctx: &AppContext, text: &str) {
        if let Ok(Some(row)) = deploy_builds::Entity::find_by_id(self.id)
            .one(&ctx.db)
            .await
        {
            let mut active: deploy_builds::ActiveModel = row.into();
            active.log = Set(Some(text.to_string()));
            if let Err(error) = active.update(&ctx.db).await {
                tracing::warn!(build = self.id, %error, "log da compilação não foi gravado");
            }
        }
    }

    async fn stage(&self, ctx: &AppContext, (stage, image): (BuildStage, Option<String>)) {
        if let Ok(Some(row)) = deploy_builds::Entity::find_by_id(self.id)
            .one(&ctx.db)
            .await
        {
            let mut active: deploy_builds::ActiveModel = row.into();
            active.stage = Set(serde_json::to_value(stage)
                .ok()
                .and_then(|value| value.as_str().map(ToString::to_string)));
            active.current_image = Set(image);
            let _ = active.update(&ctx.db).await;
        }
        publish(ctx, self.id).await;
    }
}

type BuildConsole = Console<BuildSink>;

/// O agente que compila: o escolhido na aplicação ou o do primeiro alvo
/// ativo que acompanha o ref.
async fn builder_of(
    ctx: &AppContext,
    application: &deploy_applications::Model,
    reference: Option<&GitRef>,
) -> AppResult<i64> {
    if let Some(probe_id) = application.build_probe_id {
        return Ok(probe_id);
    }
    let targets = deploy_targets::Entity::find()
        .filter(deploy_targets::Column::ApplicationId.eq(application.id))
        .order_by_asc(deploy_targets::Column::Position)
        .order_by_asc(deploy_targets::Column::Id)
        .all(&ctx.db)
        .await?;
    targets
        .iter()
        .filter(|target| target.enabled)
        .find(|target| reference.is_none_or(|reference| refs::matches(&target.reference, reference)))
        .or_else(|| targets.first())
        .and_then(|target| target.probe_id)
        .ok_or_else(|| {
            AppError::business_rule(
                "Nenhum servidor para compilar: cadastre um alvo ou escolha onde compilar na aplicação",
            )
        })
}

fn request_of(
    application: &deploy_applications::Model,
    build: &deploy_builds::Model,
    git_token: Option<String>,
    secrets: Vec<agent_protocol::docker::images::BuildSecret>,
) -> ImageBuildRequest {
    let args: Vec<BuildArgInput> =
        serde_json::from_value(application.build_args.clone()).unwrap_or_default();
    ImageBuildRequest {
        source: GitSource {
            repository_url: repository_url(&application.repository),
            commit: build.commit_sha.clone(),
            token: git_token.map(agent_protocol::docker::stacks::Secret::new),
        },
        images: applications::images_of(application)
            .into_iter()
            .map(|image| BuildImageSpec {
                name: image.name,
                dockerfile: image.dockerfile,
                context: image.context,
                target: image.target,
            })
            .collect(),
        test_target: application.test_target.clone(),
        build_args: args
            .into_iter()
            .map(|arg| BuildArg {
                name: arg.name,
                value: arg.value,
            })
            .collect(),
        secrets,
        no_cache: build.no_cache,
        timeout_secs: BUILD_TIMEOUT_SECS,
    }
}

async fn mark(
    ctx: &AppContext,
    id: i64,
    change: impl FnOnce(&mut deploy_builds::ActiveModel),
) -> AppResult<()> {
    let row = super::find(&ctx.db, id).await?;
    let mut active: deploy_builds::ActiveModel = row.into();
    change(&mut active);
    active.update(&ctx.db).await?;
    Ok(())
}

/// A compilação inteira; o fim (sucesso, falha) é gravado aqui.
pub async fn execute(ctx: AppContext, id: i64) {
    let Ok(build) = super::find(&ctx.db, id).await else {
        return;
    };
    let Ok(application) = applications::find(&ctx.db, build.application_id).await else {
        return;
    };
    let reference = build.reference.as_deref().and_then(parse_ref);
    let probe_id = match builder_of(&ctx, &application, reference.as_ref()).await {
        Ok(probe_id) => probe_id,
        Err(error) => {
            finish(&ctx, id, BuildStatus::Failed, Some(error.to_string()), None).await;
            return;
        }
    };
    let _ = mark(&ctx, id, |active| active.probe_id = Set(Some(probe_id))).await;
    publish(&ctx, id).await;

    // Espera a vez do builder; quem ficou velho na fila sai substituído.
    let permit = slot(probe_id).acquire_owned().await;
    let Ok(current) = super::find(&ctx.db, id).await else {
        return;
    };
    if BuildStatus::parse(&current.status) != BuildStatus::Queued {
        return;
    }
    let token = CancellationToken::new();
    running()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(id, token.clone());
    let started = Instant::now();
    let _ = mark(&ctx, id, |active| {
        active.status = Set(BuildStatus::Running.as_str().to_string());
        active.started_at = Set(Some(Utc::now().into()));
    })
    .await;
    publish(&ctx, id).await;

    let mut console: BuildConsole = Console::new(&ctx, BuildSink { id }, None);
    // O cancelamento vai até o agente (o `buildx` é encerrado) e `run` só
    // volta depois disso: a vaga do builder não é liberada antes da hora.
    let result = run(&ctx, &application, &current, probe_id, &mut console, &token).await;
    running()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&id);
    drop(permit);
    let elapsed = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
    match result {
        Ok(images) => {
            console
                .line(format!("✔ Compilação concluída em {}s", elapsed / 1000))
                .await;
            console.flush().await;
            finish(
                &ctx,
                id,
                BuildStatus::Succeeded,
                None,
                Some((images.clone(), elapsed)),
            )
            .await;
            if let Err(error) = open_deploys(&ctx, &application, &current).await {
                tracing::warn!(build = id, %error, "deploys da compilação não foram abertos");
            }
        }
        Err(error) => {
            let cancelled = token.is_cancelled();
            if !cancelled {
                console.line(format!("✖ {error}")).await;
            }
            console.flush().await;
            if !cancelled {
                finish(
                    &ctx,
                    id,
                    BuildStatus::Failed,
                    Some(error.to_string()),
                    Some((Vec::new(), elapsed)),
                )
                .await;
                if let Ok(build) = super::find(&ctx.db, id).await {
                    if let Ok(view) = super::view(&ctx.db, &build).await {
                        crate::deploys::notify::send_build_failed(&ctx, &view).await;
                    }
                }
            }
        }
    }
}

async fn finish(
    ctx: &AppContext,
    id: i64,
    status: BuildStatus,
    error: Option<String>,
    result: Option<(Vec<StoredImage>, i64)>,
) {
    let saved = mark(ctx, id, |active| {
        active.status = Set(status.as_str().to_string());
        // Falhou: a etapa fica, para a tela mostrar onde parou.
        if status == BuildStatus::Succeeded {
            active.stage = Set(None);
        }
        active.current_image = Set(None);
        active.error = Set(error);
        active.finished_at = Set(Some(Utc::now().into()));
        if let Some((images, elapsed)) = result {
            if !images.is_empty() {
                active.images = Set(json!(images));
            }
            active.duration_ms = Set(Some(elapsed));
        }
    })
    .await;
    if let Err(error) = saved {
        tracing::warn!(build = id, %error, "o fim da compilação não foi gravado");
    }
    publish(ctx, id).await;
    super::super::events::publish_updated(ctx);
}

async fn run(
    ctx: &AppContext,
    application: &deploy_applications::Model,
    build: &deploy_builds::Model,
    probe_id: i64,
    console: &mut BuildConsole,
    cancel: &CancellationToken,
) -> AppResult<Vec<StoredImage>> {
    let store = image_store(ctx)?;
    let host = hosts::resolve(ctx, HostKey::agent(probe_id))?;
    console
        .line(format!(
            "▶ {} — commit {}{}",
            application.name,
            short_sha(&build.commit_sha),
            build
                .reference
                .as_deref()
                .map(|reference| format!(" ({reference})"))
                .unwrap_or_default()
        ))
        .await;
    let request = request_of(
        application,
        build,
        access::git_token(&ctx.db, application).await?,
        secrets::reveal_all(&ctx.db, application.id).await?,
    );
    request
        .validate()
        .map_err(|error| AppError::validation(error.to_string()))?;

    let (events, mut received) = mpsc::unbounded_channel::<BuildChunk>();
    let images = host.images.clone();
    let ask = request.clone();
    let stop = cancel.clone();
    let task = tokio::spawn(async move { images.build_images(&ask, events, stop).await });
    while let Some(chunk) = received.recv().await {
        match chunk {
            BuildChunk::Stage { stage, image } => {
                if let Some(image) = &image {
                    console.line(format!("── {image} ──")).await;
                }
                console.stage((stage, image)).await;
            }
            BuildChunk::Line { text } => console.line(text).await,
        }
    }
    let outcome = task
        .await
        .map_err(|_| DockerError::Engine)
        .and_then(|result| result)
        .map_err(|error| AppError::validation(error.to_string()))?;

    let tags: Vec<String> = std::iter::once(build.commit_sha.clone())
        .chain(
            build
                .reference
                .as_deref()
                .and_then(parse_ref)
                .and_then(|reference| tag_for(reference.name())),
        )
        .collect();
    let origin = ImageOrigin {
        source: OriginKind::Build,
        repository: application.repository.clone(),
        commit: outcome.commit.clone(),
        reference: build.reference.clone(),
        build_id: Some(build.id),
        agent: Some(host.key.to_string()),
        triggered_by: build.triggered_by.clone(),
        at: Utc::now().to_rfc3339(),
    };
    let mut stored = Vec::with_capacity(outcome.artifacts.len());
    for artifact in &outcome.artifacts {
        if cancel.is_cancelled() {
            return Err(AppError::conflict("Compilação cancelada"));
        }
        console
            .stage((BuildStage::Store, Some(artifact.image.clone())))
            .await;
        console
            .line(format!(
                "Guardando {} no registry ({})…",
                artifact.image,
                human_bytes(artifact.bytes)
            ))
            .await;
        let archive = host
            .images
            .open_artifact(artifact.artifact)
            .await
            .map_err(|error| AppError::service_unavailable(error.to_string()))?;
        let repository = repository_for(&application.slug, &artifact.image);
        let digest = store
            .ingest(ctx, &repository, application.id, &tags, archive, &origin)
            .await?;
        let bytes = store
            .find(ctx, &repository, &build.commit_sha)
            .await?
            .map_or(0, |(_, bytes)| bytes);
        console
            .line(format!(
                "✔ {repository}:{} ({})",
                short_sha(&build.commit_sha),
                short_digest(&digest)
            ))
            .await;
        stored.push(StoredImage {
            name: artifact.image.clone(),
            repository,
            tag: build.commit_sha.clone(),
            digest,
            bytes,
        });
    }

    console.stage((BuildStage::Scan, None)).await;
    for image in &stored {
        let verdict = store
            .verdict(ctx, &image.repository, &image.tag, application.id)
            .await?;
        let line = match verdict.status {
            ScanStatus::Done if verdict.critical + verdict.high == 0 => format!(
                "✔ {}: nenhuma vulnerabilidade crítica ou alta ({} média(s), {} baixa(s))",
                image.name, verdict.medium, verdict.low
            ),
            ScanStatus::Done => format!(
                "⚠ {}: {} crítica(s), {} alta(s) — {}",
                image.name,
                verdict.critical,
                verdict.high,
                verdict
                    .top
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ScanStatus::NoSbom => format!("⚠ {}: sem lista de pacotes para analisar", image.name),
            _ => format!(
                "⚠ {}: a análise não concluiu (a base de vulnerabilidades está indisponível)",
                image.name
            ),
        };
        console.line(line).await;
    }
    Ok(stored)
}

/// Os deploys dos alvos que acompanham o ref compilado.
async fn open_deploys(
    ctx: &AppContext,
    application: &deploy_applications::Model,
    build: &deploy_builds::Model,
) -> AppResult<()> {
    let Some(reference) = build.reference.as_deref().and_then(parse_ref) else {
        return Ok(());
    };
    let revision = Revision {
        commit_sha: build.commit_sha.clone(),
        commit_message: build.commit_message.clone(),
        reference: Some(reference.name().to_string()),
        image_tag: build.commit_sha.clone(),
        build_id: Some(build.id),
    };
    webhook::request_revision(
        ctx,
        application,
        &reference,
        &revision,
        build.triggered_by.clone(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_gravado_vira_branch_ou_tag() {
        assert_eq!(
            parse_ref("refs/heads/main"),
            Some(GitRef::Branch("main".into()))
        );
        assert_eq!(
            parse_ref("refs/tags/v1.2"),
            Some(GitRef::Tag("v1.2".into()))
        );
        assert_eq!(parse_ref("main"), Some(GitRef::Branch("main".into())));
        assert_eq!(parse_ref("refs/pull/3/head"), None);
    }
}
