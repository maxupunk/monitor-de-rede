//! Um ciclo do scheduler de monitores, acionado pelo scheduler nativo do Loco.

use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    models::{monitors, probes},
    services::{
        discovery::queue::{process_pending_runs, schedule_due_networks},
        monitoring::{
            contracts::{CheckResult, MonitorStatus},
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
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
pub async fn run_cycle(ctx: &AppContext) -> AppResult<usize> {
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
        execute_one(ctx, monitor).await?;
    }
    // A fila de discovery é persistente: o scheduler apenas enfileira as redes
    // vencidas e processa uma por ciclo para não saturar a LAN.
    if let Err(error) = schedule_due_networks(&ctx.db).await {
        tracing::warn!(%error, "falha ao agendar discovery");
    }
    if let Err(error) = process_pending_runs(ctx).await {
        tracing::warn!(%error, "falha ao processar discovery");
    }
    Ok(due.len())
}

async fn execute_one(ctx: &AppContext, monitor: &monitors::Model) -> AppResult<()> {
    if let Some(probe_id) = monitor.probe_id {
        let alive = probes::Entity::find_by_id(probe_id)
            .one(&ctx.db)
            .await?
            .is_some_and(|probe| {
                probe.status != "revoked"
                    && probe.last_seen_at.is_some_and(|seen| {
                        (Utc::now() - seen.with_timezone(&Utc)).num_seconds() <= 90
                    })
            });
        if alive {
            // A entrega remota é introduzida com a fila persistente na Fase 7.
            // Nesta fase a execução local conserva observabilidade, em vez de
            // deixar um monitor saudável sem resultado apenas por ter um probe configurado.
            tracing::debug!(
                monitor_id = monitor.id,
                probe_id,
                "probe remoto ainda sem dispatcher; executando localmente"
            );
        } else {
            // Regra de preservação: probe offline nunca encerra o ciclo sem
            // tentar a rota local. Só produz UNKNOWN se nem ela puder iniciar.
            tracing::warn!(
                monitor_id = monitor.id,
                probe_id,
                "probe offline; tentando fallback local"
            );
        }
    }
    let result = match run_monitor(
        ctx,
        &monitor.r#type,
        &monitor.configuration,
        RunOptions {
            timeout_ms: Some(monitor.timeout_seconds.max(1) as u64 * 1_000),
        },
    )
    .await
    {
        Ok(result) => result,
        Err(error) => unavailable_result(&error.to_string()),
    };
    process_result(ctx, monitor.id, &result, None).await?;
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
