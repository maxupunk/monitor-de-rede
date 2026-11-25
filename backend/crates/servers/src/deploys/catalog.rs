//! A página de deploys numa consulta: aplicações com os alvos (agente,
//! versão no ar, pendência, `.env`), os deploys recentes e os números do
//! topo. É a hidratação que junta as tabelas; a forma de cada peça está em
//! [`crate::views::deploys`].

use std::collections::HashMap;

use chrono::{Duration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

use agent_protocol::policy::Permission;
use netmonitor_core::{
    agents::{hub::AgentHub, identity},
    models::{
        database_connections, deploy_applications, deploy_env_versions, deploy_runs, deploy_targets,
    },
    shared::errors::AppResult,
};

use super::{
    applications, builds, connections as github_connections, credentials, env_versions, guide,
    oidc,
    runs::{self, status_of},
    workflow::HOOK_PATH,
    RunStatus,
};
use crate::views::deploys::{
    DeployAgentView, DeployApplicationView, DeployCounts, DeployCredentialView,
    DeployEnvVersionView, DeployGithubConnectionView, DeployGuideView, DeployOverview,
    DeployRunView, DeployTargetView, TargetParts,
};

/// Deploys recentes na visão geral.
const RECENT_RUNS: u64 = 30;

/// Os agentes que podem receber deploy — o local incluído (é o Docker desta
/// central, ADR 014).
///
/// # Errors
///
/// Banco.
pub async fn agents(ctx: &AppContext) -> AppResult<Vec<DeployAgentView>> {
    let hub = AgentHub::from_context(ctx).ok();
    Ok(identity::list(&ctx.db)
        .await?
        .into_iter()
        .filter(|agent| agent.status != netmonitor_core::models::probes::STATUS_REVOKED)
        .map(|agent| {
            let session = hub.as_ref().and_then(|hub| hub.get(agent.id));
            DeployAgentView {
                id: agent.id,
                name: agent.name,
                connected: session.is_some(),
                allows_deploy: session
                    .as_ref()
                    .is_none_or(|session| session.allows(Permission::Deploy)),
                allows_build: session
                    .as_ref()
                    .is_some_and(|session| session.allows(Permission::Build)),
            }
        })
        .collect())
}

struct Lookup {
    applications: HashMap<i64, deploy_applications::Model>,
    targets: HashMap<i64, deploy_targets::Model>,
}

impl Lookup {
    fn run(&self, run: &deploy_runs::Model) -> DeployRunView {
        DeployRunView::new(
            run,
            self.applications.get(&run.application_id),
            self.targets.get(&run.target_id),
        )
    }
}

fn target_view(
    target: &deploy_targets::Model,
    agents: &[DeployAgentView],
    connections: &HashMap<i64, String>,
    live_by_target: &HashMap<i64, deploy_runs::Model>,
    env_by_target: &HashMap<i64, deploy_env_versions::Model>,
    active: &[deploy_runs::Model],
    lookup: &Lookup,
) -> DeployTargetView {
    let live = live_by_target.get(&target.id);
    let pending = active
        .iter()
        .filter(|run| run.target_id == target.id)
        .max_by_key(|run| run.id);
    let env = env_by_target.get(&target.id);
    DeployTargetView::new(
        target,
        TargetParts {
            agent: target
                .probe_id
                .and_then(|id| agents.iter().find(|agent| agent.id == id).cloned()),
            backup_connection_name: target
                .backup_connection_id
                .and_then(|id| connections.get(&id).cloned()),
            live: live.map(|run| lookup.run(run)),
            pending: pending.map(|run| lookup.run(run)),
            env: env.map(|version| DeployEnvVersionView::new(version, true)),
        },
    )
}

async fn counts_24h(ctx: &AppContext, since: chrono::DateTime<Utc>) -> AppResult<(u32, u32)> {
    let rows: Vec<(String, i64)> = deploy_runs::Entity::find()
        .select_only()
        .column(deploy_runs::Column::Status)
        .column_as(deploy_runs::Column::Id.count(), "total")
        .filter(deploy_runs::Column::CreatedAt.gte(since))
        .filter(deploy_runs::Column::Status.is_in([
            RunStatus::Succeeded.as_str(),
            RunStatus::Failed.as_str(),
            RunStatus::RolledBack.as_str(),
        ]))
        .group_by(deploy_runs::Column::Status)
        .into_tuple::<(String, i64)>()
        .all(&ctx.db)
        .await?;
    let mut succeeded = 0u32;
    let mut failed = 0u32;
    for (status, total) in rows {
        let count = u32::try_from(total).unwrap_or(u32::MAX);
        match RunStatus::parse(&status) {
            Ok(RunStatus::Succeeded) => succeeded = succeeded.saturating_add(count),
            Ok(RunStatus::Failed | RunStatus::RolledBack) => {
                failed = failed.saturating_add(count);
            }
            _ => {}
        }
    }
    Ok((succeeded, failed))
}

/// A visão geral da página.
///
/// # Errors
///
/// Banco.
pub async fn overview(ctx: &AppContext) -> AppResult<DeployOverview> {
    let agents = agents(ctx).await?;
    let connections: HashMap<i64, String> = database_connections::Entity::find()
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|row| (row.id, row.name))
        .collect();
    let credential_rows = credentials::list(&ctx.db).await?;
    let credential_names: HashMap<i64, String> = credential_rows
        .iter()
        .map(|row| (row.id, row.name.clone()))
        .collect();
    let connection_rows = github_connections::list(&ctx.db).await?;
    let accounts: HashMap<i64, String> = connection_rows
        .iter()
        .map(|row| (row.id, row.account_login.clone()))
        .collect();
    let application_rows = applications::list(&ctx.db).await?;
    let target_rows = applications::all_targets(&ctx.db).await?;
    let lookup = Lookup {
        applications: application_rows
            .iter()
            .map(|row| (row.id, row.clone()))
            .collect(),
        targets: target_rows
            .iter()
            .map(|row| (row.id, row.clone()))
            .collect(),
    };
    let mut active_statuses = RunStatus::PENDING.to_vec();
    active_statuses.push(RunStatus::Running);
    let active = runs::with_status(&ctx.db, &active_statuses).await?;
    let target_ids: Vec<i64> = target_rows.iter().map(|target| target.id).collect();
    let live_by_target = runs::live_by_targets(&ctx.db, &target_ids).await?;
    let env_by_target = env_versions::current_by_targets(&ctx.db, &target_ids).await?;

    let mut views = Vec::with_capacity(application_rows.len());
    for application in &application_rows {
        let mut targets = Vec::new();
        for target in target_rows
            .iter()
            .filter(|target| target.application_id == application.id)
        {
            targets.push(target_view(
                target,
                &agents,
                &connections,
                &live_by_target,
                &env_by_target,
                &active,
                &lookup,
            ));
        }
        views.push(DeployApplicationView::new(
            application,
            application
                .credential_id
                .and_then(|id| credential_names.get(&id).cloned()),
            application
                .github_connection_id
                .and_then(|id| accounts.get(&id).cloned()),
            targets,
        ));
    }

    let since = Utc::now() - Duration::hours(24);
    let (succeeded_24h, failed_24h) = counts_24h(ctx, since).await?;
    let counts = DeployCounts {
        applications: u32::try_from(application_rows.len()).unwrap_or(u32::MAX),
        targets: u32::try_from(target_rows.len()).unwrap_or(u32::MAX),
        succeeded_24h,
        failed_24h,
        awaiting_approval: u32::try_from(
            active
                .iter()
                .filter(|run| status_of(run) == RunStatus::AwaitingApproval)
                .count(),
        )
        .unwrap_or(u32::MAX),
        running: u32::try_from(
            active
                .iter()
                .filter(|run| status_of(run) == RunStatus::Running)
                .count(),
        )
        .unwrap_or(u32::MAX),
    };
    let mut overview = DeployOverview {
        recent_runs: runs::recent(&ctx.db, None, RECENT_RUNS)
            .await?
            .iter()
            .map(|run| lookup.run(run))
            .collect(),
        applications: views,
        counts,
        credentials: credential_rows
            .iter()
            .map(DeployCredentialView::from)
            .collect(),
        github_connections: connection_rows
            .iter()
            .map(DeployGithubConnectionView::from)
            .collect(),
        hook_path: HOOK_PATH.to_string(),
        oidc_audience: oidc::audience(),
        guide: DeployGuideView::default(),
    };
    let latest_build = builds::recent(&ctx.db, None, 1)
        .await?
        .into_iter()
        .next()
        .map(|build| guide::BuildFact {
            status: builds::BuildStatus::parse(&build.status),
            short_sha: super::short_sha(&build.commit_sha).to_string(),
        });
    overview.guide = guide::steps(&guide::facts_of(&overview, &agents, latest_build));
    Ok(overview)
}

/// O histórico de um alvo, mais recentes primeiro.
///
/// # Errors
///
/// `404`; banco.
pub async fn history(
    ctx: &AppContext,
    target_id: i64,
    limit: u64,
) -> AppResult<Vec<DeployRunView>> {
    let target = applications::find_target(&ctx.db, target_id).await?;
    let application = applications::find(&ctx.db, target.application_id)
        .await
        .ok();
    Ok(runs::recent(&ctx.db, Some(target_id), limit)
        .await?
        .iter()
        .map(|run| DeployRunView::new(run, application.as_ref(), Some(&target)))
        .collect())
}

/// Catálogo de aplicações de deploy fornecido pela área Servidores ao servidor
/// MCP (ADR 017 / ADR 024).
pub struct ServersDeployAppCatalog;

#[async_trait::async_trait]
impl netmonitor_core::mcp::McpDeployAppCatalog for ServersDeployAppCatalog {
    async fn list_applications(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> AppResult<Vec<netmonitor_core::mcp::McpDeployAppSummary>> {
        Ok(applications::list(db)
            .await?
            .into_iter()
            .map(|app| netmonitor_core::mcp::McpDeployAppSummary {
                id: app.id,
                name: app.name,
                slug: app.slug,
                repository: app.repository,
            })
            .collect())
    }
}
