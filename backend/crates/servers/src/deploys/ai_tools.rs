//! As ferramentas de deploy da IA (ADR 021).
//!
//! Quatro de consulta — o estado de cada aplicação, o log de um deploy, as
//! compilações no servidor e o log de uma delas (ADR 023), para responder
//! "por que o último deploy do SGC falhou?" — e uma de ação, `deploy_action`,
//! que aprova, recusa, publica, restaura ou compila. A de ação é
//! **sempre** de risco alto: pede confirmação a cada chamada, qualquer que
//! seja o modo de automação, porque troca a versão de uma aplicação em
//! produção.

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use sea_orm::EntityTrait;
use serde_json::{json, Value};

use netmonitor_core::{
    ai::{
        args::ToolArgs,
        risk::RiskAssessment,
        tools::{audit, AiToolHandler, ToolGroup, ToolKind, ToolOutput, VIA_AI},
    },
    areas::Area,
    audit::{AuditAction, AuditEntryInput, ResourceType},
    models::users,
    shared::errors::AppResult,
};

use super::{
    applications, builds, catalog, events, orchestrator, runs, short_digest, short_sha, RunStatus,
};
use crate::dtos::deploys::{DeployInput, DeployWhen};

/// Linhas do fim do log que vão à IA.
const LOG_TAIL_LINES: usize = 80;

/// As últimas [`LOG_TAIL_LINES`] linhas de um console.
fn log_tail(log: Option<&str>) -> String {
    let lines: Vec<&str> = log.unwrap_or_default().lines().collect();
    lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n")
}

pub struct DeployStatus;

#[async_trait]
impl AiToolHandler for DeployStatus {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "get_deploys"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "Deploy contínuo: cada aplicação (repositório do GitHub), os alvos (servidor e projeto compose), a versão no ar, o deploy pendente e os deploys recentes com o estado e o erro. Use para perguntas sobre versões publicadas, aprovações pendentes e falhas de deploy."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "application": { "type": "string", "description": "Nome ou parte do nome da aplicação (opcional)" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let filter = args.text("application").map(|value| value.to_lowercase());
        let overview = catalog::overview(ctx).await?;
        let matches = |name: &str| {
            filter
                .as_deref()
                .is_none_or(|filter| name.to_lowercase().contains(filter))
        };
        let applications: Vec<Value> = overview
            .applications
            .iter()
            .filter(|app| matches(&app.name))
            .map(|app| {
                json!({
                    "applicationId": app.id,
                    "name": app.name,
                    "repository": app.repository,
                    "targets": app.targets.iter().map(|target| json!({
                        "targetId": target.id,
                        "name": target.name,
                        "agent": target.agent.as_ref().map(|agent| &agent.name),
                        "agentConnected": target.agent.as_ref().is_some_and(|agent| agent.connected),
                        "project": target.project,
                        "follows": target.reference,
                        "requiresApproval": target.requires_approval,
                        "live": target.live.as_ref().map(|run| json!({
                            "commit": run.short_sha,
                            "message": run.commit_message.as_deref().and_then(|message| message.lines().next()),
                            "at": run.finished_at,
                        })),
                        "pending": target.pending.as_ref().map(|run| json!({
                            "runId": run.id,
                            "status": run.status,
                            "commit": run.short_sha,
                        })),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let recent: Vec<Value> = overview
            .recent_runs
            .iter()
            .filter(|run| matches(&run.application_name))
            .take(15)
            .map(|run| {
                json!({
                    "runId": run.id,
                    "application": run.application_name,
                    "target": run.target_name,
                    "status": run.status,
                    "commit": run.short_sha,
                    "trigger": run.trigger,
                    "error": run.error,
                    "at": run.finished_at.as_ref().or(Some(&run.created_at)),
                })
            })
            .collect();
        Ok(ToolOutput::data(json!({
            "applications": applications,
            "recentRuns": recent,
            "counts": overview.counts,
        })))
    }
}

pub struct DeployLog;

#[async_trait]
impl AiToolHandler for DeployLog {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "get_deploy_log"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "O fim do console de um deploy (git, pull, healthchecks, rollback), com estado e erro. Informe 'runId' (de get_deploys) para explicar por que um deploy falhou."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "runId": { "type": "integer", "description": "Id do deploy (get_deploys)" }
            },
            "required": ["runId"]
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Some(id) = args.integer("runId") else {
            return Ok(ToolOutput::not_found("Informe 'runId' (veja get_deploys)"));
        };
        let Ok(run) = runs::find(&ctx.db, id).await else {
            return Ok(ToolOutput::not_found(format!(
                "Deploy #{id} não encontrado"
            )));
        };
        let view = events::view(&ctx.db, &run).await?;
        let tail = log_tail(run.log.as_deref());
        Ok(ToolOutput::data(json!({
            "runId": view.id,
            "application": view.application_name,
            "target": view.target_name,
            "status": view.status,
            "stage": view.stage,
            "commit": view.short_sha,
            "message": view.commit_message,
            "error": view.error,
            "logTail": tail,
        })))
    }
}

pub struct BuildStatus;

#[async_trait]
impl AiToolHandler for BuildStatus {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "get_builds"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "Compilações de imagem no servidor (deploy contínuo sem GitHub Actions): aplicação, commit, branch, quem pediu, estado, etapa, imagens geradas e o erro. Use para 'a compilação do SGC falhou?' ou 'o que está compilando?'."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "applicationId": { "type": "integer", "description": "Só desta aplicação (get_deploys); vazio, todas" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let rows = builds::recent(&ctx.db, args.integer("applicationId"), 15).await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            let view = builds::view(&ctx.db, row).await?;
            items.push(json!({
                "buildId": view.id,
                "application": view.application,
                "applicationId": view.application_id,
                "branch": builds::branch_of(row),
                "commit": view.short_sha,
                "message": view.commit_message.as_deref().and_then(|message| message.lines().next()),
                "trigger": view.trigger,
                "by": view.triggered_by,
                "agent": view.agent,
                "status": view.status,
                "stage": view.stage,
                "noCache": view.no_cache,
                "images": view.images.iter().map(|image| json!({
                    "name": image.name,
                    "digest": short_digest(&image.digest),
                })).collect::<Vec<_>>(),
                "error": view.error,
                "durationMs": view.duration_ms,
                "at": view.finished_at.as_ref().unwrap_or(&view.created_at),
            }));
        }
        Ok(ToolOutput::data(json!({ "builds": items })))
    }
}

pub struct BuildLog;

#[async_trait]
impl AiToolHandler for BuildLog {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "get_build_log"
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "O fim do console de uma compilação (clone, buildx, envio ao registry, análise de vulnerabilidades), com estado e erro. Informe 'buildId' (de get_builds) para explicar por que falhou."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "buildId": { "type": "integer", "description": "Id da compilação (get_builds)" }
            },
            "required": ["buildId"]
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Some(id) = args.integer("buildId") else {
            return Ok(ToolOutput::not_found("Informe 'buildId' (veja get_builds)"));
        };
        let Ok(build) = builds::find(&ctx.db, id).await else {
            return Ok(ToolOutput::not_found(format!(
                "Compilação #{id} não encontrada"
            )));
        };
        let view = builds::view(&ctx.db, &build).await?;
        Ok(ToolOutput::data(json!({
            "buildId": view.id,
            "application": view.application,
            "branch": builds::branch_of(&build),
            "commit": view.short_sha,
            "status": view.status,
            "stage": view.stage,
            "error": view.error,
            "logTail": log_tail(build.log.as_deref()),
        })))
    }
}

/// O que `deploy_action` faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Approve,
    Reject,
    Deploy,
    Restore,
    /// Compila no servidor (ADR 023) e, ao fim, publica nos alvos.
    Build,
}

impl Action {
    const NAMES: [&'static str; 5] = ["approve", "reject", "deploy", "restore", "build"];

    const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Deploy => "deploy",
            Self::Restore => "restore",
            Self::Build => "build",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "approve" => Some(Self::Approve),
            "reject" => Some(Self::Reject),
            "deploy" => Some(Self::Deploy),
            "restore" => Some(Self::Restore),
            "build" => Some(Self::Build),
            _ => None,
        }
    }
}

pub struct DeployAction;

fn when_of(args: &ToolArgs) -> DeployWhen {
    if args.text("when").as_deref() == Some("window") {
        DeployWhen::Window
    } else {
        DeployWhen::Now
    }
}

async fn actor_name(ctx: &AppContext, args: &ToolArgs) -> String {
    let name = match args.actor().user_id {
        Some(id) => users::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .ok()
            .flatten()
            .map(|user| user.name),
        None => None,
    };
    format!("{} ({VIA_AI})", name.unwrap_or_else(|| "IA".to_string()))
}

#[async_trait]
impl AiToolHandler for DeployAction {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "deploy_action"
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Action
    }

    fn admin_only(&self) -> bool {
        true
    }

    fn schedulable(&self) -> bool {
        false
    }

    async fn risk(&self, _ctx: &AppContext, _args: &ToolArgs) -> RiskAssessment {
        RiskAssessment::high("Troca a versão de uma aplicação: sempre pede confirmação")
    }

    fn description(&self) -> &'static str {
        "Age sobre o deploy contínuo, sempre com confirmação do usuário: 'approve'/'reject' um deploy aguardando aprovação (runId), 'deploy' publica uma versão num alvo (targetId e commitSha completo; sem commit republica a versão no ar), 'restore' volta um alvo à versão de um deploy que deu certo (runId), 'build' compila no servidor uma aplicação que não usa GitHub Actions (applicationId; branch e commitSha opcionais: vazios, o mais novo da branch que os alvos acompanham) e os alvos recebem o deploy ao fim. 'when': 'now' (padrão) ou 'window' (janela de manutenção do alvo)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": Action::NAMES },
                "runId": { "type": "integer", "description": "Deploy (approve, reject, restore)" },
                "targetId": { "type": "integer", "description": "Alvo (deploy)" },
                "applicationId": { "type": "integer", "description": "Aplicação (build)" },
                "branch": { "type": "string", "description": "Branch a compilar (build)" },
                "commitSha": { "type": "string", "description": "SHA completo do commit (deploy, build)" },
                "when": { "type": "string", "enum": ["now", "window"] },
                "reason": { "type": "string", "description": "Motivo da recusa (reject)" }
            },
            "required": ["action"]
        })
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let action = args.text("action").as_deref().and_then(Action::parse);
        let when = match when_of(args) {
            DeployWhen::Now => "agora",
            DeployWhen::Window => "na janela de manutenção",
        };
        Ok(match action {
            Some(Action::Build) => {
                let application = match args.integer("applicationId") {
                    Some(id) => applications::find(&ctx.db, id)
                        .await
                        .map_or_else(|_| format!("aplicação #{id}"), |app| app.name),
                    None => "aplicação não informada".to_string(),
                };
                let version = args.text("commitSha").map_or_else(
                    || "o commit mais novo".to_string(),
                    |sha| format!("o commit {}", short_sha(&sha)),
                );
                let branch = args
                    .text("branch")
                    .map_or_else(String::new, |branch| format!(" de {branch}"));
                format!(
                    "Compilar {version}{branch} de {application} no servidor e publicar nos alvos"
                )
            }
            Some(Action::Deploy) => {
                let target = match args.integer("targetId") {
                    Some(id) => applications::find_target(&ctx.db, id)
                        .await
                        .map_or_else(|_| format!("alvo #{id}"), |target| target.name),
                    None => "alvo não informado".to_string(),
                };
                let version = args.text("commitSha").map_or_else(
                    || "a versão no ar (de novo)".to_string(),
                    |sha| format!("a versão {}", short_sha(&sha)),
                );
                format!("Publicar {version} em {target}, {when}")
            }
            Some(action) => {
                let label = match args.integer("runId") {
                    Some(id) => match runs::find(&ctx.db, id).await {
                        Ok(run) => match events::view(&ctx.db, &run).await {
                            Ok(view) => format!(
                                "o deploy #{} ({} em {}, versão {})",
                                view.id, view.application_name, view.target_name, view.short_sha
                            ),
                            Err(_) => format!("o deploy #{id}"),
                        },
                        Err(_) => format!("o deploy #{id}"),
                    },
                    None => "deploy não informado".to_string(),
                };
                match action {
                    Action::Approve => format!("Aprovar {label}, {when}"),
                    Action::Reject => format!("Recusar {label}"),
                    _ => format!("Restaurar a versão de {label}, {when}"),
                }
            }
            None => "Ação de deploy inválida".to_string(),
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Some(action) = args.text("action").as_deref().and_then(Action::parse) else {
            return Ok(ToolOutput::not_found(format!(
                "Informe 'action': {}",
                Action::NAMES.join(", ")
            )));
        };
        let user = actor_name(ctx, args).await;
        if action == Action::Build {
            return build(ctx, args, &user).await;
        }
        let when = when_of(args);
        let result = match action {
            Action::Deploy => match args.integer("targetId") {
                Some(target) => {
                    orchestrator::request_manual(
                        ctx,
                        target,
                        &DeployInput {
                            commit_sha: args.text("commitSha"),
                            image_tag: None,
                            when,
                        },
                        &user,
                    )
                    .await
                }
                None => return Ok(ToolOutput::not_found("Informe 'targetId' (get_deploys)")),
            },
            other => {
                let Some(run_id) = args.integer("runId") else {
                    return Ok(ToolOutput::not_found("Informe 'runId' (get_deploys)"));
                };
                match other {
                    Action::Approve => orchestrator::approve(ctx, run_id, when, &user).await,
                    Action::Reject => {
                        orchestrator::reject(ctx, run_id, &user, args.text("reason").as_deref())
                            .await
                    }
                    _ => orchestrator::request_restore(ctx, run_id, when, &user).await,
                }
            }
        };
        let run = match result {
            Ok(run) => run,
            Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
        };
        let view = events::view(&ctx.db, &run).await?;
        audit(
            ctx,
            args,
            AuditEntryInput {
                action: AuditAction::Execute,
                resource_type: ResourceType::Deploy,
                resource_id: Some(view.id),
                resource_label: Some(format!(
                    "#{} {} / {}",
                    view.id, view.application_name, view.target_name
                )),
                description: Some(format!("Deploy: {} ({VIA_AI})", action.as_str())),
                changes: None,
            },
        )
        .await;
        Ok(ToolOutput::data(json!({
            "ok": true,
            "runId": view.id,
            "status": view.status,
            "scheduledFor": view.scheduled_for,
            "running": view.status == RunStatus::Running || view.status == RunStatus::Queued,
        })))
    }
}

/// `deploy_action` com `build`: o mesmo caminho do "Compilar agora" da tela.
async fn build(ctx: &AppContext, args: &ToolArgs, user: &str) -> AppResult<ToolOutput> {
    let Some(application_id) = args.integer("applicationId") else {
        return Ok(ToolOutput::not_found(
            "Informe 'applicationId' (get_deploys)",
        ));
    };
    let application = match applications::find(&ctx.db, application_id).await {
        Ok(application) => application,
        Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
    };
    let branch = args.text("branch");
    let commit = args.text("commitSha");
    let requested = builds::request_manual(
        ctx,
        &application,
        &builds::ManualBuild {
            reference: branch.as_deref(),
            commit: commit.as_deref(),
            no_cache: false,
        },
        user,
    )
    .await;
    let build = match requested {
        Ok(build) => build,
        Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
    };
    audit(
        ctx,
        args,
        AuditEntryInput {
            action: AuditAction::Execute,
            resource_type: ResourceType::Deploy,
            resource_id: Some(application.id),
            resource_label: Some(application.name.clone()),
            description: Some(format!(
                "Compilação #{} de {} ({}) ({VIA_AI})",
                build.id,
                short_sha(&build.commit_sha),
                builds::branch_of(&build)
            )),
            changes: None,
        },
    )
    .await;
    Ok(ToolOutput::data(json!({
        "ok": true,
        "buildId": build.id,
        "commit": short_sha(&build.commit_sha),
        "status": build.status,
    })))
}
