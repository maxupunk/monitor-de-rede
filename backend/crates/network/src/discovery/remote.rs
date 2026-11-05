//! A varredura de uma rede pelo agente remoto ou pelo probe (ADR 011): a
//! entrega da run pendente, o despacho pelo canal do agente
//! ([`DiscoveryDispatch`], `AgentCommandDispatcher`) e o retorno dos hosts.

use std::{sync::Arc, time::Duration};

use agent_protocol::{
    policy::Permission,
    probes::{DiscoveredHost, ProbeDiscoveryResultPayload, ProbeDiscoveryTask},
    Command, ErrorCode, RemoteError,
};
use async_trait::async_trait;
use chrono::Utc;
use loco_rs::app::AppContext;
use netmonitor_core::{
    agents::{command::AgentCommandDispatcher, session::AgentSession},
    models::{_entities::discovery_runs as discovery_runs_entity, discovery_runs},
    shared::errors::{AppError, AppResult},
};
use sea_orm::{
    sea_query::Expr, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
};

use super::service::{self, ScanSessionService};

/// Folga sobre o prazo da própria varredura para a volta pelo canal.
const DISCOVERY_GRACE: Duration = Duration::from_secs(10);

/// Entrega uma run remota pendente e a marca em execução. Uma única run por
/// polling evita que um probe pequeno receba blocos grandes em paralelo.
pub async fn get_pending_discovery_tasks<C: ConnectionTrait>(
    db: &C,
    probe_id: i64,
) -> AppResult<Vec<ProbeDiscoveryTask>> {
    let Some(run) = discovery_runs::Entity::find()
        .filter(discovery_runs_entity::Column::ProbeId.eq(probe_id))
        .filter(discovery_runs_entity::Column::Status.eq("pending"))
        .order_by_asc(discovery_runs_entity::Column::Id)
        .one(db)
        .await?
    else {
        return Ok(Vec::new());
    };
    let cidr = run
        .configuration
        .as_ref()
        .and_then(|value| value.get("cidr"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let claimed = discovery_runs::Entity::update_many()
        .col_expr(
            discovery_runs_entity::Column::Status,
            Expr::value("running"),
        )
        .col_expr(
            discovery_runs_entity::Column::StartedAt,
            Expr::value(Utc::now()),
        )
        .filter(discovery_runs_entity::Column::Id.eq(run.id))
        .filter(discovery_runs_entity::Column::Status.eq("pending"))
        .exec(db)
        .await?;
    if claimed.rows_affected == 0 {
        return Ok(Vec::new());
    }
    Ok(vec![ProbeDiscoveryTask {
        id: format!("discovery-{}", run.id),
        run_id: run.id,
        cidr,
        timeout_ms: 6 * 60 * 60 * 1_000,
    }])
}

/// Verifica se a run de discovery informada pertence ao probe autenticado.
async fn assert_discovery_run_belongs_to_probe(
    ctx: &AppContext,
    run_id: i64,
    probe_id: i64,
) -> AppResult<()> {
    let Some(run) = discovery_runs::Entity::find_by_id(run_id)
        .one(&ctx.db)
        .await?
    else {
        return Err(AppError::not_found("run de discovery não encontrada"));
    };
    if run.probe_id != Some(probe_id) {
        return Err(AppError::unauthorized(
            "run de discovery não pertence a este probe",
        ));
    }
    Ok(())
}

pub async fn receive_discovery_results(
    ctx: &AppContext,
    probe_id: i64,
    payloads: &[ProbeDiscoveryResultPayload],
) -> AppResult<usize> {
    let mut processed = 0;
    for item in payloads {
        if let Err(error) = assert_discovery_run_belongs_to_probe(ctx, item.run_id, probe_id).await
        {
            tracing::warn!(
                %error,
                run_id = item.run_id,
                probe_id,
                "resultado de discovery rejeitado: run não pertence ao probe"
            );
            continue;
        }
        match service::complete_remote_discovery(
            ctx,
            probe_id,
            item.run_id,
            &item.hosts,
            item.error.as_deref(),
        )
        .await
        {
            Ok(()) => processed += 1,
            Err(error) => tracing::warn!(
                %error,
                run_id = item.run_id,
                probe_id,
                "erro ao processar discovery do probe"
            ),
        }
    }
    Ok(processed)
}

/// O despacho das varreduras pendentes de um agente conectado (o lado
/// central de `Command::Discovery`).
pub struct DiscoveryDispatch;

#[async_trait]
impl AgentCommandDispatcher for DiscoveryDispatch {
    async fn dispatch(&self, ctx: &AppContext, session: &Arc<AgentSession>) -> AppResult<()> {
        if !session.allows(Permission::Discovery) {
            return Ok(());
        }
        for task in get_pending_discovery_tasks(&ctx.db, session.probe_id).await? {
            if let Ok(scan) = ScanSessionService::from_context(ctx) {
                scan.remote_started(task.run_id).await;
            }
            tokio::spawn(run_discovery(ctx.clone(), session.clone(), task));
        }
        Ok(())
    }
}

async fn run_discovery(ctx: AppContext, session: Arc<AgentSession>, task: ProbeDiscoveryTask) {
    let timeout = Duration::from_millis(task.timeout_ms) + DISCOVERY_GRACE;
    let run_id = task.run_id;
    let task_id = task.id.clone();
    let result = session.request(Command::Discovery { task }, timeout).await;
    let payload = match result.and_then(|value| {
        serde_json::from_value::<Vec<DiscoveredHost>>(value)
            .map_err(|error| RemoteError::new(ErrorCode::Internal, error.to_string()))
    }) {
        Ok(hosts) => ProbeDiscoveryResultPayload {
            run_id,
            task_id: Some(task_id),
            hosts,
            error: None,
        },
        Err(error) => ProbeDiscoveryResultPayload {
            run_id,
            task_id: Some(task_id),
            hosts: Vec::new(),
            error: Some(error.message),
        },
    };
    if let Err(error) = receive_discovery_results(&ctx, session.probe_id, &[payload]).await {
        tracing::warn!(%error, run_id, "falha ao processar discovery do agente");
    }
}
