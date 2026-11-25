//! Limpeza no boot (ADR 015 §4): o processo caiu no meio de um teste.
//!
//! 1. Toda perna com limpeza pendente (`cleanup`, gravada antes de o
//!    servidor ser usado) tem o `stop` do plugin chamado com o `handle`
//!    guardado — o MikroTik volta a autenticar o `bandwidth-server`, o
//!    `iperf3 -s` do OpenWrt é encerrado. Sem sucesso, a pendência fica para
//!    o próximo boot.
//! 2. Todo teste que constava como em curso vira `failed` com
//!    [`INTERRUPTED`]; pernas e etapas por terminar também.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{
    env::LinkTestEnv,
    events, guard,
    model::{as_db, from_db, LinkTestStage, LinkTestStatus, LinkTestStepStatus},
    repository,
    runner::PendingCleanup,
    schedules,
};
use netmonitor_core::shared::{errors::AppResult, transaction::begin_write};
use netmonitor_network::plugins::{
    invoke::{RunStatus, ServiceRunRequest},
    runs::Origin,
};

/// Erro dos testes interrompidos.
pub const INTERRUPTED: &str = "interrompido pelo reinício do servidor";

/// O que a limpeza fez.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoverStats {
    pub stopped: usize,
    pub still_pending: usize,
    pub interrupted: usize,
}

async fn stop_one(env: &LinkTestEnv, cleanup: &PendingCleanup, test_id: i64) -> bool {
    let request = ServiceRunRequest {
        device_id: cleanup.device_id,
        plugin_slug: cleanup.plugin_slug.clone(),
        action: cleanup.action.clone(),
        params: json!({ "engine": cleanup.engine.as_str(), "handle": cleanup.handle }),
        origin: Origin::LinkTest,
        user_id: None,
        reason: Some(format!(
            "Teste de enlace #{test_id}: limpeza após o reinício do servidor"
        )),
        progress: None,
        cancel: Some(CancellationToken::new()),
    };
    match env.plugins.execute(request, None).await {
        Ok(run) => {
            run.status == RunStatus::Succeeded
                && run
                    .output
                    .as_ref()
                    .and_then(|output| output.get("stopped"))
                    .and_then(Value::as_bool)
                    != Some(false)
        }
        Err(error) => {
            tracing::warn!(%error, device_id = cleanup.device_id, "limpeza de teste de enlace pendente");
            false
        }
    }
}

/// Desfaz o que um processo anterior deixou de pé.
///
/// # Errors
///
/// Erro do banco.
pub async fn recover(env: &LinkTestEnv) -> AppResult<RecoverStats> {
    recover_before(env, Utc::now()).await
}

/// Desfaz apenas pendências e testes ativos anteriores a `boot_time` que não
/// estejam rodando neste processo — impede corrida quando `recover` roda em
/// segundo plano enquanto a API já aceita novos testes.
///
/// # Errors
///
/// Erro do banco.
pub async fn recover_before(
    env: &LinkTestEnv,
    boot_time: DateTime<Utc>,
) -> AppResult<RecoverStats> {
    let mut stats = RecoverStats::default();
    for mut leg in repository::pending_cleanups(&env.db).await? {
        if guard::is_running(leg.test_id) || leg.created_at.to_utc() > boot_time {
            continue;
        }
        let Some(cleanup) = leg
            .cleanup
            .clone()
            .and_then(|value| serde_json::from_value::<PendingCleanup>(value).ok())
        else {
            leg.cleanup = None;
            repository::save_leg(&env.db, &leg).await?;
            continue;
        };
        if stop_one(env, &cleanup, leg.test_id).await {
            leg.cleanup = None;
            repository::save_leg(&env.db, &leg).await?;
            stats.stopped += 1;
        } else {
            stats.still_pending += 1;
        }
    }

    let now = Utc::now();
    for mut test in repository::active(&env.db).await? {
        if guard::is_running(test.id) || test.created_at.to_utc() > boot_time {
            continue;
        }
        let txn = begin_write(&env.db).await?;
        for mut leg in repository::legs_of(&txn, &[test.id]).await? {
            let status =
                from_db::<LinkTestStepStatus>(&leg.status).unwrap_or(LinkTestStepStatus::Pending);
            if status.is_final() {
                continue;
            }
            let mut stages: Vec<LinkTestStage> =
                serde_json::from_value(leg.stages.clone()).unwrap_or_default();
            for stage in &mut stages {
                if !stage.status.is_final() {
                    stage.status = LinkTestStepStatus::Failed;
                    stage.message = Some(INTERRUPTED.to_owned());
                    stage.finished_at = Some(now.to_rfc3339());
                }
            }
            leg.stages = json!(stages);
            leg.status = as_db(&LinkTestStepStatus::Failed);
            leg.error = Some(INTERRUPTED.to_owned());
            leg.finished_at = Some(now.into());
            repository::save_leg(&txn, &leg).await?;
        }
        test.status = as_db(&LinkTestStatus::Failed);
        test.error = Some(INTERRUPTED.to_owned());
        test.finished_at = Some(now.into());
        repository::save_test(&txn, &test).await?;
        txn.commit().await?;

        if let Ok(detail) = repository::detail(&env.db, test.id).await {
            events::publish_test(env.bus.as_ref(), &detail);
        }
        if let Some(schedule_id) = test.schedule_id {
            if let Err(error) =
                schedules::record_result(env, schedule_id, test.id, LinkTestStatus::Failed).await
            {
                tracing::warn!(%error, schedule_id, test_id = test.id, "falha ao atualizar agendamento interrompido");
            }
        }
        stats.interrupted += 1;
    }
    if stats != RecoverStats::default() {
        tracing::info!(
            encerrados = stats.stopped,
            pendentes = stats.still_pending,
            interrompidos = stats.interrupted,
            "limpeza de testes de enlace interrompidos"
        );
    }
    Ok(stats)
}
