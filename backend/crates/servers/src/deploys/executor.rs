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

use std::time::{Duration, Instant};

use loco_rs::app::AppContext;
use tokio::sync::mpsc;

use agent_protocol::docker::stacks::{
    GitSource, RegistryAuth, Secret, StackChunk, StackDeployOutcome, StackDeployRequest,
};
use netmonitor_core::{
    agents::HostKey,
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::errors::{AppError, AppResult},
};

use super::{
    applications::{self, compose_files_of, repository_url},
    credentials, env_versions, events,
    notify::{self, Moment},
    runs::{self, Change, LogBuffer},
    short_sha, DeployStage, RunStatus,
};
use crate::{
    databases::backups,
    docker::{hosts, DockerError},
};

/// Linhas do console vão à tela em lotes, no máximo a cada intervalo.
const PUBLISH_EVERY: Duration = Duration::from_millis(300);
/// O log vai ao banco no máximo a cada intervalo (e no fim).
const SAVE_EVERY: Duration = Duration::from_secs(2);

/// O console de um deploy: guarda, publica e grava.
struct Console {
    ctx: AppContext,
    run_id: i64,
    buffer: LogBuffer,
    unsent: Vec<String>,
    last_publish: Instant,
    last_save: Instant,
    /// No rollback, as etapas do agente não trocam a etapa "Rollback".
    pinned: bool,
}

impl Console {
    fn new(ctx: &AppContext, run: &deploy_runs::Model) -> Self {
        Self {
            ctx: ctx.clone(),
            run_id: run.id,
            buffer: LogBuffer::from_saved(run.log.as_deref()),
            unsent: Vec::new(),
            last_publish: Instant::now(),
            last_save: Instant::now(),
            pinned: false,
        }
    }

    async fn line(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.buffer.push(&text);
        self.unsent.push(text);
        if self.last_publish.elapsed() >= PUBLISH_EVERY {
            self.publish();
        }
        if self.last_save.elapsed() >= SAVE_EVERY {
            self.save().await;
        }
    }

    fn publish(&mut self) {
        events::publish_lines(&self.ctx, self.run_id, std::mem::take(&mut self.unsent));
        self.last_publish = Instant::now();
    }

    async fn save(&mut self) {
        if let Err(error) = runs::save_log(&self.ctx.db, self.run_id, &self.buffer.text()).await {
            tracing::warn!(run_id = self.run_id, %error, "log do deploy não foi gravado");
        }
        self.last_save = Instant::now();
    }

    async fn stage(&mut self, stage: DeployStage) {
        if self.pinned {
            return;
        }
        self.publish();
        if let Err(error) = runs::set_stage(&self.ctx.db, self.run_id, stage).await {
            tracing::warn!(run_id = self.run_id, %error, "etapa do deploy não foi gravada");
        }
        events::publish_run(&self.ctx, self.run_id).await;
    }

    async fn flush(&mut self) {
        self.publish();
        self.save().await;
    }
}

/// Tudo o que o pedido ao agente precisa, já carregado.
struct Plan {
    application: deploy_applications::Model,
    target: deploy_targets::Model,
    registry: Option<RegistryAuth>,
    git_token: Option<String>,
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
        registry: plan.registry.clone(),
        wait_timeout_secs: u32::try_from(plan.target.wait_timeout_secs).unwrap_or(180),
        keep_images: u32::try_from(plan.target.keep_images).unwrap_or(3),
    })
}

async fn plan(ctx: &AppContext, run: &deploy_runs::Model) -> AppResult<Plan> {
    let target = applications::find_target(&ctx.db, run.target_id).await?;
    let application = applications::find(&ctx.db, run.application_id).await?;
    let credential = match application.credential_id {
        Some(id) => Some(credentials::open(&ctx.db, id).await?),
        None => None,
    };
    Ok(Plan {
        registry: credential.as_ref().map(|credential| RegistryAuth {
            registry: credential.row.registry.clone(),
            username: credential.row.username.clone(),
            secret: Secret::new(credential.secret.clone()),
        }),
        git_token: credential.map(|credential| credential.secret),
        application,
        target,
    })
}

/// Leva o pedido ao agente e repassa etapas e linhas ao console.
async fn deliver(
    ctx: &AppContext,
    plan: &Plan,
    request: StackDeployRequest,
    console: &mut Console,
) -> Result<StackDeployOutcome, String> {
    let probe_id = plan
        .target
        .probe_id
        .ok_or_else(|| "O agente deste alvo foi removido".to_string())?;
    let host = hosts::resolve(ctx, HostKey::agent(probe_id)).map_err(|error| error.to_string())?;
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
            let mut console = Console::new(&ctx, &run);
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
    console: &mut Console,
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
