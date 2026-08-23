//! Executor de monitor individual, com fallback para execução local e confirmação de quedas.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::EntityTrait;

use crate::{
    models::{monitors, probes},
    services::{
        monitoring::{
            contracts::{CheckResult, MonitorStatus},
            execution_guard::{effective_timeout_seconds, try_acquire_monitor},
            ping_diagnostics,
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        probes::{
            dispatcher::{self, ProbeTask},
            liveness::is_probe_alive,
        },
        shared::errors::AppResult,
    },
};

/// Teto de tentativas extras, independentemente do que a linha do monitor diga.
pub const MAX_RETRIES: i32 = 5;

/// Despacha ou executa um monitor:
/// 1. Se tem probe_id e o probe está vivo -> despacha para o probe.
/// 2. Se o probe está offline -> tenta execução local de fallback. Se passar, grava; senão, reporta probe_unavailable.
/// 3. Se não tem probe_id -> executa localmente com confirmação de queda (retentativas imediatas).
pub async fn execute_one(ctx: &AppContext, monitor: &monitors::Model) -> AppResult<()> {
    let Some(_guard) = try_acquire_monitor(monitor.id) else {
        tracing::debug!(
            monitor_id = monitor.id,
            "monitor já em execução; ciclo ignorado"
        );
        return Ok(());
    };
    let timeout_ms =
        u64::from(
            effective_timeout_seconds(monitor.timeout_seconds, monitor.interval_seconds) as u32,
        ) * 1_000;
    let execution_configuration = ping_diagnostics::prepare_configuration(ctx, monitor).await?;

    if let Some(probe_id) = monitor.probe_id {
        let probe = probes::Entity::find_by_id(probe_id).one(&ctx.db).await?;
        if is_probe_alive(probe.as_ref()) {
            let task = ProbeTask {
                id: dispatcher::task_id(monitor.id, Utc::now()),
                monitor_id: monitor.id,
                task_type: monitor.r#type.clone(),
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                timeout_ms: timeout_ms as i32,
                payload: execution_configuration.clone(),
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
            &execution_configuration,
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

    let result =
        run_local_confirming_failure(ctx, monitor, &execution_configuration, timeout_ms).await;
    process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
    Ok(())
}

/// Executa a checagem local honrando `monitors.retry_count`.
///
/// Só reconfirma status `Down` para evitar falsos positivos por perda de pacotes transitória.
pub async fn run_local_confirming_failure(
    ctx: &AppContext,
    monitor: &monitors::Model,
    configuration: &serde_json::Value,
    timeout_ms: u64,
) -> CheckResult {
    if monitor.r#type.eq_ignore_ascii_case("ping") {
        return run_local_once(ctx, monitor, configuration, timeout_ms).await;
    }
    let attempts = 1 + monitor.retry_count.clamp(0, MAX_RETRIES);
    let budget = chrono::Duration::seconds(i64::from(monitor.interval_seconds.max(1)));
    let started = Utc::now();

    let mut used = 1;
    let mut result = run_local_once(ctx, monitor, configuration, timeout_ms).await;
    while result.status == MonitorStatus::Down && used < attempts && Utc::now() - started < budget {
        used += 1;
        result = run_local_once(ctx, monitor, configuration, timeout_ms).await;
    }

    if used > 1 {
        tracing::debug!(
            monitor_id = monitor.id,
            tentativas = used,
            status = result.status.as_str(),
            "queda reconfirmada antes de gravar o resultado"
        );
        if let Some(extras) = result.data.as_object_mut() {
            extras.insert("attempts".into(), serde_json::json!(used));
        }
    }
    result
}

async fn run_local_once(
    ctx: &AppContext,
    monitor: &monitors::Model,
    configuration: &serde_json::Value,
    timeout_ms: u64,
) -> CheckResult {
    match run_monitor(
        ctx,
        &monitor.r#type,
        configuration,
        RunOptions {
            timeout_ms: Some(timeout_ms),
        },
    )
    .await
    {
        Ok(result) => result,
        Err(error) => unavailable_result(&error.to_string()),
    }
}

/// Registra a impossibilidade de medir como resultado `unknown`.
pub async fn report_probe_unavailable(
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

#[must_use]
pub fn unavailable_result(message: &str) -> CheckResult {
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

    #[test]
    fn o_numero_de_tentativas_e_limitado_pelo_teto() {
        for (configurado, esperado) in [(-3, 1), (0, 1), (3, 4), (99, 1 + MAX_RETRIES)] {
            assert_eq!(
                1 + configurado.clamp(0, MAX_RETRIES),
                esperado,
                "retry_count {configurado} produziu contagem errada"
            );
        }
    }
}
