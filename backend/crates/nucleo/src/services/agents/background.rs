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

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use futures::{stream::FuturesUnordered, StreamExt};
use loco_rs::app::AppContext;
use netmonitor_core::{
    agents::command::AgentCommandDispatchers,
    areas::{self, Area},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use tokio::sync::Semaphore;

use crate::{
    models::_entities::probe_tasks as probe_tasks_entity,
    services::{
        events::{interest, Interests},
        monitoring::contracts::CheckResult,
        probes::{
            dispatcher::{self, ProbeTask},
            receiver::{self, ProbeResultPayload},
        },
    },
};

use super::{
    hub::AgentHub,
    policy::Permission,
    protocol::{Command, RemoteError},
    service::AgentService,
    session::AgentSession,
};

pub const LIVE_INTERVAL_MS: u64 = 3_000;
const LIVE_CHECK: Duration = Duration::from_secs(3);
const AREA_CACHE_TTL: Duration = Duration::from_secs(15);
const PUMP_INTERVAL: Duration = Duration::from_secs(1);
/// Renovação em lote de `last_seen_at` para todos os agentes conectados.
const TOUCH_INTERVAL: Duration = Duration::from_secs(30);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(10);
/// Folga sobre o timeout do próprio monitor para a volta pelo túnel.
const MONITOR_GRACE: Duration = Duration::from_secs(10);
/// Alinhado com `MAX_CONCURRENT_REQUESTS` do agente: evita enfileirar 100
/// monitores de uma vez que estourariam timeout esperando vaga no agente.
const MAX_INFLIGHT_MONITORS: usize = 16;
/// Quanto um resultado pronto espera companhia antes de ir ao banco: junta
/// os que terminam juntos sem prender o rápido atrás do mais lento do lote.
const RESULT_FLUSH_WINDOW: Duration = Duration::from_millis(500);

pub fn spawn(ctx: AppContext) {
    tokio::spawn(live_loop(ctx.clone()));
    tokio::spawn(touch_loop(ctx.clone()));
    tokio::spawn(pump_loop(ctx));
}

/// Intervalo desejado para os snapshots ao vivo: só enquanto alguma aba exibe
/// o Docker (interesse declarado no stream SSE, ARQ-05) **e** a área
/// Servidores está ligada (ADR 017). Fora disso, `0` desliga o envio.
#[must_use]
pub const fn desired_live_interval(watching: bool) -> u64 {
    if watching {
        LIVE_INTERVAL_MS
    } else {
        0
    }
}

async fn live_loop(ctx: AppContext) {
    let mut ticker = tokio::time::interval(LIVE_CHECK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut cached_servers_enabled: Option<(Instant, bool)> = None;
    loop {
        ticker.tick().await;
        let Ok(hub) = AgentHub::from_context(&ctx) else {
            continue;
        };
        let watching_sse = Interests::from_context(&ctx).is_watched(interest::DOCKER);
        let watching = if watching_sse {
            let enabled = match cached_servers_enabled {
                Some((at, value)) if at.elapsed() < AREA_CACHE_TTL => value,
                _ => {
                    let value = areas::is_enabled(&ctx.db, Area::Servers)
                        .await
                        .unwrap_or(true);
                    cached_servers_enabled = Some((Instant::now(), value));
                    value
                }
            };
            enabled
        } else {
            false
        };
        let interval = desired_live_interval(watching);
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

/// Renova `last_seen_at` de todos os agentes conectados numa única consulta a
/// cada 30s (substitui N consultas individuais por conexão).
async fn touch_loop(ctx: AppContext) {
    let mut ticker = tokio::time::interval(TOUCH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let Ok(hub) = AgentHub::from_context(&ctx) else {
            continue;
        };
        let ids: Vec<i64> = hub
            .sessions()
            .into_iter()
            .map(|session| session.probe_id)
            .collect();
        if ids.is_empty() {
            continue;
        }
        if let Err(error) = AgentService::new(&ctx.db).touch_many(&ids).await {
            tracing::warn!(%error, count = ids.len(), "falha ao renovar last_seen_at em lote");
        }
    }
}

fn monitor_limiter(probe_id: i64) -> Arc<Semaphore> {
    static LIMITERS: OnceLock<Mutex<HashMap<i64, Arc<Semaphore>>>> = OnceLock::new();
    let mut map = LIMITERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(probe_id)
        .or_insert_with(|| Arc::new(Semaphore::new(MAX_INFLIGHT_MONITORS)))
        .clone()
}

/// Quais `probe_id` conectados têm ao menos uma tarefa pendente em `probe_tasks`.
/// Uma única consulta em vez de N consultas/segundo quando não há tarefas.
async fn probes_with_pending_tasks(
    ctx: &AppContext,
    probe_ids: &[i64],
) -> crate::services::shared::errors::AppResult<HashSet<i64>> {
    if probe_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let rows: Vec<i64> = probe_tasks_entity::Entity::find()
        .select_only()
        .column(probe_tasks_entity::Column::ProbeId)
        .filter(probe_tasks_entity::Column::ProbeId.is_in(probe_ids.iter().copied()))
        .distinct()
        .into_tuple()
        .all(&ctx.db)
        .await?;
    Ok(rows.into_iter().collect())
}

async fn pump_loop(ctx: AppContext) {
    let mut ticker = tokio::time::interval(PUMP_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        let Ok(hub) = AgentHub::from_context(&ctx) else {
            continue;
        };
        let sessions = hub.sessions();
        if sessions.is_empty() {
            continue;
        }
        let monitor_ids: Vec<i64> = sessions
            .iter()
            .filter(|session| {
                session.allows(Permission::Monitor)
                    && monitor_limiter(session.probe_id).available_permits() > 0
            })
            .map(|session| session.probe_id)
            .collect();
        let active_probes = match probes_with_pending_tasks(&ctx, &monitor_ids).await {
            Ok(set) => set,
            Err(error) => {
                tracing::warn!(%error, "falha ao consultar tarefas pendentes dos agentes");
                HashSet::new()
            }
        };
        let dispatchers = ctx
            .shared_store
            .get::<AgentCommandDispatchers>()
            .unwrap_or_default();

        for session in sessions {
            if active_probes.contains(&session.probe_id) {
                if let Err(error) = pump_monitors(&ctx, &session).await {
                    tracing::warn!(%error, probe_id = session.probe_id, "falha ao entregar monitores ao agente");
                }
            }
            for dispatcher in dispatchers.iter() {
                if let Err(error) = dispatcher.dispatch(&ctx, &session).await {
                    tracing::warn!(%error, probe_id = session.probe_id, "falha ao entregar tarefas de área ao agente");
                }
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
        pump_monitors(ctx, session).await?;
    }
    // O trabalho das áreas (a varredura, da Rede): ADR 017,
    // `AgentCommandDispatcher`.
    let dispatchers = ctx
        .shared_store
        .get::<AgentCommandDispatchers>()
        .unwrap_or_default();
    for dispatcher in dispatchers.iter() {
        dispatcher.dispatch(ctx, session).await?;
    }
    Ok(())
}

async fn pump_monitors(
    ctx: &AppContext,
    session: &Arc<AgentSession>,
) -> crate::services::shared::errors::AppResult<()> {
    let tasks = dispatcher::get_pending_tasks(&ctx.db, session.probe_id).await?;
    if tasks.is_empty() {
        return Ok(());
    }
    let ctx = ctx.clone();
    let session = session.clone();
    let limiter = monitor_limiter(session.probe_id);
    tokio::spawn(run_monitor_batch(ctx, session, limiter, tasks));
    Ok(())
}

async fn run_one_monitor(
    session: Arc<AgentSession>,
    _permit: tokio::sync::OwnedSemaphorePermit,
    task: ProbeTask,
) -> Option<ProbeResultPayload> {
    execute_monitor(&session, task).await
}

/// Executa o lote com concorrência limitada a `MAX_INFLIGHT_MONITORS` e envia
/// os resultados concluídos para `receive_batch_results` em grupos: até
/// `MAX_INFLIGHT_MONITORS` ou `RESULT_FLUSH_WINDOW` depois do primeiro.
async fn run_monitor_batch(
    ctx: AppContext,
    session: Arc<AgentSession>,
    limiter: Arc<Semaphore>,
    tasks: Vec<ProbeTask>,
) {
    let probe_id = session.probe_id;
    let mut in_flight = FuturesUnordered::new();
    let mut pending = tasks.into_iter();
    let mut results = Vec::new();
    let mut first_ready: Option<tokio::time::Instant> = None;

    loop {
        while in_flight.len() < MAX_INFLIGHT_MONITORS {
            let Ok(permit) = limiter.clone().try_acquire_owned() else {
                break;
            };
            let Some(task) = pending.next() else {
                drop(permit);
                break;
            };
            in_flight.push(run_one_monitor(session.clone(), permit, task));
        }

        if in_flight.is_empty() {
            if let Some(task) = pending.next() {
                // Vai esperar vaga: o que já voltou não espera junto.
                flush_monitor_results(&ctx, probe_id, &mut results).await;
                first_ready = None;
                let Ok(permit) = limiter.clone().acquire_owned().await else {
                    break;
                };
                in_flight.push(run_one_monitor(session.clone(), permit, task));
                continue;
            }
            break;
        }

        let next = match first_ready {
            Some(at) => {
                match tokio::time::timeout_at(at + RESULT_FLUSH_WINDOW, in_flight.next()).await {
                    Ok(next) => next,
                    Err(_) => {
                        flush_monitor_results(&ctx, probe_id, &mut results).await;
                        first_ready = None;
                        continue;
                    }
                }
            }
            None => in_flight.next().await,
        };
        if let Some(Some(payload)) = next {
            if results.is_empty() {
                first_ready = Some(tokio::time::Instant::now());
            }
            results.push(payload);
            if results.len() >= MAX_INFLIGHT_MONITORS {
                flush_monitor_results(&ctx, probe_id, &mut results).await;
                first_ready = None;
            }
        }
    }
    flush_monitor_results(&ctx, probe_id, &mut results).await;
}

async fn flush_monitor_results(
    ctx: &AppContext,
    probe_id: i64,
    results: &mut Vec<ProbeResultPayload>,
) {
    if results.is_empty() {
        return;
    }
    let batch = std::mem::take(results);
    if let Err(error) = receiver::receive_batch_results(ctx, probe_id, &batch).await {
        tracing::warn!(%error, probe_id, count = batch.len(), "falha ao processar lote de resultados do agente");
    }
}

async fn execute_monitor(session: &AgentSession, task: ProbeTask) -> Option<ProbeResultPayload> {
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
        Ok(result) => Some(ProbeResultPayload {
            monitor_id,
            task_id: Some(task_id),
            result,
        }),
        // Sem resposta, o agendador enfileira de novo no próximo ciclo — e
        // roda local se o agente estiver fora (fallback do AGENTS §6).
        Err(error) => {
            tracing::debug!(%error, monitor_id, "monitor não voltou do agente");
            None
        }
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
