//! Laços de fundo da central para os agentes conectados.
//!
//! * **Ao vivo** — liga os snapshots Docker nos agentes só enquanto houver
//!   assinante SSE, o mesmo critério do coletor local. Sem tela aberta, o
//!   agente só manda os rollups de minuto (AGENTS §9: um coletor, nunca um
//!   por cliente).
//! * **Pump** — entrega pelo canal as tarefas de monitor e discovery que o
//!   agendador enfileirou em `probe_tasks`/`discovery_runs`. A fila continua
//!   no banco, então funciona com o agendador em outro processo; e o fallback
//!   local do agendador segue valendo quando o agente cai (AGENTS §6).

use std::{sync::Arc, time::Duration};

use loco_rs::app::AppContext;

use crate::services::{
    discovery::service::ScanSessionService,
    events::EventBus,
    monitoring::contracts::CheckResult,
    probes::{
        dispatcher::{self, ProbeDiscoveryTask, ProbeTask},
        receiver::{self, ProbeDiscoveryResultPayload, ProbeResultPayload},
    },
};

use super::{
    hub::AgentHub,
    policy::Permission,
    protocol::{Command, RemoteError},
    session::AgentSession,
};

pub const LIVE_INTERVAL_MS: u64 = 3_000;
const LIVE_CHECK: Duration = Duration::from_secs(3);
const PUMP_INTERVAL: Duration = Duration::from_secs(1);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(10);
/// Folga sobre o timeout do próprio monitor para a volta pelo túnel.
const MONITOR_GRACE: Duration = Duration::from_secs(10);

pub fn spawn(ctx: AppContext) {
    tokio::spawn(live_loop(ctx.clone()));
    tokio::spawn(pump_loop(ctx));
}

/// Intervalo desejado para os snapshots ao vivo.
#[must_use]
pub const fn desired_live_interval(has_subscribers: bool) -> u64 {
    if has_subscribers {
        LIVE_INTERVAL_MS
    } else {
        0
    }
}

async fn live_loop(ctx: AppContext) {
    let mut ticker = tokio::time::interval(LIVE_CHECK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        let (Ok(hub), Ok(bus)) = (AgentHub::from_context(&ctx), EventBus::from_context(&ctx))
        else {
            continue;
        };
        let interval = desired_live_interval(bus.has_subscribers());
        if hub.set_live_interval_ms(interval) {
            for session in hub.sessions() {
                tokio::spawn(async move {
                    let _ = session
                        .request(
                            Command::SetLive {
                                interval_ms: interval,
                            },
                            CONTROL_TIMEOUT,
                        )
                        .await;
                });
            }
        }
    }
}

async fn pump_loop(ctx: AppContext) {
    let mut ticker = tokio::time::interval(PUMP_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        let Ok(hub) = AgentHub::from_context(&ctx) else {
            continue;
        };
        for session in hub.sessions() {
            if let Err(error) = pump_session(&ctx, &session).await {
                tracing::warn!(%error, probe_id = session.probe_id, "falha ao entregar tarefas ao agente");
            }
        }
    }
}

/// Uma rodada de entrega para um agente.
///
/// # Errors
///
/// Propaga erro do banco ao reivindicar tarefas.
pub async fn pump_session(
    ctx: &AppContext,
    session: &Arc<AgentSession>,
) -> crate::services::shared::errors::AppResult<()> {
    if session.allows(Permission::Monitor) {
        for task in dispatcher::get_pending_tasks(&ctx.db, session.probe_id).await? {
            tokio::spawn(run_monitor(ctx.clone(), session.clone(), task));
        }
    }
    if session.allows(Permission::Discovery) {
        for task in dispatcher::get_pending_discovery_tasks(&ctx.db, session.probe_id).await? {
            if let Ok(scan) = ScanSessionService::from_context(ctx) {
                scan.remote_started(task.run_id).await;
            }
            tokio::spawn(run_discovery(ctx.clone(), session.clone(), task));
        }
    }
    Ok(())
}

async fn run_monitor(ctx: AppContext, session: Arc<AgentSession>, task: ProbeTask) {
    let timeout =
        Duration::from_millis(u64::try_from(task.timeout_ms.max(1_000)).unwrap_or(30_000))
            + MONITOR_GRACE;
    let monitor_id = task.monitor_id;
    let task_id = task.id.clone();
    match session
        .request(Command::Monitor { task }, timeout)
        .await
        .and_then(|value| {
            serde_json::from_value::<CheckResult>(value).map_err(|error| {
                RemoteError::new(super::protocol::ErrorCode::Internal, error.to_string())
            })
        }) {
        Ok(result) => {
            let payload = ProbeResultPayload {
                monitor_id,
                task_id: Some(task_id),
                result,
            };
            if let Err(error) =
                receiver::receive_batch_results(&ctx, session.probe_id, &[payload]).await
            {
                tracing::warn!(%error, monitor_id, "falha ao processar resultado do agente");
            }
        }
        // Sem resposta, o agendador enfileira de novo no próximo ciclo — e
        // roda local se o agente estiver fora (fallback do AGENTS §6).
        Err(error) => tracing::debug!(%error, monitor_id, "monitor não voltou do agente"),
    }
}

async fn run_discovery(ctx: AppContext, session: Arc<AgentSession>, task: ProbeDiscoveryTask) {
    let timeout = Duration::from_millis(task.timeout_ms) + MONITOR_GRACE;
    let run_id = task.run_id;
    let task_id = task.id.clone();
    let result = session.request(Command::Discovery { task }, timeout).await;
    let payload = match result.and_then(|value| {
        serde_json::from_value::<Vec<crate::services::discovery::merger::DiscoveredHost>>(value)
            .map_err(|error| {
                RemoteError::new(super::protocol::ErrorCode::Internal, error.to_string())
            })
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
    if let Err(error) =
        receiver::receive_discovery_results(&ctx, session.probe_id, &[payload]).await
    {
        tracing::warn!(%error, run_id, "falha ao processar discovery do agente");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ao_vivo_so_com_assinantes() {
        assert_eq!(desired_live_interval(true), LIVE_INTERVAL_MS);
        assert_eq!(desired_live_interval(false), 0);
    }
}
