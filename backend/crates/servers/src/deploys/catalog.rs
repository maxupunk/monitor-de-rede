//! A página de deploys numa consulta: aplicações com os alvos (agente,
//! versão no ar, pendência, `.env`), os deploys recentes e os números do
//! topo. É a hidratação que junta as tabelas; a forma de cada peça está em
//! [`crate::views::deploys`].

use std::collections::HashMap;

use chrono::{Duration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use agent_protocol::policy::Permission;
use netmonitor_core::{
    agents::{hub::AgentHub, identity},
    models::{database_connections, deploy_applications, deploy_runs, deploy_targets},
    shared::errors::AppResult,
};

use super::{
    applications, credentials, env_versions, events, oidc,
    runs::{self, status_of},
    workflow::HOOK_PATH,
    RunStatus,
};
use crate::views::deploys::{
    DeployAgentView, DeployApplicationView, DeployCounts, DeployCredentialView,
    DeployEnvVersionView, DeployOverview, DeployRunView, DeployTargetView, TargetParts,
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

async fn target_view(
    ctx: &AppContext,
    target: &deploy_targets::Model,
    agents: &[DeployAgentView],
    connections: &HashMap<i64, String>,
    active: &[deploy_runs::Model],
    lookup: &Lookup,
) -> AppResult<DeployTargetView> {
    let live = runs::live(&ctx.db, target.id).await?;
    let pending = active
        .iter()
        .filter(|run| run.target_id == target.id)
        .max_by_key(|run| run.id);
    let env = env_versions::current(&ctx.db, target.id).await?;
    Ok(DeployTargetView::new(
        target,
        TargetParts {
            agent: target
                .probe_id
                .and_then(|id| agents.iter().find(|agent| agent.id == id).cloned()),
            backup_connection_name: target
                .backup_connection_id
                .and_then(|id| connections.get(&id).cloned()),
            live: live.as_ref().map(|run| lookup.run(run)),
            pending: pending.map(|run| lookup.run(run)),
            env: env
                .as_ref()
                .map(|version| DeployEnvVersionView::new(version, true)),
        },
    ))
}

async fn count_since(
    ctx: &AppContext,
    status: RunStatus,
    since: chrono::DateTime<Utc>,
) -> AppResult<u32> {
    let count = deploy_runs::Entity::find()
        .filter(deploy_runs::Column::Status.eq(status.as_str()))
        .filter(deploy_runs::Column::CreatedAt.gte(since))
        .count(&ctx.db)
        .await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
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

    let mut views = Vec::with_capacity(application_rows.len());
    for application in &application_rows {
        let mut targets = Vec::new();
        for target in target_rows
            .iter()
            .filter(|target| target.application_id == application.id)
        {
            targets.push(target_view(ctx, target, &agents, &connections, &active, &lookup).await?);
        }
        views.push(DeployApplicationView::new(
            application,
            application
                .credential_id
                .and_then(|id| credential_names.get(&id).cloned()),
            targets,
        ));
    }

    let since = Utc::now() - Duration::hours(24);
    let counts = DeployCounts {
        applications: u32::try_from(application_rows.len()).unwrap_or(u32::MAX),
        targets: u32::try_from(target_rows.len()).unwrap_or(u32::MAX),
        succeeded_24h: count_since(ctx, RunStatus::Succeeded, since).await?,
        failed_24h: count_since(ctx, RunStatus::Failed, since).await?
            + count_since(ctx, RunStatus::RolledBack, since).await?,
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
    Ok(DeployOverview {
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
        hook_path: HOOK_PATH.to_string(),
        oidc_audience: oidc::audience(),
    })
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
    applications::find_target(&ctx.db, target_id).await?;
    let mut out = Vec::new();
    for run in runs::recent(&ctx.db, Some(target_id), limit).await? {
        out.push(events::view(&ctx.db, &run).await?);
    }
    Ok(out)
}
