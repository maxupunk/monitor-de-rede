//! Backup automático do NetMonitor: o ciclo que manda a cópia da configuração
//! ao destino do plano quando ela vence.
//!
//! Um laço só, no processo do servidor, que a cada minuto pergunta ao plano se
//! venceu ([`BackupPolicy::is_due`]). Não há fila nem estado em memória: o
//! `last_backup_at` gravado é a agenda, e um restart retoma de onde parou.

use std::time::Duration;

use chrono::{DateTime, Utc};
use loco_rs::{app::AppContext, app::Hooks};

use super::{copies, plan};
use crate::{
    app::App,
    services::{shared::errors::AppResult, storage},
};

#[cfg(doc)]
use crate::services::shared::backup_schedule::BackupPolicy;

/// De quanto em quanto tempo o ciclo confere os vencimentos.
const TICK: Duration = Duration::from_secs(60);

/// Espera antes da primeira conferência: o boot já tem trabalho de sobra, e um
/// restart em sequência não deve disparar uma rajada de backups.
const FIRST_TICK_DELAY: Duration = Duration::from_secs(120);

/// Sobe o ciclo. Fora do ambiente de teste — lá, quem testa chama
/// [`run_due`] diretamente.
pub fn spawn(ctx: &AppContext) {
    if ctx.environment == loco_rs::environment::Environment::Test {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(FIRST_TICK_DELAY).await;
        loop {
            if let Err(error) = run_due(&ctx, Utc::now()).await {
                tracing::warn!(%error, "ciclo de backup automático falhou");
            }
            tokio::time::sleep(TICK).await;
        }
    });
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
    let outcome = match storage::service::load(&ctx.db, destination_id).await {
        Ok(destination) => copies::run(&ctx.db, &current, &destination, App::app_version())
            .await
            .map(|_| ()),
        Err(error) => Err(error),
    };
    if let Err(error) = outcome {
        tracing::warn!(%error, "backup automático do NetMonitor falhou");
    }
    plan::publish_updated(ctx).await;
    Ok(true)
}
