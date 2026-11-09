//! O andamento do deploy pelo barramento SSE (AGENTS §9).
//!
//! * `deploys:run` — o estado de um deploy, como **snapshot** por deploy:
//!   quem abre a tela no meio recebe os que estão em curso logo ao
//!   conectar. Terminado, o snapshot sai da memória (a tela já o recebeu; o
//!   histórico vem com a página).
//! * `deploys:log` — linhas novas do console, em lotes, efêmeras.
//! * `deploys:updated` — o cadastro mudou (aplicação, alvo, credencial,
//!   `.env`): a página recarrega a visão geral uma vez.

use loco_rs::app::AppContext;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::json;

use netmonitor_core::{
    events::EventBus,
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::errors::AppResult,
};

use super::runs::status_of;
use crate::views::deploys::DeployRunView;

pub const RUN_EVENT: &str = "deploys:run";
pub const LOG_EVENT: &str = "deploys:log";
pub const UPDATED_EVENT: &str = "deploys:updated";

fn snapshot_key(run_id: i64) -> String {
    format!("deploy-run:{run_id}")
}

/// O deploy com os nomes da aplicação e do alvo.
///
/// # Errors
///
/// Banco.
pub async fn view<C: ConnectionTrait>(
    db: &C,
    run: &deploy_runs::Model,
) -> AppResult<DeployRunView> {
    let application = deploy_applications::Entity::find_by_id(run.application_id)
        .one(db)
        .await?;
    let target = deploy_targets::Entity::find_by_id(run.target_id)
        .one(db)
        .await?;
    Ok(DeployRunView::new(
        run,
        application.as_ref(),
        target.as_ref(),
    ))
}

/// Publica o estado atual do deploy. Falha de banco só vira log: o
/// andamento não pode derrubar o deploy.
pub async fn publish_run(ctx: &AppContext, run_id: i64) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    let run = match super::runs::find(&ctx.db, run_id).await {
        Ok(run) => run,
        Err(error) => {
            tracing::warn!(run_id, %error, "deploy sumiu antes de ser publicado");
            return;
        }
    };
    let view = match view(&ctx.db, &run).await {
        Ok(view) => view,
        Err(error) => {
            tracing::warn!(run_id, %error, "não foi possível montar o deploy para o SSE");
            return;
        }
    };
    let key = snapshot_key(run_id);
    bus.publish_snapshot(RUN_EVENT, key.clone(), json!(view));
    if status_of(&run).is_terminal() {
        bus.forget_snapshots(&key);
    }
}

/// Linhas novas do console.
pub fn publish_lines(ctx: &AppContext, run_id: i64, lines: Vec<String>) {
    if lines.is_empty() {
        return;
    }
    if let Ok(bus) = EventBus::from_context(ctx) {
        bus.publish_ephemeral(LOG_EVENT, json!({ "runId": run_id, "lines": lines }));
    }
}

/// O cadastro mudou.
pub fn publish_updated(ctx: &AppContext) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        bus.publish_ephemeral(UPDATED_EVENT, json!({}));
    }
}
