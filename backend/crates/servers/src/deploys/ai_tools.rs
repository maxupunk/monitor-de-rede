//! As ferramentas de deploy da IA e do servidor MCP (ADR 021 / ADR 024).
//!
//! Seis de consulta — o passo a passo até o primeiro deploy (o guia da tela),
//! o estado de cada aplicação, a prontidão de um alvo/aplicação, o log de um
//! deploy, as compilações no servidor e o log de uma delas (ADR 023) — e uma
//! de ação, `deploy_action`, que aprova, recusa, publica, restaura, compila ou
//! cancela. A de ação exige papel de administrador na área Servidores e é
//! **sempre** de risco alto no chat; via MCP, respeita `target.requires_approval`,
//! `allowed_application_ids` e `can_approve_deploys`.

use async_trait::async_trait;
use futures::future::try_join_all;
use loco_rs::prelude::AppContext;
use sea_orm::EntityTrait;
use serde_json::{json, Value};

use netmonitor_core::{
    ai::{
        access::require_area_admin,
        args::{McpToolContext, ToolArgs},
        risk::RiskAssessment,
        tools::{audit, via_label, AiToolHandler, McpExposure, ToolGroup, ToolKind, ToolOutput},
    },
    areas::Area,
    audit::{AuditAction, AuditEntryInput, ResourceType},
    models::users,
    shared::errors::AppResult,
};

use super::{
    applications, builds, catalog, events, guide, orchestrator, readiness, runs, short_digest,
    short_sha, RunStatus,
};
use crate::dtos::deploys::{DeployInput, DeployWhen};

/// Linhas do fim do log que vão à IA.
const LOG_TAIL_LINES: usize = 80;

/// As últimas [`LOG_TAIL_LINES`] linhas de um console.
fn log_tail(log: Option<&str>) -> String {
    let lines: Vec<&str> = log.unwrap_or_default().lines().collect();
    lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n")
}

fn mcp_allows_app(args: &ToolArgs, app_id: i64) -> bool {
    args.mcp().is_none_or(|mcp| mcp.allows_application(app_id))
}

fn require_mcp_app(args: &ToolArgs, app_id: i64) -> AppResult<()> {
    match args.mcp() {
        Some(mcp) => mcp.require_application(app_id),
        None => Ok(()),
    }
}

/// O passo a passo do deploy contínuo e onde o usuário está — o mesmo guia
/// do topo da tela de Deploys (`deploys::guide`), para a IA conduzir o
/// cadastro com o estado real em vez de adivinhar.
pub struct DeployGuide;

#[async_trait]
impl AiToolHandler for DeployGuide {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "get_deploy_guide"
    }

    fn mcp_exposure(&self) -> McpExposure {
        McpExposure::DeploysRead
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "O passo a passo para colocar uma aplicação no ar pelo deploy contínuo (GitHub → agente → aplicação → alvo → .env → primeira versão) e em que etapa o usuário está agora: o que já está pronto, o que falta e como fazer cada etapa. Use quando perguntarem como configurar o deploy, por onde começar, o que falta ou o que significa uma etapa — e conduza uma etapa por vez, com o nome exato dos botões da tela."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let mut overview = catalog::overview(ctx).await?;
        let guide = match args
            .mcp()
            .and_then(|mcp| mcp.allowed_application_ids.as_deref())
        {
            Some(allowed) => {
                overview
                    .applications
                    .retain(|app| mcp_allows_app(args, app.id));
                overview
                    .recent_runs
                    .retain(|run| mcp_allows_app(args, run.application_id));
                let agents = catalog::agents(ctx).await?;
                let latest_build = builds::recent_for_applications(&ctx.db, allowed, 1)
                    .await?
                    .into_iter()
                    .next()
                    .map(|build| guide::BuildFact {
                        status: builds::BuildStatus::parse(&build.status),
                        short_sha: short_sha(&build.commit_sha).to_string(),
                    });
                guide::steps(&guide::facts_of(&overview, &agents, latest_build))
            }
            None => overview.guide,
        };
        let current = guide
            .steps
            .iter()
            .find(|step| step.status == crate::views::deploys::DeployGuideStatus::Current)
            .map(|step| step.title.clone());
        Ok(ToolOutput::data(json!({
            "done": guide.done,
            "currentStep": current,
            "steps": guide.steps.iter().map(|step| json!({
                "step": step.key,
                "title": step.title,
                "status": step.status,
                "now": step.detail,
                "how": step.how,
            })).collect::<Vec<_>>(),
            "screen": "Deploys: o guia fica no topo; botões Conexões, Automatizar deploy, e em cada aplicação Compilar agora, Adicionar alvo e .env do alvo. Os agentes ficam em Servidores remotos.",
        })))
    }
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

    fn mcp_exposure(&self) -> McpExposure {
        McpExposure::DeploysRead
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
                "application": { "type": "string", "description": "Nome ou parte do nome da aplicação (opcional)" },
                "applicationId": { "type": "integer", "description": "Id da aplicação (opcional)" },
                "targetId": { "type": "integer", "description": "Id do alvo (opcional)" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let filter = args.text("application").map(|value| value.to_lowercase());
        let app_id_filter = args.integer("applicationId");
        let target_id_filter = args.integer("targetId");
        if let Some(app_id) = app_id_filter {
            require_mcp_app(args, app_id)?;
        }
        if let Some(target_id) = target_id_filter {
            if let Ok(target) = applications::find_target(&ctx.db, target_id).await {
                require_mcp_app(args, target.application_id)?;
            }
        }
        let overview = catalog::overview(ctx).await?;
        let restricted = args
            .mcp()
            .and_then(|mcp| mcp.allowed_application_ids.as_ref())
            .is_some();
        let name_matches = |name: &str| {
            filter
                .as_deref()
                .is_none_or(|filter| name.to_lowercase().contains(filter))
        };
        let visible_apps: Vec<_> = overview
            .applications
            .iter()
            .filter(|app| mcp_allows_app(args, app.id))
            .collect();
        let applications: Vec<Value> = visible_apps
            .iter()
            .filter(|app| app_id_filter.is_none_or(|id| app.id == id))
            .filter(|app| {
                target_id_filter
                    .is_none_or(|tid| app.targets.iter().any(|target| target.id == tid))
            })
            .filter(|app| name_matches(&app.name))
            .map(|app| {
                json!({
                    "applicationId": app.id,
                    "name": app.name,
                    "repository": app.repository,
                    "targets": app.targets.iter()
                        .filter(|target| target_id_filter.is_none_or(|tid| target.id == tid))
                        .map(|target| json!({
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
            .filter(|run| mcp_allows_app(args, run.application_id))
            .filter(|run| app_id_filter.is_none_or(|id| run.application_id == id))
            .filter(|run| target_id_filter.is_none_or(|tid| run.target_id == tid))
            .filter(|run| name_matches(&run.application_name))
            .take(15)
            .map(|run| {
                json!({
                    "runId": run.id,
                    "applicationId": run.application_id,
                    "application": run.application_name,
                    "targetId": run.target_id,
                    "target": run.target_name,
                    "status": run.status,
                    "commit": run.short_sha,
                    "trigger": run.trigger,
                    "error": run.error,
                    "at": run.finished_at.as_ref().or(Some(&run.created_at)),
                })
            })
            .collect();
        let counts = if restricted {
            let targets: usize = visible_apps.iter().map(|app| app.targets.len()).sum();
            let awaiting_approval = visible_apps
                .iter()
                .flat_map(|app| &app.targets)
                .filter(|target| {
                    target
                        .pending
                        .as_ref()
                        .is_some_and(|run| run.status == RunStatus::AwaitingApproval)
                })
                .count();
            let running = visible_apps
                .iter()
                .flat_map(|app| &app.targets)
                .filter(|target| {
                    target
                        .pending
                        .as_ref()
                        .is_some_and(|run| run.status == RunStatus::Running)
                })
                .count();
            json!({
                "applications": visible_apps.len(),
                "targets": targets,
                "awaitingApproval": awaiting_approval,
                "running": running,
            })
        } else {
            json!(overview.counts)
        };
        Ok(ToolOutput::data(json!({
            "applications": applications,
            "recentRuns": recent,
            "counts": counts,
        })))
    }
}

/// Checagem de prontidão de um alvo (ou de todos os alvos de uma aplicação):
/// agente conectado, permissões `deploy`/`build`, diretório de stacks, git,
/// compose, espaço em disco, healthchecks, credencial e presença do `.env`
/// (nunca expõe segredos nem o conteúdo do `.env`).
pub struct DeployReadiness;

#[async_trait]
impl AiToolHandler for DeployReadiness {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "get_deploy_readiness"
    }

    fn mcp_exposure(&self) -> McpExposure {
        McpExposure::DeploysRead
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "Verifica a prontidão de um alvo de deploy (targetId) ou de todos os alvos de uma aplicação (applicationId): conexão e permissões do agente/builder, diretório de stacks, git, Docker Compose, espaço em disco, healthchecks e presença de acesso ao GitHub e .env (sem expor segredos). Use antes de disparar um build ou deploy."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "targetId": { "type": "integer", "description": "Id do alvo (get_deploys)" },
                "applicationId": { "type": "integer", "description": "Id da aplicação para verificar todos os alvos dela (get_deploys)" }
            }
        })
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        if let Some(target_id) = args.integer("targetId") {
            let Ok(target) = applications::find_target(&ctx.db, target_id).await else {
                return Ok(ToolOutput::not_found(format!(
                    "Alvo de deploy #{target_id} não encontrado"
                )));
            };
            require_mcp_app(args, target.application_id)?;
            let application = applications::find(&ctx.db, target.application_id).await?;
            return Ok(ToolOutput::data(
                inspect_target_readiness(ctx, &application, &target).await?,
            ));
        }

        let Some(application_id) = args.integer("applicationId") else {
            return Ok(ToolOutput::not_found(
                "Informe 'targetId' ou 'applicationId' (veja get_deploys)",
            ));
        };
        require_mcp_app(args, application_id)?;
        let Ok(application) = applications::find(&ctx.db, application_id).await else {
            return Ok(ToolOutput::not_found(format!(
                "Aplicação #{application_id} não encontrada"
            )));
        };
        let targets = applications::targets_of(&ctx.db, application.id).await?;
        let results = try_join_all(
            targets
                .iter()
                .map(|target| inspect_target_readiness(ctx, &application, target)),
        )
        .await?;
        let all_ready = !results.is_empty()
            && results
                .iter()
                .all(|report| report["ready"].as_bool().unwrap_or(false));
        Ok(ToolOutput::data(json!({
            "applicationId": application.id,
            "application": application.name,
            "ready": all_ready,
            "targets": results,
        })))
    }
}

async fn inspect_target_readiness(
    ctx: &AppContext,
    application: &netmonitor_core::models::deploy_applications::Model,
    target: &netmonitor_core::models::deploy_targets::Model,
) -> AppResult<Value> {
    let Some(probe_id) = target.probe_id else {
        return Ok(json!({
            "targetId": target.id,
            "target": target.name,
            "applicationId": application.id,
            "application": application.name,
            "project": target.project,
            "ready": false,
            "error": "O agente deste alvo foi removido: escolha outro",
            "checks": [],
        }));
    };
    let view = readiness::inspect(
        ctx,
        probe_id,
        &target.project,
        Some(application),
        Some(target.id),
    )
    .await?;
    Ok(json!({
        "targetId": target.id,
        "target": target.name,
        "applicationId": application.id,
        "application": application.name,
        "project": target.project,
        "requiresApproval": target.requires_approval,
        "ready": view.ready,
        "checks": view.checks,
    }))
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

    fn mcp_exposure(&self) -> McpExposure {
        McpExposure::DeploysRead
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
        require_mcp_app(args, run.application_id)?;
        let view = events::view(&ctx.db, &run).await?;
        let tail = log_tail(run.log.as_deref());
        Ok(ToolOutput::data(json!({
            "runId": view.id,
            "applicationId": view.application_id,
            "application": view.application_name,
            "targetId": view.target_id,
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

    fn mcp_exposure(&self) -> McpExposure {
        McpExposure::DeploysRead
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Docker
    }

    fn description(&self) -> &'static str {
        "Compilações de imagem no servidor (deploy contínuo sem GitHub Actions): aplicação, commit, branch, quem pediu, estado, etapa, imagens geradas e o erro. Use para 'a última compilação falhou?' ou 'o que está compilando?'."
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
        let requested_app = args.integer("applicationId");
        if let Some(app_id) = requested_app {
            require_mcp_app(args, app_id)?;
        }
        let rows = match (
            requested_app,
            args.mcp()
                .and_then(|mcp| mcp.allowed_application_ids.as_deref()),
        ) {
            (Some(app_id), _) => builds::recent(&ctx.db, Some(app_id), 15).await?,
            (None, Some(allowed)) => builds::recent_for_applications(&ctx.db, allowed, 15).await?,
            (None, None) => builds::recent(&ctx.db, None, 15).await?,
        };
        let views = builds::views(&ctx.db, &rows).await?;
        let items: Vec<Value> = rows
            .iter()
            .zip(views.iter())
            .map(|(row, view)| {
                json!({
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
                })
            })
            .collect();
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

    fn mcp_exposure(&self) -> McpExposure {
        McpExposure::DeploysRead
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
        require_mcp_app(args, build.application_id)?;
        let view = builds::view(&ctx.db, &build).await?;
        Ok(ToolOutput::data(json!({
            "buildId": view.id,
            "applicationId": view.application_id,
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
    /// Cancela um deploy pendente (`runId`) ou uma compilação em andamento (`buildId`).
    Cancel,
}

impl Action {
    const NAMES: [&'static str; 6] = ["approve", "reject", "deploy", "restore", "build", "cancel"];

    const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Deploy => "deploy",
            Self::Restore => "restore",
            Self::Build => "build",
            Self::Cancel => "cancel",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "approve" => Some(Self::Approve),
            "reject" => Some(Self::Reject),
            "deploy" => Some(Self::Deploy),
            "restore" => Some(Self::Restore),
            "build" => Some(Self::Build),
            "cancel" => Some(Self::Cancel),
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
    let origin = via_label(args);
    format!("{} ({origin})", name.unwrap_or_else(|| "IA".to_string()))
}

#[async_trait]
impl AiToolHandler for DeployAction {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn name(&self) -> &'static str {
        "deploy_action"
    }

    fn mcp_exposure(&self) -> McpExposure {
        McpExposure::DeploysAction
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
        "Age sobre o deploy contínuo, sempre com confirmação do usuário: 'approve'/'reject' um deploy aguardando aprovação (runId), 'deploy' publica uma versão num alvo (targetId e commitSha completo; sem commit republica a versão no ar), 'restore' volta um alvo à versão de um deploy que deu certo (runId), 'build' compila no servidor uma aplicação que não usa GitHub Actions (applicationId; branch, commitSha e noCache opcionais) e os alvos recebem o deploy ao fim, 'cancel' cancela um deploy pendente (runId) ou uma compilação em andamento (buildId). 'when': 'now' (padrão) ou 'window' (janela de manutenção do alvo)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": Action::NAMES },
                "runId": { "type": "integer", "description": "Deploy (approve, reject, restore, cancel)" },
                "buildId": { "type": "integer", "description": "Compilação (cancel)" },
                "targetId": { "type": "integer", "description": "Alvo (deploy)" },
                "applicationId": { "type": "integer", "description": "Aplicação (build)" },
                "branch": { "type": "string", "description": "Branch a compilar (build)" },
                "commitSha": { "type": "string", "description": "SHA completo do commit (deploy, build)" },
                "noCache": { "type": "boolean", "description": "Ignorar cache do BuildKit (build)" },
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
                let cache = if args.flag("noCache") {
                    " (sem cache)"
                } else {
                    ""
                };
                format!(
                    "Compilar {version}{branch} de {application}{cache} no servidor e publicar nos alvos"
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
            Some(Action::Cancel) => {
                if let Some(id) = args.integer("runId") {
                    format!("Cancelar o deploy #{id}")
                } else if let Some(id) = args.integer("buildId") {
                    format!("Cancelar a compilação #{id}")
                } else {
                    "Cancelar deploy ou compilação não informados".to_string()
                }
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
        // Defesa em profundidade: mesmo que o chamador não cheque `admin_only()`,
        // ações de deploy só rodam para administradores da área Servidores.
        require_area_admin(
            ctx,
            args,
            Area::Servers,
            "Apenas administradores podem executar ações de deploy.",
        )
        .await?;

        let Some(action) = args.text("action").as_deref().and_then(Action::parse) else {
            return Ok(ToolOutput::not_found(format!(
                "Informe 'action': {}",
                Action::NAMES.join(", ")
            )));
        };
        let origin = via_label(args);
        let user = actor_name(ctx, args).await;
        if action == Action::Build {
            return build(ctx, args, &user, &origin).await;
        }
        if action == Action::Cancel && args.integer("runId").is_none() {
            return cancel_build(ctx, args, &origin).await;
        }
        if matches!(action, Action::Approve | Action::Reject) {
            if let Some(mcp) = args.mcp() {
                mcp.require_deploy_approval_scope()?;
            }
        }
        let when = when_of(args);
        let auto_approve = args
            .mcp()
            .is_none_or(McpToolContext::auto_approve_manual_deploys);
        let result = match action {
            Action::Deploy => match args.integer("targetId") {
                Some(target_id) => {
                    let target = match applications::find_target(&ctx.db, target_id).await {
                        Ok(target) => target,
                        Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
                    };
                    require_mcp_app(args, target.application_id)?;
                    orchestrator::request_manual_with_approval_policy(
                        ctx,
                        target_id,
                        &DeployInput {
                            commit_sha: args.text("commitSha"),
                            image_tag: None,
                            when,
                        },
                        &user,
                        auto_approve,
                    )
                    .await
                }
                None => return Ok(ToolOutput::not_found("Informe 'targetId' (get_deploys)")),
            },
            other => {
                let Some(run_id) = args.integer("runId") else {
                    return Ok(ToolOutput::not_found("Informe 'runId' (get_deploys)"));
                };
                let existing = match runs::find(&ctx.db, run_id).await {
                    Ok(run) => run,
                    Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
                };
                require_mcp_app(args, existing.application_id)?;
                // Cancelar o que aguarda aprovação é recusá-lo: pelo MCP, só
                // com o escopo de aprovar.
                if other == Action::Cancel
                    && existing.status == RunStatus::AwaitingApproval.as_str()
                {
                    if let Some(mcp) = args.mcp() {
                        mcp.require_deploy_approval_scope()?;
                    }
                }
                match other {
                    Action::Approve => orchestrator::approve(ctx, run_id, when, &user).await,
                    Action::Reject => {
                        orchestrator::reject(ctx, run_id, &user, args.text("reason").as_deref())
                            .await
                    }
                    Action::Cancel => orchestrator::cancel(ctx, run_id, &user).await,
                    _ => {
                        orchestrator::request_restore_with_approval_policy(
                            ctx,
                            run_id,
                            when,
                            &user,
                            auto_approve,
                        )
                        .await
                    }
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
                description: Some(format!("Deploy: {} ({origin})", action.as_str())),
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
async fn build(
    ctx: &AppContext,
    args: &ToolArgs,
    user: &str,
    origin: &str,
) -> AppResult<ToolOutput> {
    let Some(application_id) = args.integer("applicationId") else {
        return Ok(ToolOutput::not_found(
            "Informe 'applicationId' (get_deploys)",
        ));
    };
    require_mcp_app(args, application_id)?;
    let application = match applications::find(&ctx.db, application_id).await {
        Ok(application) => application,
        Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
    };
    let branch = args.text("branch");
    let commit = args.text("commitSha");
    let no_cache = args.flag("noCache");
    let requested = builds::request_manual(
        ctx,
        &application,
        &builds::ManualBuild {
            reference: branch.as_deref(),
            commit: commit.as_deref(),
            no_cache,
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
                "Compilação #{} de {} ({}) ({origin})",
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
        "noCache": build.no_cache,
        "status": build.status,
    })))
}

/// `deploy_action` com `cancel` e `buildId`: cancela uma compilação na fila ou em curso.
async fn cancel_build(ctx: &AppContext, args: &ToolArgs, origin: &str) -> AppResult<ToolOutput> {
    let Some(build_id) = args.integer("buildId") else {
        return Ok(ToolOutput::not_found(
            "Informe 'runId' ou 'buildId' para cancelar",
        ));
    };
    let existing = match builds::find(&ctx.db, build_id).await {
        Ok(build) => build,
        Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
    };
    require_mcp_app(args, existing.application_id)?;
    let cancelled = match builds::cancel(ctx, build_id).await {
        Ok(build) => build,
        Err(error) => return Ok(ToolOutput::not_found(error.to_string())),
    };
    audit(
        ctx,
        args,
        AuditEntryInput {
            action: AuditAction::Execute,
            resource_type: ResourceType::Deploy,
            resource_id: Some(cancelled.id),
            resource_label: Some(format!("Compilação #{}", cancelled.id)),
            description: Some(format!("Deploy: cancel build #{} ({origin})", cancelled.id)),
            changes: None,
        },
    )
    .await;
    Ok(ToolOutput::data(json!({
        "ok": true,
        "buildId": cancelled.id,
        "status": cancelled.status,
    })))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use loco_rs::app::AppContext;
    use sea_orm::{ActiveModelTrait, ConnectOptions, Database, Set};
    use serde_json::json;
    use uuid::Uuid;

    use netmonitor_core::{
        ai::args::{McpToolContext, ToolArgs},
        audit::AuditActor,
        mcp::McpDeployAppCatalog,
        models::{deploy_applications, deploy_builds, deploy_runs, deploy_targets, probes, users},
        shared::errors::AppError,
    };

    use super::*;
    use crate::{
        deploys::ServersDeployAppCatalog,
        migrations::{MigratorTrait, TestMigrator},
    };

    const COMMIT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const COMMIT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    async fn test_ctx() -> AppContext {
        let db = Database::connect(
            ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco em memória");
        TestMigrator::up(&db, None).await.expect("migrations");
        let mut ctx = loco_rs::tests_cfg::app::get_app_context().await;
        ctx.db = db;
        ctx
    }

    async fn insert_user(ctx: &AppContext, role: &str, active: bool) -> users::Model {
        let pid = Uuid::new_v4();
        users::ActiveModel {
            pid: Set(pid),
            email: Set(format!("{role}-{pid}@test.local")),
            password: Set("hash".into()),
            api_key: Set(format!("key-{pid}")),
            name: Set(format!("User {role}")),
            active: Set(active),
            role: Set(role.into()),
            area_roles: Set(Some(json!({ "servers": "operator" }))),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("usuário")
    }

    async fn insert_agent(ctx: &AppContext, name: &str) -> probes::Model {
        probes::ActiveModel {
            name: Set(name.into()),
            token_hash: Set(format!("hash-{name}")),
            status: Set("offline".into()),
            configuration: Set(Some(json!({ "role": "agent" }))),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("agente")
    }

    async fn insert_app(ctx: &AppContext, name: &str, slug: &str) -> deploy_applications::Model {
        deploy_applications::ActiveModel {
            name: Set(name.into()),
            slug: Set(slug.into()),
            repository: Set(format!("acme/{slug}")),
            compose_files: Set(json!(["docker-compose.yml"])),
            image_tag_variable: Set("IMAGE_TAG".into()),
            images: Set(json!([])),
            rollout: Set("parallel".into()),
            builder: Set("netmonitor".into()),
            build_args: Set(json!([])),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("aplicação")
    }

    async fn insert_target(
        ctx: &AppContext,
        app_id: i64,
        probe_id: i64,
        name: &str,
        project: &str,
        requires_approval: bool,
    ) -> deploy_targets::Model {
        deploy_targets::ActiveModel {
            application_id: Set(app_id),
            name: Set(name.into()),
            probe_id: Set(Some(probe_id)),
            project: Set(project.into()),
            reference: Set("main".into()),
            requires_approval: Set(requires_approval),
            wait_timeout_secs: Set(180),
            auto_rollback: Set(true),
            keep_images: Set(3),
            enabled: Set(true),
            position: Set(0),
            vulnerability_policy: Set("warn".into()),
            require_provenance: Set(false),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alvo")
    }

    async fn insert_run(
        ctx: &AppContext,
        app_id: i64,
        target_id: i64,
        status: RunStatus,
        log: Option<&str>,
    ) -> deploy_runs::Model {
        deploy_runs::ActiveModel {
            application_id: Set(app_id),
            target_id: Set(target_id),
            trigger: Set("manual".into()),
            commit_sha: Set(COMMIT_A.into()),
            image_tag: Set(COMMIT_A.into()),
            status: Set(status.as_str().into()),
            log: Set(log.map(str::to_owned)),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("run")
    }

    async fn insert_build(
        ctx: &AppContext,
        app_id: i64,
        status: &str,
        log: Option<&str>,
    ) -> deploy_builds::Model {
        let now = Utc::now().into();
        deploy_builds::ActiveModel {
            application_id: Set(app_id),
            reference: Set(Some("refs/heads/main".into())),
            commit_sha: Set(COMMIT_A.into()),
            trigger: Set("manual".into()),
            status: Set(status.into()),
            no_cache: Set(false),
            images: Set(json!([])),
            log: Set(log.map(str::to_owned)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("build")
    }

    fn actor(user_id: i64) -> AuditActor {
        AuditActor {
            user_id: Some(user_id),
            ip: Some("127.0.0.1".into()),
            user_agent: Some("test".into()),
        }
    }

    fn mcp_ctx(allowed: Option<Vec<i64>>, can_approve: bool) -> McpToolContext {
        McpToolContext {
            token_id: 10,
            token_name: "ide-test".into(),
            allowed_application_ids: allowed,
            respect_target_approval: true,
            can_approve_deploys: can_approve,
        }
    }

    #[tokio::test]
    async fn catalogo_mcp_lista_aplicacoes_ordenadas() {
        let ctx = test_ctx().await;
        insert_app(&ctx, "Zeta", "zeta").await;
        insert_app(&ctx, "Alfa", "alfa").await;

        let list = ServersDeployAppCatalog
            .list_applications(&ctx.db)
            .await
            .expect("listar");
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Alfa", "Zeta"]);
        assert_eq!(list[0].repository, "acme/alfa");
    }

    #[tokio::test]
    async fn leituras_respeitam_allowed_application_ids() {
        let ctx = test_ctx().await;
        let agent = insert_agent(&ctx, "srv-1").await;
        let app1 = insert_app(&ctx, "App Um", "app-um").await;
        let app2 = insert_app(&ctx, "App Dois", "app-dois").await;
        let t1 = insert_target(&ctx, app1.id, agent.id, "Prod 1", "app-um", true).await;
        let t2 = insert_target(&ctx, app2.id, agent.id, "Prod 2", "app-dois", true).await;
        let r1 = insert_run(&ctx, app1.id, t1.id, RunStatus::Failed, Some("erro app1")).await;
        let r2 = insert_run(&ctx, app2.id, t2.id, RunStatus::Failed, Some("erro app2")).await;
        let b1 = insert_build(&ctx, app1.id, "succeeded", Some("build ok 1")).await;
        let b2 = insert_build(&ctx, app2.id, "failed", Some("build err 2")).await;

        let scoped = mcp_ctx(Some(vec![app1.id]), false);

        // get_deploys filtra só app1 e recalcula contadores
        let out = DeployStatus
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({})).with_mcp(scoped.clone()),
            )
            .await
            .expect("get_deploys");
        assert_eq!(out.data["applications"].as_array().unwrap().len(), 1);
        assert_eq!(out.data["applications"][0]["applicationId"], app1.id);
        assert_eq!(out.data["recentRuns"].as_array().unwrap().len(), 1);
        assert_eq!(out.data["recentRuns"][0]["runId"], r1.id);
        assert_eq!(out.data["counts"]["applications"], 1);

        // get_deploys com applicationId ou targetId proibido retorna Forbidden
        let err = DeployStatus
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "applicationId": app2.id })).with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));

        let err = DeployStatus
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "targetId": t2.id })).with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));

        // get_deploy_log permite r1 e bloqueia r2
        let ok_log = DeployLog
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "runId": r1.id })).with_mcp(scoped.clone()),
            )
            .await
            .expect("log permitido");
        assert_eq!(ok_log.data["logTail"], "erro app1");
        let err = DeployLog
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "runId": r2.id })).with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));

        // get_builds filtra só app1 e bloqueia applicationId = app2
        let ok_builds = BuildStatus
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({})).with_mcp(scoped.clone()),
            )
            .await
            .expect("builds filtrados");
        assert_eq!(ok_builds.data["builds"].as_array().unwrap().len(), 1);
        assert_eq!(ok_builds.data["builds"][0]["buildId"], b1.id);
        let err = BuildStatus
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "applicationId": app2.id })).with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));

        // get_build_log permite b1 e bloqueia b2
        let ok_blog = BuildLog
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "buildId": b1.id })).with_mcp(scoped.clone()),
            )
            .await
            .expect("build log permitido");
        assert_eq!(ok_blog.data["logTail"], "build ok 1");
        let err = BuildLog
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "buildId": b2.id })).with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));

        // get_deploy_readiness permite t1 e bloqueia t2/app2
        let ready = DeployReadiness
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "targetId": t1.id })).with_mcp(scoped.clone()),
            )
            .await
            .expect("prontidão permitida");
        assert_eq!(ready.data["targetId"], t1.id);
        assert_eq!(ready.data["ready"], false);
        let err = DeployReadiness
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "targetId": t2.id })).with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
        let err = DeployReadiness
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "applicationId": app2.id })).with_mcp(scoped),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn deploy_action_exige_admin_e_respeita_aprovacao_e_escopos_mcp() {
        let ctx = test_ctx().await;
        let admin = insert_user(&ctx, "admin", true).await;
        let operator = insert_user(&ctx, "operator", true).await;
        let inactive_admin = insert_user(&ctx, "admin", false).await;
        let agent = insert_agent(&ctx, "srv-prod").await;
        let app1 = insert_app(&ctx, "SGC", "sgc").await;
        let app2 = insert_app(&ctx, "Outro", "outro").await;
        let prod = insert_target(&ctx, app1.id, agent.id, "Produção", "sgc-prod", true).await;
        let stage = insert_target(&ctx, app1.id, agent.id, "Homolog", "sgc-stage", false).await;
        let other_target = insert_target(&ctx, app2.id, agent.id, "Outro", "outro", false).await;

        // Defesa em profundidade: operator, admin inativo ou sem ator são bloqueados
        for bad_args in [
            ToolArgs::from_value(
                json!({ "action": "deploy", "targetId": prod.id, "commitSha": COMMIT_A }),
            )
            .with_actor(actor(operator.id)),
            ToolArgs::from_value(
                json!({ "action": "deploy", "targetId": prod.id, "commitSha": COMMIT_A }),
            )
            .with_actor(actor(inactive_admin.id)),
            ToolArgs::from_value(
                json!({ "action": "deploy", "targetId": prod.id, "commitSha": COMMIT_A }),
            ),
        ] {
            let err = DeployAction.execute(&ctx, &bad_args).await.unwrap_err();
            assert!(matches!(err, AppError::Forbidden(_)));
        }

        // MCP com respect_target_approval=true: alvo com requires_approval entra em awaiting_approval
        let mcp_no_approve = mcp_ctx(Some(vec![app1.id]), false);
        let pending_out = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({
                    "action": "deploy",
                    "targetId": prod.id,
                    "commitSha": COMMIT_A,
                }))
                .with_actor(actor(admin.id))
                .with_mcp(mcp_no_approve.clone()),
            )
            .await
            .expect("deploy aguardando aprovação");
        assert_eq!(pending_out.data["status"], "awaiting_approval");
        let pending_id = pending_out.data["runId"].as_i64().unwrap();
        let stored = runs::find(&ctx.db, pending_id).await.unwrap();
        assert!(stored.approved_by.is_none());

        // Sem escopo deploys:approve, approve e reject via MCP falham com Forbidden
        for act in ["approve", "reject"] {
            let err = DeployAction
                .execute(
                    &ctx,
                    &ToolArgs::from_value(json!({ "action": act, "runId": pending_id }))
                        .with_actor(actor(admin.id))
                        .with_mcp(mcp_no_approve.clone()),
                )
                .await
                .unwrap_err();
            assert!(matches!(err, AppError::Forbidden(_)));
        }

        // Com escopo deploys:approve, aprovar funciona
        let mcp_with_approve = mcp_ctx(Some(vec![app1.id]), true);
        let approved_out = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "action": "approve", "runId": pending_id }))
                    .with_actor(actor(admin.id))
                    .with_mcp(mcp_with_approve.clone()),
            )
            .await
            .expect("aprovar com escopo");
        assert_eq!(approved_out.data["status"], "queued");

        // MCP em alvo SEM requires_approval entra direto na fila (queued)
        let stage_out = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({
                    "action": "deploy",
                    "targetId": stage.id,
                    "commitSha": COMMIT_A,
                }))
                .with_actor(actor(admin.id))
                .with_mcp(mcp_no_approve.clone()),
            )
            .await
            .expect("deploy homolog");
        assert_eq!(stage_out.data["status"], "queued");

        // Chat/UI (sem MCP) mantém auto_approve=true mesmo em alvo com requires_approval=true
        let direct_out = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({
                    "action": "deploy",
                    "targetId": prod.id,
                    "commitSha": COMMIT_B,
                }))
                .with_actor(actor(admin.id)),
            )
            .await
            .expect("deploy direto sem MCP");
        assert_eq!(direct_out.data["status"], "queued");

        // MCP fora de allowed_application_ids é bloqueado
        let err = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({
                    "action": "deploy",
                    "targetId": other_target.id,
                    "commitSha": COMMIT_A,
                }))
                .with_actor(actor(admin.id))
                .with_mcp(mcp_no_approve),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn deploy_action_suporta_cancel_e_build_no_cache() {
        let ctx = test_ctx().await;
        let admin = insert_user(&ctx, "admin", true).await;
        let agent = insert_agent(&ctx, "srv-build").await;
        let app1 = insert_app(&ctx, "App1", "app1").await;
        let app2 = insert_app(&ctx, "App2", "app2").await;
        let t1 = insert_target(&ctx, app1.id, agent.id, "Prod", "app1", true).await;
        let run1 = insert_run(&ctx, app1.id, t1.id, RunStatus::AwaitingApproval, None).await;
        let build1 = insert_build(&ctx, app1.id, "queued", None).await;
        let build2 = insert_build(&ctx, app2.id, "queued", None).await;
        let scoped = mcp_ctx(Some(vec![app1.id]), false);

        // Aguardando aprovação: cancelar é recusar, e o token não aprova
        let refused = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "action": "cancel", "runId": run1.id }))
                    .with_actor(actor(admin.id))
                    .with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(refused, AppError::Forbidden(_)));

        // Com o escopo de aprovar, cancelar runId é permitido
        let cancelled_run = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "action": "cancel", "runId": run1.id }))
                    .with_actor(actor(admin.id))
                    .with_mcp(mcp_ctx(Some(vec![app1.id]), true)),
            )
            .await
            .expect("cancelar deploy");
        assert_eq!(cancelled_run.data["status"], "cancelled");

        // Cancelar buildId permitido
        let cancelled_build = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "action": "cancel", "buildId": build1.id }))
                    .with_actor(actor(admin.id))
                    .with_mcp(scoped.clone()),
            )
            .await
            .expect("cancelar build");
        assert_eq!(cancelled_build.data["status"], "cancelled");

        // Cancelar buildId fora do escopo falha com Forbidden
        let err = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({ "action": "cancel", "buildId": build2.id }))
                    .with_actor(actor(admin.id))
                    .with_mcp(scoped.clone()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));

        // Build com noCache=true
        let built = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({
                    "action": "build",
                    "applicationId": app1.id,
                    "branch": "main",
                    "commitSha": COMMIT_B,
                    "noCache": true,
                }))
                .with_actor(actor(admin.id))
                .with_mcp(scoped.clone()),
            )
            .await
            .expect("disparar build noCache");
        assert_eq!(built.data["ok"], true);
        assert_eq!(built.data["noCache"], true);

        // Build fora do escopo falha com Forbidden
        let err = DeployAction
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({
                    "action": "build",
                    "applicationId": app2.id,
                    "branch": "main",
                    "commitSha": COMMIT_B,
                }))
                .with_actor(actor(admin.id))
                .with_mcp(scoped),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn deploy_guide_respeita_escopo_mcp() {
        let ctx = test_ctx().await;
        let agent = insert_agent(&ctx, "srv-guide").await;
        let app1 = insert_app(&ctx, "Permitida", "permitida").await;
        let app2 = insert_app(&ctx, "Secreta", "secreta").await;
        let t2 = insert_target(&ctx, app2.id, agent.id, "ProdSecreta", "secreta", false).await;
        insert_run(&ctx, app2.id, t2.id, RunStatus::Failed, None).await;

        let out = DeployGuide
            .execute(
                &ctx,
                &ToolArgs::from_value(json!({})).with_mcp(mcp_ctx(Some(vec![app1.id]), false)),
            )
            .await
            .expect("guia escopado");
        let serialized = out.data.to_string();
        assert!(serialized.contains("Permitida"));
        assert!(!serialized.contains("Secreta"));
        assert!(!serialized.contains("ProdSecreta"));
    }
}
