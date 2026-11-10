//! A execução de um deploy já marcado como `running`: backup, agente e, se
//! preciso, o rollback.
//!
//! ## Rollback
//!
//! Quando a versão nova não fica saudável e o alvo tem `auto_rollback`, a
//! central pede ao agente **outro deploy**: o commit, a tag e o `.env` da
//! versão que estava no ar. As imagens dela ainda estão no disco (a limpeza
//! só roda depois de um sucesso e guarda as últimas N), então a volta não
//! depende do registry.
//!
//! O que o rollback **não** desfaz é o banco: se a versão nova rodou
//! migrations, o esquema ficou à frente. O deploy termina `rolled_back` com
//! esse aviso escrito, e a tela oferece a cópia feita antes (quando o alvo
//! tem backup configurado) para restaurar.

use loco_rs::app::AppContext;
use tokio::sync::mpsc;

use agent_protocol::docker::stacks::{
    GitSource, RegistryAuth, Secret, StackChunk, StackDeployOutcome, StackDeployRequest,
};
use netmonitor_core::{
    agents::{hub::AgentHub, HostKey},
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::errors::{AppError, AppResult},
};

use super::{
    access,
    applications::{self, compose_files_of, repository_url},
    console::{human_bytes, Console, ConsoleSink},
    env_versions, events,
    notify::{self, Moment},
    runs::{self, Change},
    short_sha, DeployStage, RunStatus,
};
use crate::{
    databases::backups,
    deploys::images::{image_store, repository_for, OriginKind, VulnerabilityPolicy},
    docker::{
        builds::{oci_architecture, read_report, ImageImportRequest},
        hosts, DockerError,
    },
    dtos::deploys::{BuildImage, Builder},
};

/// Linhas do console vão à tela em lotes, no máximo a cada intervalo.
/// O console de um deploy: o log e a etapa do `deploy_runs`.
struct RunSink {
    run_id: i64,
}

#[async_trait::async_trait]
impl ConsoleSink for RunSink {
    type Stage = DeployStage;

    fn publish(&self, ctx: &AppContext, lines: Vec<String>) {
        events::publish_lines(ctx, self.run_id, lines);
    }

    async fn save(&self, ctx: &AppContext, text: &str) {
        if let Err(error) = runs::save_log(&ctx.db, self.run_id, text).await {
            tracing::warn!(run_id = self.run_id, %error, "log do deploy não foi gravado");
        }
    }

    async fn stage(&self, ctx: &AppContext, stage: DeployStage) {
        if let Err(error) = runs::set_stage(&ctx.db, self.run_id, stage).await {
            tracing::warn!(run_id = self.run_id, %error, "etapa do deploy não foi gravada");
        }
        events::publish_run(ctx, self.run_id).await;
    }
}

type RunConsole = Console<RunSink>;

fn console_for(ctx: &AppContext, run: &deploy_runs::Model) -> RunConsole {
    Console::new(ctx, RunSink { run_id: run.id }, run.log.as_deref())
}

struct Plan {
    application: deploy_applications::Model,
    target: deploy_targets::Model,
    registry: Option<RegistryAuth>,
    git_token: Option<String>,
    /// As imagens vêm do registry da central, entregues pelo canal (ADR 023).
    from_central: bool,
}

/// Monta o pedido de uma revisão (commit, tag e versão do `.env`).
async fn request_for(
    ctx: &AppContext,
    plan: &Plan,
    run: &deploy_runs::Model,
) -> AppResult<StackDeployRequest> {
    let env_file = match run.env_version_id {
        Some(id) => Some(Secret::new(env_versions::reveal(
            &env_versions::find(&ctx.db, id).await?,
        )?)),
        None => None,
    };
    Ok(StackDeployRequest {
        project: plan.target.project.clone(),
        source: GitSource {
            repository_url: repository_url(&plan.application.repository),
            commit: run.commit_sha.clone(),
            token: plan.git_token.clone().map(Secret::new),
        },
        compose_files: compose_files_of(&plan.application),
        env_file,
        image_tag_variable: plan.application.image_tag_variable.clone(),
        image_tag: run.image_tag.clone(),
        registry: if plan.from_central {
            None
        } else {
            plan.registry.clone()
        },
        wait_timeout_secs: u32::try_from(plan.target.wait_timeout_secs).unwrap_or(180),
        keep_images: u32::try_from(plan.target.keep_images).unwrap_or(3),
        pull: !plan.from_central,
    })
}

async fn plan(ctx: &AppContext, run: &deploy_runs::Model) -> AppResult<Plan> {
    let target = applications::find_target(&ctx.db, run.target_id).await?;
    let application = applications::find(&ctx.db, run.application_id).await?;
    Ok(Plan {
        registry: access::registry_auth(&ctx.db, &application).await?,
        git_token: access::git_token(&ctx.db, &application).await?,
        from_central: Builder::parse(&application.builder) == Builder::Netmonitor,
        application,
        target,
    })
}

/// Leva o pedido ao agente e repassa etapas e linhas ao console.
async fn deliver(
    ctx: &AppContext,
    plan: &Plan,
    request: StackDeployRequest,
    console: &mut RunConsole,
) -> Result<StackDeployOutcome, String> {
    let probe_id = plan
        .target
        .probe_id
        .ok_or_else(|| "O agente deste alvo foi removido".to_string())?;
    let host = hosts::resolve(ctx, HostKey::agent(probe_id)).map_err(|error| error.to_string())?;
    if plan.from_central {
        console.stage(DeployStage::Image).await;
        ensure_images(ctx, plan, &host, probe_id, &request.image_tag, console)
            .await
            .map_err(|error| error.to_string())?;
    }
    let (events, mut received) = mpsc::unbounded_channel::<StackChunk>();
    let stacks = host.stacks.clone();
    let task = tokio::spawn(async move { stacks.deploy_stack(&request, events).await });
    while let Some(chunk) = received.recv().await {
        match chunk {
            StackChunk::Stage { stage } => console.stage(stage.into()).await,
            StackChunk::Line { text } => console.line(text).await,
        }
    }
    task.await
        .map_err(|_| DockerError::Engine)
        .and_then(|result| result)
        .map_err(|error| error.to_string())
}

/// O nome que a imagem tem no host: o `image:` do compose com a tag.
fn host_reference(image: &BuildImage, tag: &str) -> Option<String> {
    image
        .reference
        .as_ref()
        .map(|reference| format!("{reference}:{tag}"))
}

/// Entrega ao host, pelo canal, as imagens da versão que ele ainda não tem
/// (ADR 022 RF-10). A que já está lá (rollback, segunda tentativa) não
/// trafega de novo.
async fn ensure_images(
    ctx: &AppContext,
    plan: &Plan,
    host: &hosts::DockerHost,
    probe_id: i64,
    tag: &str,
    console: &mut RunConsole,
) -> AppResult<()> {
    let store = image_store(ctx)?;
    let architecture = AgentHub::from_context(ctx)?
        .get(probe_id)
        .map(|session| oci_architecture(&session.hello.arch))
        .unwrap_or_else(|| "amd64".to_string());
    for image in applications::images_of(&plan.application) {
        let Some(reference) = host_reference(&image, tag) else {
            return Err(AppError::validation(format!(
                "A imagem {} não tem o nome do image: do compose",
                image.name
            )));
        };
        if host.engine.inspect_image_raw(&reference).await.is_ok() {
            console
                .line(format!("✔ {reference} já está no servidor"))
                .await;
            continue;
        }
        let repository = repository_for(&plan.application.slug, &image.name);
        let Some((_, bytes)) = store.find(ctx, &repository, tag).await? else {
            return Err(AppError::not_found(format!(
                "A imagem {repository}:{} não está no registry: compile esta versão antes",
                short_sha(tag)
            )));
        };
        console
            .line(format!(
                "Enviando {} ao servidor ({})…",
                image.name,
                human_bytes(u64::try_from(bytes).unwrap_or(0))
            ))
            .await;
        let mut stream = host
            .images
            .open_import(&ImageImportRequest {
                references: vec![reference.clone()],
            })
            .await
            .map_err(|error| AppError::service_unavailable(error.to_string()))?;
        store
            .deliver(
                ctx,
                &repository,
                tag,
                &architecture,
                &reference,
                &mut stream,
            )
            .await?;
        let report = read_report(&mut stream)
            .await
            .map_err(|error| AppError::service_unavailable(error.to_string()))?;
        if let Some(error) = report.error {
            return Err(AppError::validation(format!(
                "O servidor não carregou {reference}: {error}"
            )));
        }
        console.line(format!("✔ {reference} carregada")).await;
    }
    Ok(())
}

/// A política do alvo para a imagem desta versão: vulnerabilidades e origem
/// (ADR 022 RF-27/28). Barra antes de qualquer efeito no servidor.
async fn verify_images(
    ctx: &AppContext,
    plan: &Plan,
    run: &deploy_runs::Model,
    console: &mut RunConsole,
) -> AppResult<()> {
    let policy = VulnerabilityPolicy::parse(&plan.target.vulnerability_policy);
    let store = image_store(ctx)?;
    for image in applications::images_of(&plan.application) {
        let repository = repository_for(&plan.application.slug, &image.name);
        if plan.target.require_provenance {
            let origin = store.provenance(ctx, &repository, &run.image_tag).await?;
            let known = origin.as_ref().is_some_and(|origin| {
                OriginKind::trusted_for(origin, &plan.application.repository)
            });
            if !known {
                return Err(AppError::validation(format!(
                    "A imagem {} não tem origem conhecida (compilada pela central a partir de {}): o alvo exige proveniência",
                    image.name, plan.application.repository
                )));
            }
        }
        let verdict = store
            .verdict(ctx, &repository, &run.image_tag, plan.application.id)
            .await?;
        if let Some(reason) = policy.blocks(&verdict) {
            return Err(AppError::validation(format!("{}: {reason}", image.name)));
        }
        if verdict.critical + verdict.high > 0 {
            console
                .line(format!(
                    "⚠ {}: {} crítica(s) e {} alta(s) — a política do alvo só avisa",
                    image.name, verdict.critical, verdict.high
                ))
                .await;
        }
    }
    Ok(())
}

/// Como o deploy terminou.
struct Ending {
    status: RunStatus,
    error: Option<String>,
    moment: Moment,
}

/// Executa o deploy e grava o fim. Nunca entra em pânico nem devolve erro:
/// tudo vira estado e log do próprio deploy.
pub async fn execute(ctx: AppContext, run_id: i64) {
    let ending = match runs::find(&ctx.db, run_id).await {
        Ok(run) => {
            let mut console = console_for(&ctx, &run);
            let ending = match run_deploy(&ctx, &run, &mut console).await {
                Ok(ending) => ending,
                Err(error) => {
                    console.line(format!("✖ {error}")).await;
                    Ending {
                        status: RunStatus::Failed,
                        error: Some(error.to_string()),
                        moment: Moment::Failed,
                    }
                }
            };
            console.flush().await;
            ending
        }
        Err(error) => {
            tracing::warn!(run_id, %error, "deploy sumiu antes de executar");
            return;
        }
    };
    let changed = runs::transition(
        &ctx.db,
        run_id,
        &[RunStatus::Running],
        ending.status,
        Change {
            stage: Some(None),
            error: Some(ending.error),
            finished: true,
            ..Change::default()
        },
    )
    .await;
    if let Err(error) = changed {
        tracing::warn!(run_id, %error, "o fim do deploy não foi gravado");
    }
    events::publish_run(&ctx, run_id).await;
    events::publish_updated(&ctx);
    if let Ok(run) = runs::find(&ctx.db, run_id).await {
        if let Ok(view) = events::view(&ctx.db, &run).await {
            notify::send(&ctx, ending.moment, &view).await;
        }
    }
    super::orchestrator::kick();
}

async fn run_deploy(
    ctx: &AppContext,
    run: &deploy_runs::Model,
    console: &mut RunConsole,
) -> AppResult<Ending> {
    let plan = plan(ctx, run).await?;
    let previous = runs::live(&ctx.db, run.target_id).await?;
    console
        .line(format!(
            "▶ {} em {} — versão #{} (imagem :{})",
            plan.application.name,
            plan.target.name,
            short_sha(&run.commit_sha),
            run.image_tag
        ))
        .await;

    let mut backed_up = false;
    if let Some(connection_id) = plan.target.backup_connection_id {
        console.stage(DeployStage::Backup).await;
        console.line("Backup do banco antes de atualizar…").await;
        match backups::backup_before_deploy(ctx, connection_id).await {
            Ok(ids) => {
                if let Some(first) = ids.first() {
                    runs::set_backup(&ctx.db, run.id, *first).await?;
                }
                console
                    .line(format!("✔ Backup concluído ({} cópia(s))", ids.len()))
                    .await;
                backed_up = true;
            }
            Err(error) => {
                return Err(AppError::validation(format!(
                    "O backup antes do deploy falhou e nada foi alterado: {error}"
                )));
            }
        }
    }

    if plan.from_central {
        console.stage(DeployStage::Image).await;
        verify_images(ctx, &plan, run, console).await?;
    }
    let request = request_for(ctx, &plan, run).await?;
    let failure = match deliver(ctx, &plan, request, console).await {
        Ok(outcome) => {
            console
                .line(format!(
                    "✔ No ar: commit {} — {} serviço(s) de pé",
                    short_sha(&outcome.commit),
                    outcome.services.len()
                ))
                .await;
            return Ok(Ending {
                status: RunStatus::Succeeded,
                error: None,
                moment: Moment::Succeeded,
            });
        }
        Err(error) => error,
    };
    console.line(format!("✖ {failure}")).await;

    let Some(previous) = previous.filter(|previous| {
        plan.target.auto_rollback
            && (previous.commit_sha != run.commit_sha || previous.image_tag != run.image_tag)
    }) else {
        return Ok(Ending {
            status: RunStatus::Failed,
            error: Some(failure),
            moment: Moment::Failed,
        });
    };

    console.stage(DeployStage::Rollback).await;
    console.pinned = true;
    console
        .line(format!(
            "── Voltando para a versão anterior #{} ──",
            short_sha(&previous.commit_sha)
        ))
        .await;
    let rollback = request_for(ctx, &plan, &previous).await?;
    let database_note = if backed_up {
        " Se a versão nova mexeu no banco (migrations), ele pode estar à frente da restaurada: \
         a cópia feita antes do deploy está disponível para restaurar."
    } else {
        " Se a versão nova mexeu no banco (migrations), ele pode estar à frente da restaurada."
    };
    Ok(match deliver(ctx, &plan, rollback, console).await {
        Ok(_) => {
            console.line("✔ Versão anterior de volta ao ar").await;
            Ending {
                status: RunStatus::RolledBack,
                error: Some(format!(
                    "{failure}. A versão #{} voltou ao ar.{database_note}",
                    short_sha(&previous.commit_sha)
                )),
                moment: Moment::RolledBack,
            }
        }
        Err(rollback_error) => {
            console.line(format!("✖ {rollback_error}")).await;
            Ending {
                status: RunStatus::Failed,
                error: Some(format!(
                    "{failure}. O rollback para #{} também falhou: {rollback_error}",
                    short_sha(&previous.commit_sha)
                )),
                moment: Moment::Failed,
            }
        }
    })
}
