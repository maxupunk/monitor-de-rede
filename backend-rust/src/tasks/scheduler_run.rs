//! Um ciclo do scheduler de monitores, acionado pelo scheduler nativo do Loco.

use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    models::{monitors, probes},
    services::{
        discovery::queue::{process_pending_runs, schedule_due_networks},
        events::relay::relay_pending,
        monitoring::{
            contracts::{CheckResult, MonitorStatus},
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        probes::{
            dispatcher::{self, ProbeTask},
            liveness::{is_probe_alive, mark_stale_probes_offline},
        },
        shared::errors::AppResult,
    },
};

/// Task unitária: o Loco agenda processos, portanto não há loop infinito aqui.
pub struct SchedulerRun;

#[async_trait]
impl Task for SchedulerRun {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "scheduler_run".into(),
            detail: "Executa um ciclo dos monitores vencidos".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &task::Vars) -> Result<()> {
        run_cycle(ctx).await.map(|_| ()).map_err(Into::into)
    }
}

/// Executa os monitores vencidos e grava `next_run_at` antes de medir.
///
/// Cada bloco tem tratamento próprio: falha de um não interrompe os outros
/// (§9.2). O watchdog vem primeiro — um probe que caiu precisa aparecer como
/// caído *antes* do despacho, ou o operador só vê monitores parados sem
/// explicação.
pub async fn run_cycle(ctx: &AppContext) -> AppResult<usize> {
    if let Err(error) = mark_stale_probes_offline(ctx).await {
        tracing::warn!(%error, "falha ao revisar a vida dos probes");
    }

    let now = Utc::now();
    let due = monitors::Entity::find_due(now.into()).all(&ctx.db).await?;
    for monitor in &due {
        // Persistir primeiro evita que dois processos do scheduler executem a
        // mesma linha quando o ciclo seguinte começar antes de a rede responder.
        let mut active: monitors::ActiveModel = monitor.clone().into();
        active.next_run_at = Set(Some(
            (now + chrono::Duration::seconds(i64::from(monitor.interval_seconds.max(1)))).into(),
        ));
        active.update(&ctx.db).await?;
    }
    for monitor in &due {
        if let Err(error) = execute_one(ctx, monitor).await {
            tracing::warn!(%error, monitor_id = monitor.id, "falha ao executar monitor");
        }
    }
    // A fila de discovery é persistente: o scheduler apenas enfileira as redes
    // vencidas e processa uma por ciclo para não saturar a LAN.
    if let Err(error) = schedule_due_networks(&ctx.db).await {
        tracing::warn!(%error, "falha ao agendar discovery");
    }
    if let Err(error) = process_pending_runs(ctx).await {
        tracing::warn!(%error, "falha ao processar discovery");
    }
    if let Err(error) = relay_pending(ctx).await {
        tracing::warn!(%error, "falha ao retransmitir eventos");
    }
    Ok(due.len())
}

/// Despacho de um monitor, na ordem portada integralmente do backend anterior
/// (§9.2): probe vivo → tarefa remota; probe offline → tentativa local; nem
/// isso → observação `unknown`.
async fn execute_one(ctx: &AppContext, monitor: &monitors::Model) -> AppResult<()> {
    let timeout_ms = u64::from(monitor.timeout_seconds.max(1) as u32) * 1_000;

    if let Some(probe_id) = monitor.probe_id {
        let probe = probes::Entity::find_by_id(probe_id).one(&ctx.db).await?;
        if is_probe_alive(probe.as_ref()) {
            let task = ProbeTask {
                id: dispatcher::task_id(monitor.id, Utc::now()),
                monitor_id: monitor.id,
                task_type: monitor.r#type.clone(),
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                timeout_ms: timeout_ms as i32,
                payload: monitor.configuration.clone(),
            };
            dispatcher::dispatch_task(&ctx.db, probe_id, &task).await?;
            return Ok(());
        }

        // Probe offline: tenta a rota local antes de desistir. Se o servidor
        // tiver rota até o alvo, a checagem acontece em vez de congelar em
        // UNKNOWN. **Não remover** — diretriz do AGENTS.md §6.
        let local = run_monitor(
            ctx,
            &monitor.r#type,
            &monitor.configuration,
            RunOptions {
                timeout_ms: Some(timeout_ms),
            },
        )
        .await;
        if let Ok(result) = local {
            if result.success {
                process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
                return Ok(());
            }
        }

        let label = probe
            .map(|probe| probe.name)
            .unwrap_or_else(|| format!("#{probe_id}"));
        return report_probe_unavailable(ctx, monitor, &label).await;
    }

    let result = match run_monitor(
        ctx,
        &monitor.r#type,
        &monitor.configuration,
        RunOptions {
            timeout_ms: Some(timeout_ms),
        },
    )
    .await
    {
        Ok(result) => result,
        Err(error) => unavailable_result(&error.to_string()),
    };
    process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
    Ok(())
}

/// Registra a impossibilidade de medir como resultado `unknown`.
///
/// Não é `down`: o alvo pode estar perfeitamente no ar — quem sumiu foi o
/// agente. Mas a checagem precisa deixar rastro no histórico, senão o operador
/// vê apenas um monitor parado e sem motivo aparente (matriz de paridade #7).
async fn report_probe_unavailable(
    ctx: &AppContext,
    monitor: &monitors::Model,
    probe_label: &str,
) -> AppResult<()> {
    tracing::warn!(
        monitor_id = monitor.id,
        probe = probe_label,
        "monitor não executado: probe sem heartbeat"
    );
    let now = Utc::now();
    let result = CheckResult {
        success: false,
        status: MonitorStatus::Unknown,
        started_at: now,
        finished_at: now,
        duration_ms: 0,
        message: Some(format!(
            "Probe {probe_label} está sem heartbeat — a checagem não pôde ser executada."
        )),
        metrics: Vec::new(),
        data: serde_json::json!({ "probeId": monitor.probe_id, "reason": "probe_offline" }),
    };
    process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
    Ok(())
}

fn unavailable_result(message: &str) -> CheckResult {
    let now = Utc::now();
    CheckResult {
        success: false,
        status: MonitorStatus::Unknown,
        started_at: now,
        finished_at: now,
        duration_ms: 0,
        message: Some(format!(
            "A checagem não pôde ser executada localmente: {message}"
        )),
        metrics: Vec::new(),
        data: serde_json::json!({"reason":"local_fallback_unavailable"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indisponibilidade_local_nao_vira_down() {
        assert_eq!(unavailable_result("x").status, MonitorStatus::Unknown);
    }
}
