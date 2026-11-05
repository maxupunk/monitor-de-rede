//! O que a descoberta contribui com a IA (ADR 017): a seção `discovery` do
//! `get_platform_status` — varreduras recentes e a em andamento.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use netmonitor_core::ai::platform::{rfc3339, PlatformSection};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use super::service::ScanSessionService;
use crate::{
    models::_entities::{discovery_results, discovery_runs},
    services::{ai_tools::platform::network_names, shared::errors::AppResult},
};

/// A seção `discovery` do estado da plataforma.
pub struct DiscoveryStatus;

#[async_trait]
impl PlatformSection for DiscoveryStatus {
    fn key(&self) -> &'static str {
        "discovery"
    }

    async fn report(
        &self,
        ctx: &AppContext,
        since: DateTime<Utc>,
        limit: usize,
    ) -> AppResult<Value> {
        let runs = discovery_runs::Entity::find()
            .filter(discovery_runs::Column::StartedAt.gte(since))
            .order_by_desc(discovery_runs::Column::StartedAt)
            .limit(limit as u64)
            .all(&ctx.db)
            .await?;
        let run_ids: Vec<i64> = runs.iter().map(|run| run.id).collect();
        let mut found: HashMap<i64, usize> = HashMap::new();
        for run_id in discovery_results::Entity::find()
            .select_only()
            .column(discovery_results::Column::DiscoveryRunId)
            .filter(discovery_results::Column::DiscoveryRunId.is_in(run_ids))
            .into_tuple::<i64>()
            .all(&ctx.db)
            .await?
        {
            *found.entry(run_id).or_default() += 1;
        }
        let network_names = network_names(ctx).await?;
        let live = match ScanSessionService::from_context(ctx) {
            Ok(service) => {
                let state = service.state().await;
                state.run_id.map(|run_id| {
                    json!({
                        "run_id": run_id,
                        "status": state.status,
                        "phase": state.phase,
                        "progress": format!("{}/{}", state.progress_current, state.progress_total),
                        "hosts_found": state.hosts.len(),
                        "error": state.error,
                    })
                })
            }
            Err(_) => None,
        };
        Ok(json!({
            "current": live,
            "runs": runs.iter().map(|run| json!({
                "id": run.id,
                "network": network_names.get(&run.network_id),
                "status": run.status,
                "started_at": rfc3339(&run.started_at),
                "finished_at": run.finished_at.as_ref().map(rfc3339),
                "hosts_found": found.get(&run.id).copied().unwrap_or(0),
                "error": run.error,
            })).collect::<Vec<_>>(),
        }))
    }
}
