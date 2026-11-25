//! Backup automático do NetMonitor: o ciclo que manda a cópia da configuração
//! ao destino do plano quando ela vence.
//!
//! Um laço só, no processo do servidor, que a cada minuto pergunta ao plano se
//! venceu ([`BackupPolicy::is_due`]). Não há fila nem estado em memória: o
//! `last_backup_at` gravado é a agenda, e um restart retoma de onde parou.

use std::time::Duration;

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;

use super::{copies, notify, plan};
use crate::services::{
    shared::{
        errors::{AppError, AppResult},
        run_guard::RunGuard,
    },
    storage,
};

#[cfg(doc)]
use crate::services::shared::backup_schedule::BackupPolicy;

/// De quanto em quanto tempo o ciclo confere os vencimentos.
const TICK: Duration = Duration::from_secs(60);

/// Espera antes da primeira conferência: o boot já tem trabalho de sobra, e um
/// restart em sequência não deve disparar uma rajada de backups.
const FIRST_TICK_DELAY: Duration = Duration::from_secs(120);

/// O ciclo, para sempre. Rotina do processo líder (ADR 024): quem o sobe é o
/// ponto de montagem, fora do ambiente de teste — lá, quem testa chama
/// [`run_due`] diretamente.
pub async fn run(ctx: AppContext) {
    tokio::time::sleep(FIRST_TICK_DELAY).await;
    loop {
        if let Err(error) = run_due(&ctx, Utc::now()).await {
            tracing::warn!(%error, "ciclo de backup automático falhou");
        }
        tokio::time::sleep(TICK).await;
    }
}

/// Executa o plano contra um destino já carregado, publica `backup_plan:updated`
/// nos dois desfechos e notifica em caso de falha.
///
/// Compartilhado entre o agendador automático (`scheduled`) e o "Fazer backup
/// agora" (`manual`).
///
/// # Errors
///
/// Propaga a falha de [`copies::run`].
pub async fn run_plan(
    ctx: &AppContext,
    current: &crate::models::system_backup_plan::Model,
    destination: &storage::service::Destination,
    trigger: &'static str,
) -> AppResult<copies::RunOutcome> {
    let result = copies::run(&ctx.db, current, destination, crate::app_version()).await;
    // O resultado — inclusive a falha — já foi gravado no plano: as outras
    // abas precisam ver o novo status nos dois casos.
    plan::publish_updated(ctx).await;
    if let Err(AppError::Conflict(ref busy)) = result {
        // Outra cópia ou uma restauração segurava a vez: nada foi tentado nem
        // gravado no plano, e notificar "backup falhou" a cada minuto de uma
        // restauração longa seria alarme falso.
        tracing::info!(trigger, busy = %busy, "backup do NetMonitor adiado: outra operação em andamento");
    } else if let Err(ref error) = result {
        tracing::warn!(%error, trigger, "backup do NetMonitor falhou");
        notify::notify_system_backup_failed(
            ctx,
            Some(&destination.row.name),
            trigger,
            &error.to_string(),
        )
        .await;
    }
    result
}

/// Faz a cópia do sistema se o plano venceu. Devolve se rodou.
///
/// A falha fica gravada no plano (ou só no log, quando nem o destino abre) e
/// o ciclo segue.
///
/// # Errors
///
/// Só a falha da leitura do plano.
pub async fn run_due(ctx: &AppContext, now: DateTime<Utc>) -> AppResult<bool> {
    let current = plan::get(&ctx.db).await?;
    if !plan::policy(&current).is_due(now) {
        return Ok(false);
    }
    let Some(destination_id) = current.storage_destination_id else {
        return Ok(false);
    };
    // Restauração ou outra cópia em curso: pula o ciclo sem registrar falha.
    // O plano continua vencido e o próximo minuto tenta de novo.
    if RunGuard::is_running(copies::RUN_SCOPE, plan::PLAN_ID) {
        return Ok(false);
    }
    match storage::service::load(&ctx.db, destination_id).await {
        Ok(destination) => {
            let _ = run_plan(ctx, &current, &destination, "scheduled").await;
        }
        Err(error) => {
            let msg = error.to_string();
            let _ = copies::record_failure(&ctx.db, &current, now, &msg).await;
            tracing::warn!(%error, "backup automático do NetMonitor falhou");
            notify::notify_system_backup_failed(ctx, None, "scheduled", &msg).await;
            plan::publish_updated(ctx).await;
        }
    }
    Ok(true)
}
